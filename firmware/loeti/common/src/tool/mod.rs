//! Drives the tool's heating element, based on target and actual temperature.

use core::f32;

use defmt::{Format, debug, error, info, trace, warn};
use embassy_futures::select::{Either, select};
use embassy_stm32::Peri;
use embassy_stm32::dac::Ch1;
use embassy_stm32::mode::Blocking;
use embassy_stm32::{
    adc, dac, exti::ExtiInput, gpio::Input, peripherals, timer::simple_pwm::SimplePwm,
};
use embassy_time::{Duration, Instant, Ticker, Timer};
use pid::{self, Pid};
use uom::si::electric_potential;
use uom::si::electric_potential::volt;
use uom::si::electrical_resistance::ohm;
use uom::si::f32::ElectricCurrent;
use uom::si::f32::ElectricPotential;
use uom::si::f32::ElectricalResistance;
use uom::si::f32::Power;
use uom::si::f32::Ratio;
use uom::si::f32::ThermodynamicTemperature;
use uom::si::ratio;
use uom::si::ratio::percent;
use uom::si::thermodynamic_temperature;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::{electric_current, power};

mod library;
use library::{TOOLS, ToolProperties};
use uom::ConstZero;

#[cfg(feature = "comm")]
use crate::comm;
#[cfg(feature = "display")]
use crate::ui::display::{display_current_power, display_current_temperature, display_power_limit};
use crate::{AutoSleep, OPERATIONAL_STATE_MUTEX, PERSISTENT_MUTEX};

/// ADC max. value (16 bit).
const ADC_MAX: f32 = 65535.0;
/// ADC sample time in cycles.
const ADC_SAMPLE_TIME: adc::SampleTime = adc::SampleTime::CYCLES247_5;
/// The number of current control iterations per temperature control iteration.
const CURRENT_CONTROL_CYCLE_COUNT: u64 = 5;
/// The total loop period in ms (temperature loop).
const LOOP_PERIOD_MS: u64 = 100;
/// The current loop period in ms.
const CURRENT_LOOP_PERIOD_MS: u64 = LOOP_PERIOD_MS / CURRENT_CONTROL_CYCLE_COUNT;

/// The ADC reference voltage.
const VREFBUF_V: f32 = 2.9;
/// The analog supply voltage.
const ANALOG_SUPPLY_V: f32 = 3.3;
/// The value at which an ADC voltage is considered to be at the upper limit.
const MAX_ADC_V: f32 = VREFBUF_V - 0.1;
/// The ratio between the defined maximum ADC voltage and analog supply voltage.
const MAX_ADC_RATIO: f32 = MAX_ADC_V / ANALOG_SUPPLY_V;

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

/// Convert an ADC value to measured voltage.
fn adc_value_to_potential(value: u16) -> ElectricPotential {
    ElectricPotential::new::<volt>(VREFBUF_V * (value as f32) / ADC_MAX)
}

/// Resources for the ADC.
pub struct AdcResources {
    /// The ADC.
    pub adc: adc::Adc<'static, peripherals::ADC1>,
    /// The ADC temperature input pin.
    pub pin_temperature: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The ADC detection input pin.
    pub pin_detect: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The ADC input for voltage on the bus.
    pub pin_voltage: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The ADC input for heater current.
    pub pin_current: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The DMA for the ADC.
    pub adc_dma: Peri<'static, peripherals::DMA1_CH6>,
}

/// Resources for driving the tool's heater and taking associated measurements.
pub struct ToolResources {
    /// ADC for measurements.
    pub adc_resources: AdcResources,

    /// The DAC that sets the current limit for the current sensing IC (INA301A).
    pub dac_current_limit: dac::DacChannel<'static, peripherals::DAC1, Ch1, Blocking>,

    /// External interrupt for current alerts.
    pub exti_current_alert: ExtiInput<'static>,

    /// The PWM for driving the tool's heating element.
    pub pwm_heater: SimplePwm<'static, peripherals::TIM1>,

    /// A pin for detecting the tool sleep position (in holder).
    pub pin_sleep: Input<'static>,
}

/// A tool's raw measurements.
#[derive(Clone, Copy)]
struct RawToolMeasurement {
    /// The result of measuring the detection circuit.
    ///
    /// The detection ratio is used for assigning a certain tool from the library of supported tools.
    detect_ratio: Ratio,
    /// The raw thermocouple voltage.
    temperature_potential: ElectricPotential,
}

impl RawToolMeasurement {
    /// Derive a tool's temperature, given its unique properties.
    ///
    /// The temperature is invalid if the ADC voltage is zero or below. The hardware cannot measure negative
    /// thermocouple voltages, thus reports invalid temperature measurements in such cases.
    fn temperature(&self, tool_properties: &ToolProperties) -> Option<ThermodynamicTemperature> {
        if self.temperature_potential <= ElectricPotential::ZERO {
            return None;
        }

        Some(ThermodynamicTemperature::new::<degree_celsius>(
            tool_properties
                .temperature_calibration
                .calc_temperature_c(self.temperature_potential.get::<volt>()),
        ))
    }
}

/// Take raw measurements of a tool.
///
/// When the tool properties are known, they can be translated to useful values (e.g. temperature).
async fn measure_tool(
    adc_resources: &mut AdcResources,
    detect_ratio_threshold: Ratio,
    temperature_potential_threshold: ElectricPotential,
) -> Result<RawToolMeasurement, Error> {
    let mut adc_buffer = [0u16; 2];

    adc_resources
        .adc
        .read(
            adc_resources.adc_dma.reborrow(),
            [
                (&mut adc_resources.pin_detect, ADC_SAMPLE_TIME),
                (&mut adc_resources.pin_temperature, ADC_SAMPLE_TIME),
            ]
            .into_iter(),
            &mut adc_buffer,
        )
        .await;

    trace!("Measured tool, ADC values: {}", adc_buffer);

    let detect_ratio = adc_value_to_potential(adc_buffer[0])
        / ElectricPotential::new::<electric_potential::volt>(3.3);
    let temperature_potential = adc_value_to_potential(adc_buffer[1]);

    if detect_ratio > detect_ratio_threshold {
        Err(Error::NoTool)
    } else if temperature_potential > temperature_potential_threshold {
        Err(Error::NoTip)
    } else {
        Ok(RawToolMeasurement {
            detect_ratio,
            temperature_potential,
        })
    }
}

/// A tool power measurement.
struct PowerMeasurement {
    /// The electric current through the tool.
    current: ElectricCurrent,
    /// The supply voltage.
    ///
    /// FIXME: Use for checking drop from negotiated voltage?
    _potential: ElectricPotential,
}

impl PowerMeasurement {
    /// Calculate the tool's power dissipation.
    fn _power(&self) -> Power {
        self._potential * self.current
    }

    /// Compensate current with an idle power measurement.
    fn compensate(&mut self, idle: &Self) {
        self.current = (self.current - idle.current).max(ElectricCurrent::ZERO);
    }
}

/// Measure the tool's power (voltage and current).
async fn measure_tool_power(adc_power_resources: &mut AdcResources) -> PowerMeasurement {
    let mut adc_buffer = [0u16; 2];

    adc_power_resources
        .adc
        .read(
            adc_power_resources.adc_dma.reborrow(),
            [
                (&mut adc_power_resources.pin_current, ADC_SAMPLE_TIME),
                (&mut adc_power_resources.pin_voltage, ADC_SAMPLE_TIME),
            ]
            .into_iter(),
            &mut adc_buffer,
        )
        .await;

    let current_sense_resistance = ElectricalResistance::new::<ohm>(0.2);
    let current = adc_value_to_potential(adc_buffer[0]) / current_sense_resistance;

    const VOLTAGE_DIVIDER_RATIO: f32 = 7.667;
    let potential = VOLTAGE_DIVIDER_RATIO * adc_value_to_potential(adc_buffer[1]);

    PowerMeasurement {
        current,
        _potential: potential,
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

/// The state of the tool.
#[derive(Debug, Clone, Copy, Format)]
pub enum ToolState {
    /// The tool is active.
    Active,
    /// The tool was placed in its stand at the recorded instant.
    InStand(Instant),
    /// The tool was automatically switched to sleep mode.
    Sleeping,
}

/// A tool (soldering iron).
struct Tool {
    /// Unique properties of the tool.
    properties: &'static ToolProperties,
    /// The temperature control.
    temperature_pid: Pid<f32>,
    /// The current control.
    current_pid: Pid<f32>,
    /// The current  PWM ratio of the heater switch (MOSFET).
    pwm_ratio: Ratio,
    /// The current temperature.
    ///
    /// Can be `None`, if the ADC reading was invalid.
    temperature_deg_c: Option<f32>,
    /// The tool supply's characteristics.
    supply: Supply,
    /// The state of the tool.
    state: ToolState,
}

impl Tool {
    /// Create a new tool from a measurement.
    ///
    /// Limits the tool's current capability to the maximum available supply current.
    fn new(tool_measurement: RawToolMeasurement, supply: Supply) -> Result<Self, Error> {
        let properties = Tool::detect(tool_measurement)?;
        info!("New tool with properties: {:?}", properties);

        let mut tool = Self {
            properties,
            temperature_pid: Pid::new(0.0, 0.0),
            current_pid: Pid::new(0.0, 1.0),
            pwm_ratio: Ratio::new::<percent>(0.0),
            temperature_deg_c: None,
            supply,
            state: ToolState::Active,
        };

        let gain = {
            // Use a scale between 0.3 (fast) and 0.1 (slow).
            const SCALE: f32 = 0.2;
            let gain = SCALE * properties.heater_resistance_ohm / tool.supply.potential_v();
            debug!(
                "Using current loop I-gain of {} (for {} V, {} Ω, scale {})",
                gain,
                tool.supply.potential_v(),
                properties.heater_resistance_ohm,
                SCALE
            );

            gain
        };

        tool.current_pid.i(gain, f32::INFINITY);

        Ok(tool)
    }

    /// Update the tool's state, based on whether it is currently in its stand, and the auto sleep duration.
    fn update_tool_state(&mut self, in_stand: bool, auto_sleep: AutoSleep) -> ToolState {
        if !in_stand {
            self.state = ToolState::Active;
        } else if matches!(self.state, ToolState::Active) {
            debug!("Tool in stand");
            self.state = match auto_sleep {
                AutoSleep::AfterDurationS(0) => ToolState::Sleeping,
                _ => ToolState::InStand(Instant::now()),
            };
        } else if let ToolState::InStand(instant) = self.state {
            self.state = match auto_sleep {
                AutoSleep::Never => self.state,
                AutoSleep::AfterDurationS(duration_s) => {
                    if let Some(passed_duration) = Instant::now().checked_duration_since(instant)
                        && passed_duration >= Duration::from_secs(duration_s as u64)
                    {
                        ToolState::Sleeping
                    } else {
                        self.state
                    }
                }
            };
        }

        self.state
    }

    /// The tool's name.
    fn name(&self) -> &'static str {
        self.properties.name
    }

    /// Calculate the tool's power limit in W, including effective power limit from the supply.
    #[allow(unused)]
    fn power_limit_w(&self) -> f32 {
        self.properties
            .max_power_w
            .min(self.supply.effective_power_limit_w())
    }

    /// Calculate the tool's current limit in Ampere, including effective current limit from the supply.
    ///
    /// Takes into account
    /// - the tool's maximum allowed power,
    /// - the max. supply current, and
    /// - the supply potential.
    fn current_limit_a(&self) -> f32 {
        self.properties
            .max_current_a(self.supply.potential_v())
            .min(self.supply.effective_current_limit_a())
    }

    /// Detect a tool, based on a measurement.
    fn detect(tool_measurement: RawToolMeasurement) -> Result<&'static ToolProperties, Error> {
        const DETECTION_RATIO_ABS_TOLERANCE: f32 = 0.05;

        for tool_properties in TOOLS {
            if (tool_measurement.detect_ratio.get::<ratio::ratio>() - tool_properties.detect_ratio)
                .abs()
                < DETECTION_RATIO_ABS_TOLERANCE
            {
                return Ok(tool_properties);
            }
        }

        Err(Error::UnknownTool)
    }

    /// Calculate tool temperature from a raw tool measurement.
    ///
    /// Checks for unexpected tool changes during the control cycle.
    fn detect_and_calculate_temperature(
        &mut self,
        tool_measurement: RawToolMeasurement,
    ) -> Result<(), Error> {
        let new_properties = Tool::detect(tool_measurement)?;
        if new_properties.name != self.name() {
            return Err(Error::ToolMismatch);
        }

        self.temperature_deg_c = tool_measurement
            .temperature(self.properties)
            .map(|v| v.get::<thermodynamic_temperature::degree_celsius>());

        Ok(())
    }

    /// Runs a temperature control step.
    fn run_temperature_control(
        &mut self,
        set_temperature_deg_c: f32,
    ) -> Result<loeti_protocol::Measurement, Error> {
        let current_limit_a = self.current_limit_a();
        self.temperature_pid.output_limit = current_limit_a;

        if self.temperature_pid.setpoint != set_temperature_deg_c {
            self.temperature_pid.reset_integral_term();
            self.temperature_pid.setpoint(set_temperature_deg_c);
        }

        // Assume 0°C, if the measurement was invalid (e.g. negative thermocouple voltage).
        let control_output = self
            .temperature_pid
            .next_control_output(self.temperature_deg_c.unwrap_or_default());

        let current_setpoint_a = control_output.output;
        self.current_pid.setpoint(current_setpoint_a);

        const UNLIMITED_OUTPUT: f32 = f32::INFINITY;
        self.temperature_pid
            .p(self.properties.p, UNLIMITED_OUTPUT)
            .d(self.properties.d * LOOP_PERIOD_MS as f32, UNLIMITED_OUTPUT);

        // The I-component is capped at the current limit to avoid excessive windup.
        let is_current_limited = current_setpoint_a >= current_limit_a || current_setpoint_a == 0.0;
        if is_current_limited {
            self.temperature_pid.i(0.0, current_limit_a);
        } else {
            self.temperature_pid
                .i(self.properties.i / LOOP_PERIOD_MS as f32, current_limit_a);
        }

        // Mitigate downward setpoint steps to cause undershoot.
        if control_output.output <= 0.0 && control_output.i < 0.0 {
            self.temperature_pid.reset_integral_term();
        }

        trace!(
            "Temperature control, current limited={}: P {}, I {}, D {} => {} A",
            is_current_limited,
            control_output.p,
            control_output.i,
            control_output.d,
            current_setpoint_a
        );

        Ok(loeti_protocol::Measurement {
            time_ms: Instant::now().as_millis(),
            pid: Some(loeti_protocol::Pid {
                output: control_output.output,
                p: control_output.p,
                i: control_output.i,
                d: control_output.d,
            }),
            set_temperature_deg_c: Some(set_temperature_deg_c),
            temperature_deg_c: self.temperature_deg_c,
        })
    }

    /// The ratio of the current setpoint and the maximum current that can be practically supplied.
    ///
    /// This is a measure for relative output power, referred to the tool, not only the supply.
    #[allow(unused)]
    fn power_ratio(&self) -> f32 {
        (self.current_pid.setpoint / self.current_limit_a()).max(0.0)
    }

    /// The PWM ratio to use for driving the heater.
    fn pwm_ratio(&self) -> f32 {
        self.pwm_ratio.get::<ratio::ratio>()
    }

    /// Runs a power control step.
    fn run_current_control(&mut self, power_measurement: &PowerMeasurement) {
        let control_output = self
            .current_pid
            .next_control_output(power_measurement.current.get::<electric_current::ampere>());

        // Mitigate downward setpoint steps to cause undershoot.
        if control_output.output <= 0.0 && control_output.i < 0.0 {
            self.current_pid.reset_integral_term();
        }

        self.pwm_ratio = Ratio::new::<ratio::ratio>(control_output.output);
    }
}

/// Handles the main tool control loop.
///
/// - Detects whether a tool is present
/// - Runs temperature (outer) control loop, while measuring tool temperature
/// - Runs current (inner) control loop, while measuring voltage and current on the tool
async fn control(tool_resources: &mut ToolResources, supply: Supply) -> Result<(), Error> {
    let detect_threshold_ratio = Ratio::new::<ratio::ratio>(MAX_ADC_RATIO);
    let temperature_threshold_potential = ElectricPotential::new::<volt>(MAX_ADC_V);

    const DAC_VOLTAGE: u16 = 2825; // 2.0 V output

    tool_resources
        .dac_current_limit
        .set_mode(dac::Mode::NormalExternalBuffered);
    tool_resources.dac_current_limit.enable();
    tool_resources
        .dac_current_limit
        .set(dac::Value::Bit12Right(DAC_VOLTAGE));

    let pwm_heater_channel: &mut embassy_stm32::timer::simple_pwm::SimplePwmChannel<
        '_,
        peripherals::TIM1,
    > = &mut tool_resources.pwm_heater.ch1();

    let mut tool = None;

    pwm_heater_channel.set_duty_cycle_fully_off();
    pwm_heater_channel.enable();

    // The set temperature of the tool when in use (not in the stand).
    let mut operational_temperature_deg_c = None;
    let idle_power_measurement = measure_tool_power(&mut tool_resources.adc_resources).await;

    // Measure idle current for potential offset compensation.
    debug!(
        "Idle current: {} mA",
        idle_power_measurement
            .current
            .get::<electric_current::milliampere>()
    );

    // Can be taken once, when creating a new tool.
    let mut supply = Some(supply);

    loop {
        // Settling time for temperature ADC filter.
        Timer::after_millis(1).await;

        let persistent = PERSISTENT_MUTEX.lock(|x| *x.borrow());

        let tool_measurement = measure_tool(
            &mut tool_resources.adc_resources,
            detect_threshold_ratio,
            temperature_threshold_potential,
        )
        .await?;

        if tool.is_none() {
            tool = Some(Tool::new(tool_measurement, supply.take().unwrap())?);
        };

        let tool = tool.as_mut().unwrap();

        // Check if tool is in its stand.
        let tool_in_stand = tool_resources.pin_sleep.is_low();
        let tool_state = tool.update_tool_state(tool_in_stand, persistent.auto_sleep);

        let operational_state = OPERATIONAL_STATE_MUTEX.lock(|x| {
            let mut operational_state = x.borrow_mut();
            operational_state.tool_state = Some(tool_state);
            operational_state.tool = Ok(tool.name());

            *operational_state
        });

        tool.supply.current_margin = ElectricCurrent::new::<electric_current::milliampere>(
            persistent.current_margin_ma as f32,
        );

        #[cfg(feature = "display")]
        display_power_limit(Some(tool.power_limit_w()));

        if !operational_state.set_temperature_is_pending {
            operational_temperature_deg_c = Some(persistent.set_temperature_deg_c as f32);
        }
        let stand_temperature_deg_c = Some(
            (persistent.stand_temperature_deg_c as f32)
                .min(operational_temperature_deg_c.unwrap_or(f32::INFINITY)),
        );

        let set_temperature_deg_c = if tool_in_stand {
            stand_temperature_deg_c
        } else {
            operational_temperature_deg_c
        };

        if set_temperature_deg_c.is_none() {
            error!("No set temperature available.");
            continue;
        }

        tool.detect_and_calculate_temperature(tool_measurement)?;

        #[cfg(feature = "display")]
        display_current_temperature(tool.temperature_deg_c);

        #[cfg(feature = "comm")]
        {
            let status = loeti_protocol::Status {
                time_ms: Instant::now().as_millis(),
                control_state: loeti_protocol::ControlState::Tool(match tool_state {
                    ToolState::Active => loeti_protocol::ToolState::Active,
                    ToolState::InStand(instant) => {
                        loeti_protocol::ToolState::InStand(instant.as_millis())
                    }
                    ToolState::Sleeping => loeti_protocol::ToolState::Sleeping,
                }),
            };
            comm::send_status(&status);
        }

        if operational_state.tool_is_off || matches!(tool_state, ToolState::Sleeping) {
            let mut power_measurement = measure_tool_power(&mut tool_resources.adc_resources).await;
            power_measurement.compensate(&idle_power_measurement);

            #[cfg(feature = "display")]
            display_current_power(None);

            // Skip the rest of the control loop.
            Timer::after_millis(LOOP_PERIOD_MS).await;

            #[cfg(feature = "comm")]
            comm::send_measurement(&loeti_protocol::Measurement {
                time_ms: Instant::now().as_millis(),
                temperature_deg_c: tool.temperature_deg_c,
                set_temperature_deg_c,
                ..Default::default()
            });
            continue;
        }

        let _measurement = tool.run_temperature_control(set_temperature_deg_c.unwrap())?;

        #[cfg(feature = "comm")]
        comm::send_measurement(&_measurement);

        let mut current_loop_ticker = Ticker::every(Duration::from_millis(CURRENT_LOOP_PERIOD_MS));
        for _ in 0..CURRENT_CONTROL_CYCLE_COUNT {
            let ratio = tool.pwm_ratio();

            pwm_heater_channel
                .set_duty_cycle((ratio * pwm_heater_channel.max_duty_cycle() as f32) as u16);

            #[cfg(feature = "display")]
            display_current_power(Some(tool.power_ratio()));

            let tool_power_fut = async {
                // Measure current and voltage after the low-pass filter settles - in the middle of the loop period.
                Timer::after_millis(CURRENT_LOOP_PERIOD_MS / 2).await;

                let mut power_measurement =
                    measure_tool_power(&mut tool_resources.adc_resources).await;
                power_measurement.compensate(&idle_power_measurement);

                // Wait for the end of this cycle.
                current_loop_ticker.next().await;
                power_measurement
            };

            match select(
                tool_power_fut,
                tool_resources.exti_current_alert.wait_for_low(),
            )
            .await
            {
                Either::First(power_measurement) => tool.run_current_control(&power_measurement),
                Either::Second(_) => {
                    warn!("Current alert");
                    break;
                }
            };
        }

        pwm_heater_channel.set_duty_cycle_fully_off();
    }
}

/// Control the tool's heating element.
///
/// Takes current and temperature measurements, and adjusts the heater PWM duty cycle accordingly.
#[embassy_executor::task]
pub async fn tool_task(mut tool_resources: ToolResources, negotiated_supply: (u32, u32)) {
    debug!("Maximum measurable voltage: {} V", MAX_ADC_V);
    debug!(
        "Maximum measurable detection resistor ratio: {}",
        MAX_ADC_RATIO
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

        let result = control(&mut tool_resources, supply).await;

        if let Err(error) = result {
            #[cfg(feature = "comm")]
            {
                let status = loeti_protocol::Status {
                    time_ms: Instant::now().as_millis(),
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
