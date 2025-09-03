//! Drives the tool's heating element, based on target and actual temperature.

use defmt::{debug, error, info, warn, Format};
use embassy_futures::select::{select, Either};
use embassy_stm32::dac::Ch1;
use embassy_stm32::mode::Blocking;
use embassy_stm32::Peri;
use embassy_stm32::{
    adc, dac, exti::ExtiInput, gpio::Input, peripherals, timer::simple_pwm::SimplePwm,
};
use embassy_time::{Duration, Ticker, Timer};
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
use library::ToolProperties;

use crate::{
    DISPLAY_POWER_SIG, MESSAGE_SIG, OPERATIONAL_STATE_MUTEX, PERSISTENT_MUTEX,
    POWER_RATIO_BARGRAPH_SIG, TEMPERATURE_MEASUREMENT_DEG_C_SIG,
};

/// ADC resolution in bit.
const ADC_RESOLUTION: adc::Resolution = adc::Resolution::BITS12;
/// The number of current control iterations per temperature control iteration.
const CURRENT_CONTROL_CYCLE_COUNT: u64 = 5;
/// The total loop period in ms (temperature loop).
const LOOP_PERIOD_MS: u64 = 100;
/// The current loop period in ms.
const CURRENT_LOOP_PERIOD_MS: u64 = LOOP_PERIOD_MS / CURRENT_CONTROL_CYCLE_COUNT;

/// The ADC reference voltage.
const VREFBUF_V: f32 = 2.9;
/// The analog supply voltage.
const SUPPLY_V: f32 = 3.3;
/// The value at which an ADC voltage is considered to be at the upper limit.
const MAX_ADC_V: f32 = VREFBUF_V - 0.1;
/// The ratio between the defined maximum ADC voltage and analog supply voltage.
const MAX_ADC_RATIO: f32 = MAX_ADC_V / SUPPLY_V;

/// Errors during tool detection.
#[derive(Debug, Format)]
enum Error {
    /// No tool was found.
    NoTool,
    /// Tool was detected, but no tip.
    NoTip,
    /// The detected tool is unknown.
    UnknownTool,
    /// Tool type mismatch during control loop operation.
    ToolMismatch,
}

fn adc_value_to_potential(value: u16) -> ElectricPotential {
    ElectricPotential::new::<volt>(
        VREFBUF_V * (value as f32) / (adc::resolution_to_max_count(ADC_RESOLUTION) as f32),
    )
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
    fn temperature(&self, tool_properties: &ToolProperties) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(
            tool_properties
                .temperature_calibration
                .calc_temperature_c(self.temperature_potential.get::<volt>()),
        )
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
    const SAMPLE_TIME: adc::SampleTime = adc::SampleTime::CYCLES247_5;
    let mut adc_buffer = [0u16; 2];

    adc_resources
        .adc
        .read(
            adc_resources.adc_dma.reborrow(),
            [
                (&mut adc_resources.pin_detect, SAMPLE_TIME),
                (&mut adc_resources.pin_temperature, SAMPLE_TIME),
            ]
            .into_iter(),
            &mut adc_buffer,
        )
        .await;

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
    _potential: ElectricPotential,
}

impl PowerMeasurement {
    /// Calculate the tool's power dissipation.
    fn _power(&self) -> Power {
        self._potential * self.current
    }

    /// Compensate current with an idle power measurement.
    fn compensate(&mut self, idle: &Self) {
        self.current -= idle.current;
    }
}

/// Measure the tool's power (voltage and current).
async fn measure_tool_power(adc_power_resources: &mut AdcResources) -> PowerMeasurement {
    const SAMPLE_TIME: adc::SampleTime = adc::SampleTime::CYCLES247_5;
    let mut adc_buffer = [0u16; 2];

    adc_power_resources
        .adc
        .read(
            adc_power_resources.adc_dma.reborrow(),
            [
                (&mut adc_power_resources.pin_current, SAMPLE_TIME),
                (&mut adc_power_resources.pin_voltage, SAMPLE_TIME),
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

/// A tool (soldering iron).
struct Tool {
    /// Unique properties of the tool.
    tool_properties: &'static ToolProperties,
    /// The maximum allowed supply current, regardless of tool capabilities.
    supply_current_limit_a: f32,
    /// The temperature control.
    temperature_pid: Pid<f32>,
    /// The current control.
    current_pid: Pid<f32>,
    /// The current  PWM ratio of the heater switch (MOSFET).
    pwm_ratio: Ratio,
}

impl Tool {
    /// Create a new tool from a measurement.
    ///
    /// Limits the tool's current capability to the maximum available supply current.
    pub fn new(
        tool_measurement: RawToolMeasurement,
        supply_current_limit: &ElectricCurrent,
    ) -> Result<Self, Error> {
        let tool_properties = Tool::detect(tool_measurement)?;

        let supply_current_limit_a = supply_current_limit.get::<electric_current::ampere>();
        let current_limit_a = tool_properties.max_current_a.min(supply_current_limit_a);

        let mut tool = Self {
            tool_properties,
            supply_current_limit_a,
            temperature_pid: Pid::new(300.0, current_limit_a),
            current_pid: Pid::new(0.0, 1.0),
            pwm_ratio: Ratio::new::<percent>(0.0),
        };

        tool.current_pid.i(
            //  TODO: explain formula
            0.25 * tool_properties.heater_resistance_ohm / (CURRENT_LOOP_PERIOD_MS as f32),
            1.0,
        );
        tool.configure_temperature_control(false);

        Ok(tool)
    }

    /// Set a new current limit for the tool.
    pub fn set_current_limit(&mut self, current_limit: &ElectricCurrent) {
        self.temperature_pid.output_limit = self
            .tool_properties
            .max_current_a
            .min(current_limit.get::<electric_current::ampere>());
    }

    /// Configures the temperaure control.
    ///
    /// If the device is current limited, disable the PID's I-component temporarily (prevent windup).
    fn configure_temperature_control(&mut self, is_current_limited: bool) {
        let max_current_a = self.temperature_pid.output_limit;
        self.temperature_pid
            .p(self.tool_properties.p, max_current_a)
            .d(self.tool_properties.d, max_current_a);

        if is_current_limited {
            self.temperature_pid.i(0.0, max_current_a);
        } else {
            self.temperature_pid.i(
                self.tool_properties.i / LOOP_PERIOD_MS as f32,
                max_current_a,
            );
        }
    }

    /// Detect a tool, based on a measurement.
    fn detect(tool_measurement: RawToolMeasurement) -> Result<&'static ToolProperties, Error> {
        for tool_properties in ToolProperties::all() {
            if (tool_measurement.detect_ratio.get::<ratio::ratio>() - tool_properties.detect_ratio)
                .abs()
                < 0.05
            {
                show_message(tool_properties.name);

                return Ok(tool_properties);
            }
        }

        Err(Error::UnknownTool)
    }

    /// Calculate tool temperature from a raw tool measurement.
    fn calculate_temperature(&self, tool_measurement: RawToolMeasurement) -> Result<f32, Error> {
        let tool_properties = Tool::detect(tool_measurement)?;
        if tool_properties.tool_type != self.tool_properties.tool_type {
            return Err(Error::ToolMismatch);
        }

        Ok(tool_measurement
            .temperature(self.tool_properties)
            .get::<thermodynamic_temperature::degree_celsius>())
    }

    /// Runs a temperature control step.
    fn control_temperature(
        &mut self,
        tool_temperature_deg_c: f32,
        set_temperature_deg_c: f32,
    ) -> Result<f32, Error> {
        self.temperature_pid.setpoint(set_temperature_deg_c);

        let current_setpoint_a = self
            .temperature_pid
            .next_control_output(tool_temperature_deg_c)
            .output;

        self.current_pid.setpoint(current_setpoint_a);

        let is_current_limited = current_setpoint_a.abs()
            == self
                .tool_properties
                .max_current_a
                .min(self.supply_current_limit_a);
        self.configure_temperature_control(is_current_limited);

        Ok(tool_temperature_deg_c)
    }

    /// The ratio of the current setpoint and the maximum current that can be supplied.
    ///
    /// This is a measure for relative output power.
    fn power_ratio(&self) -> f32 {
        self.current_pid.setpoint / self.supply_current_limit_a
    }

    /// Runs a power control step.
    fn control_power(&mut self, power_measurement: &PowerMeasurement) {
        let output = self
            .current_pid
            .next_control_output(power_measurement.current.get::<electric_current::ampere>())
            .output
            .max(0.0);

        self.pwm_ratio = Ratio::new::<ratio::ratio>(output);
    }
}

/// Handles the main tool control loop.
///
/// - Detects whether a tool is present
/// - Runs temperature (outer) control loop, while measuring tool temperature
/// - Runs current (inner) control loop, while measuring voltage and current on the tool
async fn control(
    tool_resources: &mut ToolResources,
    max_supply_current: &ElectricCurrent,
) -> Result<(), Error> {
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
    let mut tool: Option<Tool> = None;

    pwm_heater_channel.set_duty_cycle_fully_off();
    pwm_heater_channel.enable();

    let mut set_temperature_deg_c = None;

    loop {
        let persistent = PERSISTENT_MUTEX.lock(|x| *x.borrow());
        let operational_state = OPERATIONAL_STATE_MUTEX.lock(|x| *x.borrow());
        let current_limit = *max_supply_current
            - ElectricCurrent::new::<electric_current::milliampere>(
                persistent.current_margin_ma as f32,
            );

        // Wait for temperature sensor value to settle after disabling the heating element.
        Timer::after_millis(10).await;

        // Measure idle current for potential offset compensation.
        let idle_power_measurement = measure_tool_power(&mut tool_resources.adc_resources).await;

        let tool_measurement = measure_tool(
            &mut tool_resources.adc_resources,
            detect_threshold_ratio,
            temperature_threshold_potential,
        )
        .await?;

        if tool.is_none() {
            tool = Some(Tool::new(tool_measurement, &current_limit)?);
        }

        let tool = tool.as_mut().unwrap();
        tool.set_current_limit(&current_limit);

        if !operational_state.set_temperature_is_pending {
            PERSISTENT_MUTEX.lock(|x| {
                set_temperature_deg_c = Some(x.borrow().set_temperature_deg_c as f32);
            });
        }

        if set_temperature_deg_c.is_none() {
            error!("No set temperature available.");
            continue;
        }

        let tool_temperature_deg_c = tool.calculate_temperature(tool_measurement)?;
        TEMPERATURE_MEASUREMENT_DEG_C_SIG.signal(tool_temperature_deg_c);

        if operational_state.is_sleeping {
            let mut power_measurement = measure_tool_power(&mut tool_resources.adc_resources).await;
            power_measurement.compensate(&idle_power_measurement);
            show_power(None);

            // Skip the rest of the control loop.
            Timer::after_millis(LOOP_PERIOD_MS).await;
            continue;
        }

        tool.control_temperature(tool_temperature_deg_c, set_temperature_deg_c.unwrap())?;

        let mut current_loop_ticker = Ticker::every(Duration::from_millis(CURRENT_LOOP_PERIOD_MS));
        for _ in 0..CURRENT_CONTROL_CYCLE_COUNT {
            let ratio = tool.pwm_ratio.get::<ratio::ratio>().max(0.0);

            pwm_heater_channel
                .set_duty_cycle((ratio * pwm_heater_channel.max_duty_cycle() as f32) as u16);

            let tool_power_fut = async {
                // Measure current and voltage after the low-pass filter settles - in the middle of the loop period.
                Timer::after_millis(CURRENT_LOOP_PERIOD_MS / 2).await;

                let mut power_measurement =
                    measure_tool_power(&mut tool_resources.adc_resources).await;
                power_measurement.compensate(&idle_power_measurement);
                show_power(Some(tool.power_ratio()));

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
                Either::First(power_measurement) => tool.control_power(&power_measurement),
                Either::Second(_) => {
                    warn!("Current alert");
                    break;
                }
            };
        }

        pwm_heater_channel.set_duty_cycle_fully_off();
    }
}

/// Display a power measurement and relative power bargraph.
fn show_power(power_ratio: Option<f32>) {
    let power_ratio = match power_ratio {
        None => f32::NAN,
        Some(x) => x,
    };
    POWER_RATIO_BARGRAPH_SIG.signal(power_ratio)
}

/// Displays a message.
fn show_message(message: &'static str) {
    MESSAGE_SIG.signal(message);
}

/// Displays a message, while being idle (not heating).
fn show_idle_message(message: &'static str) {
    POWER_RATIO_BARGRAPH_SIG.signal(f32::NAN);
    TEMPERATURE_MEASUREMENT_DEG_C_SIG.signal(f32::NAN);

    MESSAGE_SIG.signal(message);
}

/// Control the tool's heating element.
///
/// Takes current and temperature measurements, and adjusts the heater PWM duty cycle accordingly.
#[embassy_executor::task]
pub async fn tool_task(mut tool_resources: ToolResources, negotiated_supply: (u32, u32)) {
    info!("Maximum measurable voltage: {} V", MAX_ADC_V);
    info!(
        "Maximum measurable detection resistor ratio: {}",
        MAX_ADC_RATIO
    );

    let negotiated_potential =
        ElectricPotential::new::<electric_potential::millivolt>(negotiated_supply.0 as f32);
    let negotiated_current =
        ElectricCurrent::new::<electric_current::milliampere>(negotiated_supply.1 as f32);

    DISPLAY_POWER_SIG.signal((negotiated_potential * negotiated_current).get::<power::watt>());

    loop {
        let result = control(&mut tool_resources, &negotiated_current).await;

        if let Err(error) = result {
            match error {
                Error::NoTool => show_idle_message("No tool"),
                Error::NoTip => show_idle_message("No tip"),
                Error::UnknownTool => show_idle_message("Unknown"),
                Error::ToolMismatch => show_idle_message("Mismatch"),
            }

            let sleep_on_error = PERSISTENT_MUTEX.lock(|x| x.borrow().sleep_on_error);
            OPERATIONAL_STATE_MUTEX.lock(|x| x.borrow_mut().is_sleeping = sleep_on_error);

            debug!("Tool control error: {}", error);
            Timer::after_millis(100).await
        }
    }
}
