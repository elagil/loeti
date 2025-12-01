//! Drives the tool's heating element, based on target and actual temperature.

mod library;
pub mod measurement;
pub mod tool;

use core::f32;
use defmt::{Format, debug, error, warn};
use embassy_futures::select::{Either, select};
use embassy_stm32::dac::Ch1;
use embassy_stm32::mode::Blocking;
use embassy_stm32::timer::simple_pwm::SimplePwm;
use embassy_stm32::{dac, exti::ExtiInput, gpio::Input, peripherals};
use embassy_time::{Duration, Ticker, Timer, WithTimeout};
use library::{TOOLS, ToolProperties};
use tool::{Tool, ToolState};
use uom::si::electric_potential;
use uom::si::f32::ElectricCurrent;
use uom::si::f32::ElectricPotential;
use uom::si::f32::Power;
use uom::si::{electric_current, power};

/// The type for the PWM heater channel.
type PwmHeaterChannel<'d> =
    embassy_stm32::timer::simple_pwm::SimplePwmChannel<'d, peripherals::TIM1>;

#[cfg(feature = "comm")]
use crate::comm;
#[cfg(feature = "display")]
use crate::ui::display::{display_current_power, display_current_temperature, display_power_limit};
use crate::{AutoSleep, OPERATIONAL_STATE_MUTEX, PERSISTENT_MUTEX, Persistent};

/// The number of current control iterations per temperature control iteration.
const CURRENT_CONTROL_CYCLE_COUNT: u64 = 5;
/// The total loop period in ms (temperature loop).
const LOOP_PERIOD_MS: u64 = 100;
/// The current loop period in ms.
const CURRENT_LOOP_PERIOD_MS: u64 = LOOP_PERIOD_MS / CURRENT_CONTROL_CYCLE_COUNT;

// Current monitor total gain (shunt + amplifier): 0.2 V/A
/// The DAC output voltage for a 10 A current limit.
const DAC_VOLTAGE_10A: u16 = 2825; // 2.0 V output -> Limit to 10 A
/// The DAC output voltage for a 0.1 A current limit.
const DAC_VOLTAGE_0A1: u16 = 28; // 0.02 V output -> Limit to 0.1 A

/// Errors during tool detection.
#[derive(Debug, Format, Clone, Copy)]
pub enum Error {
    /// No tool was found.
    NoTool,
    /// Tool was detected, but no tip.
    NoTip,
    /// The detected tool is unknown.
    UnknownTool,
    /// Tool type mismatch during control loop operation.
    ToolMismatch,
}

/// Resources for driving the tool's heater and taking associated measurements.
pub struct ToolResources {
    /// ADC for measurements.
    pub adc_resources: measurement::AdcResources,

    /// The DAC that sets the current limit for the current sensing IC (INA301A).
    pub dac_current_limit: dac::DacChannel<'static, peripherals::DAC1, Ch1, Blocking>,

    /// External interrupt for current alerts.
    pub exti_current_alert: ExtiInput<'static>,

    /// The PWM for driving the tool's heating element.
    pub pwm_heater: SimplePwm<'static, peripherals::TIM1>,

    /// A pin for detecting the tool sleep position (in holder).
    pub pin_sleep: Input<'static>,
}

impl ToolResources {
    /// Get the PWM heater channel.
    fn pwm_heater_channel(&mut self) -> PwmHeaterChannel<'_> {
        self.pwm_heater.ch1()
    }
}

/// Properties of the tool's power supply.
#[derive(Default, Debug, Clone)]
struct Supply {
    /// The maximum allowed current.
    current_limit: ElectricCurrent,
    /// A margin to leave until the limit (reduces the effective limit).
    current_margin: ElectricCurrent,
    /// The negotiated potential.
    potential: ElectricPotential,
}

impl Supply {
    /// The supply's maximum current minus the margin to leave.
    ///
    /// This is the effectively usable current limit.
    fn effective_current_limit(&self) -> ElectricCurrent {
        self.current_limit - self.current_margin
    }

    /// Calculate the supply's maximum power output.
    fn _power_limit(&self) -> Power {
        self.current_limit * self.potential
    }

    /// Calculate the supply's maximum power output, taking into account the margin to leave.
    ///
    /// This is the effectively usable power limit.
    fn effective_power_limit(&self) -> Power {
        self.effective_current_limit() * self.potential
    }

    /// Calculate the supply's maximum power output in W, taking into account the margin to leave.
    ///
    /// This is the effectively usable power limit.
    fn effective_power_limit_w(&self) -> f32 {
        self.effective_power_limit().get::<power::watt>()
    }

    /// The supply's maximum current, in Ampere.
    fn _current_limit_a(&self) -> f32 {
        self.current_limit.get::<electric_current::ampere>()
    }

    /// The supply's maximum current minus the margin to leave, in Ampere.
    ///
    /// This is the effectively usable current limit.
    fn effective_current_limit_a(&self) -> f32 {
        self.effective_current_limit()
            .get::<electric_current::ampere>()
    }

    /// The supply's potential in Volt.
    fn potential_v(&self) -> f32 {
        self.potential.get::<electric_potential::volt>()
    }
}

/// Top-level iron temperature and current control.
struct Control<'d> {
    /// The tool to control.
    tool: Option<Tool>,
    /// The resources of the tool to control.
    tool_resources: &'d mut ToolResources,
    /// Power supply properties.
    supply: Option<Supply>,

    /// The set temperature of the tool when in use (not in the stand).
    operational_temperature_deg_c: Option<f32>,
}

impl<'d> Control<'d> {
    /// Create a new control instance.
    fn new(tool_resources: &'d mut ToolResources, supply: Supply) -> Self {
        let this = Self {
            tool: None,
            tool_resources,
            supply: Some(supply),
            operational_temperature_deg_c: None,
        };

        this.tool_resources
            .dac_current_limit
            .set_mode(dac::Mode::NormalExternalBuffered);
        this.tool_resources.dac_current_limit.enable();
        this.tool_resources
            .dac_current_limit
            .set(dac::Value::Bit12Right(DAC_VOLTAGE_10A));

        this
    }

    /// Check if a tip is inserted by passing a small test current.
    ///
    /// Use the current-sensor overcurrent interrupt for a quick evaluation.
    /// Before passing the test current, the current limit is set to just 100 mA.
    async fn test_tip_current(&mut self) -> Result<(), Error> {
        {
            // Set a low current limit.
            self.tool_resources
                .dac_current_limit
                .set(dac::Value::Bit12Right(DAC_VOLTAGE_0A1));

            // Enable heater power output.
            let duty = 0.1 * self.tool_resources.pwm_heater_channel().max_duty_cycle() as f32;
            self.tool_resources
                .pwm_heater_channel()
                .set_duty_cycle(duty as u16);

            // React to overcurrent event. Times out if no tip is present.
            let tip_present = self
                .tool_resources
                .exti_current_alert
                .wait_for_low()
                .with_timeout(Duration::from_micros(50))
                .await
                .is_ok();

            self.tool_resources
                .pwm_heater_channel()
                .set_duty_cycle_fully_off();

            // Restore original current limit.
            self.tool_resources
                .dac_current_limit
                .set(dac::Value::Bit12Right(DAC_VOLTAGE_10A));

            if !tip_present {
                return Err(Error::NoTip);
            }

            Ok(())
        }
    }

    /// Detect a connected tool.
    async fn detect_tool(&mut self, persistent: &Persistent) -> Result<(), Error> {
        let tool_measurement = self.tool_resources.adc_resources.measure_tool().await?;

        // Check if tool is in its stand.
        let tool_in_stand = self.tool_resources.pin_sleep.is_low();

        self.test_tip_current().await?;

        if self.tool.is_none() {
            self.tool = Some(Tool::new(tool_measurement, self.supply.take().unwrap())?);
        };

        let tool = self.tool.as_mut().unwrap();
        tool.detect_and_calculate_temperature(tool_measurement)?;
        tool.update_tool_state(tool_in_stand, persistent.auto_sleep);

        Ok(())
    }

    /// Handles the main tool control loop.
    ///
    /// - Detects whether a tool is present
    /// - Runs temperature (outer) control loop, while measuring tool temperature
    /// - Runs current (inner) control loop, while measuring voltage and current on the tool
    async fn run(&mut self) -> Result<(), Error> {
        self.tool_resources
            .pwm_heater_channel()
            .set_duty_cycle_fully_off();
        self.tool_resources.pwm_heater_channel().enable();

        let idle_power_measurement = self.tool_resources.adc_resources.measure_tool_power().await;

        // Measure idle current for potential offset compensation.
        debug!(
            "Idle current: {} mA",
            idle_power_measurement
                .current
                .get::<electric_current::milliampere>()
        );

        loop {
            // Settling time for temperature ADC filter.
            Timer::after_millis(1).await;

            let persistent = PERSISTENT_MUTEX.lock(|x| *x.borrow());

            self.detect_tool(&persistent).await?;
            let tool = self.tool.as_mut().unwrap();

            let operational_state = OPERATIONAL_STATE_MUTEX.lock(|x| {
                let mut operational_state = x.borrow_mut();
                operational_state.tool_state = Some(tool.state);
                operational_state.tool = Ok(tool.name());

                *operational_state
            });

            tool.supply.current_margin = ElectricCurrent::new::<electric_current::milliampere>(
                persistent.current_margin_ma as f32,
            );

            #[cfg(feature = "display")]
            display_power_limit(Some(tool.power_limit_w()));

            if !operational_state.set_temperature_is_pending {
                self.operational_temperature_deg_c = Some(persistent.set_temperature_deg_c as f32);
            }
            let stand_temperature_deg_c = Some(
                (persistent.stand_temperature_deg_c as f32)
                    .min(self.operational_temperature_deg_c.unwrap_or(f32::INFINITY)),
            );

            let set_temperature_deg_c = if tool.in_stand() {
                stand_temperature_deg_c
            } else {
                self.operational_temperature_deg_c
            };

            if set_temperature_deg_c.is_none() {
                error!("No set temperature available.");
                continue;
            }

            #[cfg(feature = "display")]
            display_current_temperature(tool.temperature_deg_c);

            #[cfg(feature = "comm")]
            {
                let status = loeti_protocol::Status {
                    time_ms: embassy_time::Instant::now().as_millis(),
                    control_state: loeti_protocol::ControlState::Tool(match tool.state {
                        ToolState::Active => {
                            loeti_protocol::ToolState::Active(tool.get_temperature_pid_parameters())
                        }
                        ToolState::InStand(instant) => {
                            loeti_protocol::ToolState::InStand(instant.as_millis())
                        }
                        ToolState::Sleeping => loeti_protocol::ToolState::Sleeping,
                    }),
                };
                comm::send_status(&status);
            }

            if operational_state.tool_is_off || matches!(tool.state, ToolState::Sleeping) {
                // FIXME: Handle inside of Tool?
                tool.temperature_pid.reset_integral_term();

                let mut power_measurement =
                    self.tool_resources.adc_resources.measure_tool_power().await;
                power_measurement.compensate(&idle_power_measurement);

                #[cfg(feature = "display")]
                display_current_power(None);

                Timer::after_millis(LOOP_PERIOD_MS).await;

                #[cfg(feature = "comm")]
                comm::send_measurement(&loeti_protocol::Measurement {
                    time_ms: embassy_time::Instant::now().as_millis(),
                    temperature_deg_c: tool.temperature_deg_c,
                    set_temperature_deg_c,
                    ..Default::default()
                });

                // Skip the rest of the control loop.
                continue;
            }

            let _measurement = tool.run_temperature_control(set_temperature_deg_c.unwrap())?;

            #[cfg(feature = "comm")]
            comm::send_measurement(&_measurement);

            let mut current_loop_ticker =
                Ticker::every(Duration::from_millis(CURRENT_LOOP_PERIOD_MS));
            for _ in 0..CURRENT_CONTROL_CYCLE_COUNT {
                let ratio = tool.pwm_ratio();

                let duty = ratio * self.tool_resources.pwm_heater_channel().max_duty_cycle() as f32;
                self.tool_resources
                    .pwm_heater_channel()
                    .set_duty_cycle(duty as u16);

                #[cfg(feature = "display")]
                display_current_power(Some(tool.power_ratio()));

                let tool_power_fut = async {
                    // Measure current and voltage after the low-pass filter settles - in the middle of the loop period.
                    Timer::after_millis(CURRENT_LOOP_PERIOD_MS / 2).await;

                    let mut power_measurement =
                        self.tool_resources.adc_resources.measure_tool_power().await;
                    power_measurement.compensate(&idle_power_measurement);

                    // Wait for the end of this cycle.
                    current_loop_ticker.next().await;
                    power_measurement
                };

                match select(
                    tool_power_fut,
                    self.tool_resources.exti_current_alert.wait_for_low(),
                )
                .await
                {
                    Either::First(power_measurement) => {
                        tool.run_current_control(&power_measurement)
                    }
                    Either::Second(_) => {
                        warn!("Current alert");
                        break;
                    }
                };
            }

            self.tool_resources
                .pwm_heater_channel()
                .set_duty_cycle_fully_off();
        }
    }
}

/// Control the tool's heating element.
///
/// Takes current and temperature measurements, and adjusts the heater PWM duty cycle accordingly.
#[embassy_executor::task]
pub async fn tool_task(mut tool_resources: ToolResources, negotiated_supply: (u32, u32)) {
    debug!("Maximum measurable voltage: {} V", measurement::MAX_ADC_V);
    debug!(
        "Maximum measurable detection resistor ratio: {}",
        measurement::MAX_ADC_RATIO
    );

    loop {
        let supply = Supply {
            current_limit: ElectricCurrent::new::<electric_current::milliampere>(
                negotiated_supply.1 as f32,
            ),
            potential: ElectricPotential::new::<electric_potential::millivolt>(
                negotiated_supply.0 as f32,
            ),
            ..Default::default()
        };

        let mut control = Control::new(&mut tool_resources, supply);
        let result = control.run().await;

        if let Err(error) = result {
            #[cfg(feature = "comm")]
            {
                let status = loeti_protocol::Status {
                    time_ms: embassy_time::Instant::now().as_millis(),
                    control_state: match error {
                        Error::NoTool => loeti_protocol::ControlState::NoTool,
                        Error::NoTip => loeti_protocol::ControlState::NoTip,
                        Error::UnknownTool => loeti_protocol::ControlState::UnknownTool,
                        Error::ToolMismatch => loeti_protocol::ControlState::ToolMismatch,
                    },
                };
                comm::send_status(&status);
            }

            let sleep_on_error = PERSISTENT_MUTEX.lock(|x| x.borrow().off_on_change);

            OPERATIONAL_STATE_MUTEX.lock(|x| {
                let mut operational_state = x.borrow_mut();
                operational_state.tool_state = None;
                operational_state.tool = Err(error);
                operational_state.tool_is_off |= sleep_on_error;
            });

            warn!("Tool control error: {}", error);
            Timer::after_millis(100).await
        }
    }
}
