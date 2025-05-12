//! Drives the tool's heating element, based on target and actual temperature.

use defmt::{debug, error, info, warn, Format};
use embassy_futures::select::{select, Either};
use embassy_stm32::dac::Ch1;
use embassy_stm32::mode::Blocking;
use embassy_stm32::Peri;
use embassy_stm32::{adc, dac, exti::ExtiInput, gpio::Input, peripherals, timer::simple_pwm::SimplePwm};
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
    MAX_SUPPLY_CURRENT_MA_SIG, PERSISTENT, POWER_BARGRAPH_SIG, POWER_MEASUREMENT_W_SIG,
    TEMPERATURE_MEASUREMENT_DEG_C_SIG, TOOL_NAME_SIG,
};

const ADC_RESOLUTION: adc::Resolution = adc::Resolution::BITS12;
const CURRENT_CONTROL_CYCLE_COUNT: u64 = 5;
const LOOP_PERIOD_MS: u64 = 100;
const CURRENT_LOOP_PERIOD_MS: u64 = LOOP_PERIOD_MS / CURRENT_CONTROL_CYCLE_COUNT;

const VREFBUF_V: f32 = 2.9;
const SUPPLY_V: f32 = 3.3;
const MAX_ADC_V: f32 = VREFBUF_V - 0.1;
const MAX_ADC_RATIO: f32 = MAX_ADC_V / SUPPLY_V;

#[derive(Debug, Format)]
enum Error {
    NoTool,
    NoTip,
    UnknownTool,
    ToolMismatch,
}

fn adc_value_to_potential(value: u16) -> ElectricPotential {
    ElectricPotential::new::<volt>(VREFBUF_V * (value as f32) / (adc::resolution_to_max_count(ADC_RESOLUTION) as f32))
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

#[derive(Clone, Copy)]
struct ToolMeasurement {
    detect_ratio: Ratio,
    temperature_potential: ElectricPotential,
}

impl ToolMeasurement {
    fn temperature(&self, tool_properties: &ToolProperties) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(
            tool_properties
                .temperature_calibration
                .calc_temperature_c(self.temperature_potential.get::<volt>()),
        )
    }
}

async fn measure_tool(
    adc_resources: &mut AdcResources,
    detect_ratio_threshold: Ratio,
    temperature_potential_threshold: ElectricPotential,
) -> Result<ToolMeasurement, Error> {
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

    let detect_ratio = adc_value_to_potential(adc_buffer[0]) / ElectricPotential::new::<electric_potential::volt>(3.3);
    let temperature_potential = adc_value_to_potential(adc_buffer[1]);

    if detect_ratio > detect_ratio_threshold {
        Err(Error::NoTool)
    } else if temperature_potential > temperature_potential_threshold {
        Err(Error::NoTip)
    } else {
        Ok(ToolMeasurement {
            detect_ratio,
            temperature_potential,
        })
    }
}

struct PowerMeasurement {
    current: ElectricCurrent,
    potential: ElectricPotential,
}

impl PowerMeasurement {
    fn power(&self) -> Power {
        self.potential * self.current
    }
}

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

    // info!(
    //     "{} mA, {} mV",
    //     current.get::<electric_current::milliampere>(),
    //     potential.get::<electric_potential::millivolt>()
    // );
    let measurement = PowerMeasurement { current, potential };

    POWER_MEASUREMENT_W_SIG.signal(measurement.power().get::<power::watt>());
    measurement
}

struct Tool {
    tool_properties: &'static ToolProperties,
    max_supply_current_a: f32,
    temperature_pid: Pid<f32>,
    current_pid: Pid<f32>,
    pwm_ratio: Ratio,
}

impl Tool {
    pub fn new(tool_measurement: ToolMeasurement, max_supply_current: &ElectricCurrent) -> Result<Self, Error> {
        let tool_properties = Tool::detect(tool_measurement)?;

        let max_supply_current_a = max_supply_current.get::<electric_current::ampere>();
        let max_current_a = tool_properties.max_current_a.min(max_supply_current_a);

        let mut tool = Self {
            tool_properties,
            max_supply_current_a,
            temperature_pid: Pid::new(300.0, max_current_a),
            current_pid: Pid::new(0.0, 1.0),
            pwm_ratio: Ratio::new::<percent>(0.0),
        };

        tool.current_pid.i(
            //  TODO: explain formula
            0.25 * tool_properties.heater_resistance_ohm / (CURRENT_LOOP_PERIOD_MS as f32),
            1.0,
        );
        tool.setup_temperature_pid(false);

        Ok(tool)
    }

    fn setup_temperature_pid(&mut self, is_current_limited: bool) {
        let max_current_a = self.temperature_pid.output_limit;
        self.temperature_pid
            .p(self.tool_properties.p, max_current_a)
            .d(self.tool_properties.d, max_current_a);

        if is_current_limited {
            self.temperature_pid.i(0.0, max_current_a);
        } else {
            self.temperature_pid
                .i(self.tool_properties.i / LOOP_PERIOD_MS as f32, max_current_a);
        }
    }

    fn detect(tool_measurement: ToolMeasurement) -> Result<&'static ToolProperties, Error> {
        for tool_properties in ToolProperties::all() {
            if (tool_measurement.detect_ratio.get::<ratio::ratio>() - tool_properties.detect_ratio).abs() < 0.05 {
                TOOL_NAME_SIG.signal(tool_properties.name);
                return Ok(tool_properties);
            }
        }

        TOOL_NAME_SIG.signal("Unknown tool");
        Err(Error::UnknownTool)
    }

    fn control_temperature(
        &mut self,
        tool_measurement: ToolMeasurement,
        set_temperature_degree_c: f32,
    ) -> Result<f32, Error> {
        let tool_properties = Tool::detect(tool_measurement)?;
        if tool_properties.tool_type != self.tool_properties.tool_type {
            return Err(Error::ToolMismatch);
        }

        let tool_temperature_deg_c = tool_measurement
            .temperature(self.tool_properties)
            .get::<thermodynamic_temperature::degree_celsius>();

        self.temperature_pid.setpoint(set_temperature_degree_c);
        let control_output = self.temperature_pid.next_control_output(tool_temperature_deg_c);

        let current_setpoint_a = control_output.output;
        self.current_pid.setpoint(current_setpoint_a);

        let is_current_limited =
            current_setpoint_a.abs() == self.tool_properties.max_current_a.min(self.max_supply_current_a);
        self.setup_temperature_pid(is_current_limited);

        POWER_BARGRAPH_SIG.signal(current_setpoint_a / self.max_supply_current_a);

        Ok(tool_temperature_deg_c)
    }

    fn control_power(&mut self, power_measurement: PowerMeasurement) {
        let measured_current_a = power_measurement.current.get::<electric_current::ampere>();
        let output = self.current_pid.next_control_output(measured_current_a).output.max(0.0);

        self.pwm_ratio = Ratio::new::<ratio::ratio>(output);
    }
}

async fn control(tool_resources: &mut ToolResources, max_supply_current: &ElectricCurrent) -> Result<(), Error> {
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

    let pwm_heater_channel: &mut embassy_stm32::timer::simple_pwm::SimplePwmChannel<'_, peripherals::TIM1> =
        &mut tool_resources.pwm_heater.ch1();
    let mut tool: Option<Tool> = None;

    pwm_heater_channel.set_duty_cycle_fully_off();
    pwm_heater_channel.enable();

    let mut set_temperature_deg_c = None;

    loop {
        // Wait for temperature sensor value to settle after disabling the heating element.
        Timer::after_millis(10).await;
        let tool_measurement = measure_tool(
            &mut tool_resources.adc_resources,
            detect_threshold_ratio,
            temperature_threshold_potential,
        )
        .await?;

        if tool.is_none() {
            tool = Some(Tool::new(tool_measurement, max_supply_current)?)
        }

        let tool = tool.as_mut().unwrap();

        PERSISTENT.lock(|x| {
            let persistent = x.borrow();
            if !persistent.set_temperature_pending {
                set_temperature_deg_c = Some(persistent.set_temperature_deg_c as f32);
            }
        });

        if set_temperature_deg_c.is_none() {
            error!("No set temperature available.");
            continue;
        }

        let tool_temperature_deg_c = tool.control_temperature(tool_measurement, set_temperature_deg_c.unwrap())?;
        TEMPERATURE_MEASUREMENT_DEG_C_SIG.signal(tool_temperature_deg_c);

        let mut current_loop_ticker = Ticker::every(Duration::from_millis(CURRENT_LOOP_PERIOD_MS));
        for _ in 0..CURRENT_CONTROL_CYCLE_COUNT {
            let ratio = tool.pwm_ratio.get::<ratio::ratio>().max(0.0);

            pwm_heater_channel.set_duty_cycle((ratio * pwm_heater_channel.max_duty_cycle() as f32) as u16);

            let tool_power_fut = async {
                // Measure current and voltage after the low-pass filter settles - in the middle of the loop period.
                Timer::after_millis(CURRENT_LOOP_PERIOD_MS / 2).await;

                let power_measurement = measure_tool_power(&mut tool_resources.adc_resources).await;

                // Wait for the end of this cycle.
                current_loop_ticker.next().await;
                power_measurement
            };

            match select(tool_power_fut, tool_resources.exti_current_alert.wait_for_low()).await {
                Either::First(power_measurement) => tool.control_power(power_measurement),
                Either::Second(_) => {
                    warn!("Current alert");
                    break;
                }
            };
        }

        pwm_heater_channel.set_duty_cycle_fully_off();
    }
}

fn display_state(message: &'static str) {
    POWER_MEASUREMENT_W_SIG.signal(f32::NAN);
    POWER_BARGRAPH_SIG.signal(f32::NAN);
    TEMPERATURE_MEASUREMENT_DEG_C_SIG.signal(f32::NAN);
    TOOL_NAME_SIG.signal(message);
}

/// Control the tool's heating element.
///
/// Takes current and temperature measurements, and adjusts the heater PWM duty cycle accordingly.
#[embassy_executor::task]
pub async fn tool_task(mut tool_resources: ToolResources) {
    info!("Maximum measurable voltage: {} V", MAX_ADC_V);
    info!("Maximum measurable detection resistor ratio: {}", MAX_ADC_RATIO);
    display_state("Negotiating...");

    // Some margin to prevent over current.
    let current_ma = MAX_SUPPLY_CURRENT_MA_SIG.wait().await - 100.0;
    let current = ElectricCurrent::new::<electric_current::milliampere>(current_ma);

    info!("Current limit: {} mA", current_ma);

    loop {
        let result = control(&mut tool_resources, &current).await;

        if result.is_err() {
            display_state("No tool");
            debug!("Tool control error: {}", result);
            Timer::after_millis(100).await
        }
    }
}
