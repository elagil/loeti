//! Drives the tool's heating element, based on target and actual temperature.
use defmt::{info, warn, Format};
use embassy_futures::select::{select, Either};
use embassy_stm32::{adc, dac, exti::ExtiInput, gpio::Input, peripherals, timer::simple_pwm::SimplePwm};
use embassy_time::{Duration, Ticker, Timer};
use micromath::F32Ext;
use pid::{self, Pid};

type PwmPercentage = u8;
type ResistanceOhm = f32;
type TemperatureC = f32;
type Volt = f32;

const ADC_RESOLUTION: adc::Resolution = adc::Resolution::BITS12;
const DETECT_THRESHOLD_V: Volt = 0.1;

#[derive(Debug, Format)]
enum Error {
    NoTool,
    UnknownTool,
    ToolMismatch,
}

fn adc_value_to_v(value: u16) -> Volt {
    (adc::VREF_DEFAULT_MV as f32) * (value as f32) / (adc::resolution_to_max_count(ADC_RESOLUTION) as f32) / 1000.0
}

/// Resources for the tool ADC, for temperature measurement and tool detection.
pub struct AdcToolResources {
    /// The ADC for temperature measurements.
    pub adc_temp: adc::Adc<'static, peripherals::ADC2>,
    /// The ADC temperature input pin.
    pub adc_pin_temperature: adc::AnyAdcChannel<peripherals::ADC2>,
    /// The ADC detection input pin.
    pub adc_pin_detect: adc::AnyAdcChannel<peripherals::ADC2>,
    /// The DMA for the temperature ADC.
    pub adc_temperature_dma: peripherals::DMA1_CH4,
}

/// Resources for the power ADC, for tool voltage and current measurement.
pub struct AdcPowerResources {
    /// The ADC for power (voltage and current) measurements.
    pub adc_power: adc::Adc<'static, peripherals::ADC1>,
    /// The ADC input for voltage on the bus.
    pub adc_pin_voltage: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The ADC input for heater current.
    pub adc_pin_current: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The DMA for the power ADC.
    pub adc_power_dma: peripherals::DMA1_CH6,
}

/// Resources for driving the tool's heater and taking associated measurements.
pub struct ToolResources {
    /// ADC for tool detection and temperature.
    pub adc_tool_resources: AdcToolResources,

    /// ADC for tool voltage and current.
    pub adc_power_resources: AdcPowerResources,

    /// The DAC that sets the current limit for the current sensing IC (INA301A).
    pub dac_current_limit: dac::DacChannel<'static, peripherals::DAC1, 1, peripherals::DMA1_CH5>,

    /// External interrupt for current alerts.
    pub exti_current_alert: ExtiInput<'static>,

    /// The PWM for driving the tool's heating element.
    pub pwm_heater: SimplePwm<'static, peripherals::TIM1>,

    /// A pin for detecting the tool sleep position (in holder).
    pub pin_sleep: Input<'static>,
}

#[derive(Clone, Copy)]
struct ToolMeasurement {
    detect_v: Volt,
    temperature_v: Volt,
}

async fn measure_tool(adc_tool_resources: &mut AdcToolResources) -> Result<ToolMeasurement, Error> {
    const SAMPLE_TIME: adc::SampleTime = adc::SampleTime::CYCLES47_5;
    let mut adc_buffer = [0u16; 2];

    adc_tool_resources
        .adc_temp
        .read(
            &mut adc_tool_resources.adc_temperature_dma,
            [
                (&mut adc_tool_resources.adc_pin_detect, SAMPLE_TIME),
                (&mut adc_tool_resources.adc_pin_temperature, SAMPLE_TIME),
            ]
            .into_iter(),
            &mut adc_buffer,
        )
        .await;

    let detect_v = adc_value_to_v(adc_buffer[0]);
    let temperature_v = adc_value_to_v(adc_buffer[1]);

    if detect_v < 3.0 {
        Ok(ToolMeasurement {
            detect_v,
            temperature_v,
        })
    } else {
        Err(Error::NoTool)
    }
}

struct PowerMeasurement {
    current_a: f32,
    voltage_v: f32,
}

impl PowerMeasurement {
    fn power_w(&self) -> f32 {
        self.voltage_v * self.current_a
    }
}

async fn measure_power(adc_power_resources: &mut AdcPowerResources) -> PowerMeasurement {
    const SAMPLE_TIME: adc::SampleTime = adc::SampleTime::CYCLES47_5;
    let mut adc_buffer = [0u16; 2];

    adc_power_resources
        .adc_power
        .read(
            &mut adc_power_resources.adc_power_dma,
            [
                (&mut adc_power_resources.adc_pin_current, SAMPLE_TIME),
                (&mut adc_power_resources.adc_pin_voltage, SAMPLE_TIME),
            ]
            .into_iter(),
            &mut adc_buffer,
        )
        .await;

    const CURRENT_SENSE_TRANSIMPEDANCE: f32 = 5.0;
    let current_a = CURRENT_SENSE_TRANSIMPEDANCE * adc_value_to_v(adc_buffer[0]);

    const VOLTAGE_DIVIDER_RATIO: f32 = 11.0;
    let voltage_v = VOLTAGE_DIVIDER_RATIO * adc_value_to_v(adc_buffer[1]);

    PowerMeasurement { current_a, voltage_v }
}

#[derive(Clone, Copy, PartialEq)]
enum ToolType {
    C210,
    C245,
}

#[derive(Clone, Copy)]
struct ToolProperties {
    tool_type: ToolType,
    max_current_a: f32,
    resistance_ohm: f32,
    detect_v: Volt,
    temperature_scale_c_per_v: f32,

    p: f32,
    i: f32,
    d: f32,
}

const C210_PROPERTIES: ToolProperties = ToolProperties {
    tool_type: ToolType::C210,
    max_current_a: 3.0,
    resistance_ohm: 3.0,
    detect_v: 1.0,
    temperature_scale_c_per_v: 0.1333,

    p: 0.025,
    i: 0.005,
    d: 0.0,
};

const C245_PROPERTIES: ToolProperties = ToolProperties {
    tool_type: ToolType::C245,
    max_current_a: 6.0,
    resistance_ohm: 2.5,
    detect_v: 2.0,
    temperature_scale_c_per_v: 0.1333,

    p: 0.2,
    i: 0.005,
    d: 0.2,
};

const ALL_TOOL_PROPERTIES: [ToolProperties; 2] = [C210_PROPERTIES, C245_PROPERTIES];

impl From<ToolType> for ToolProperties {
    fn from(value: ToolType) -> Self {
        match value {
            ToolType::C210 => C210_PROPERTIES,
            ToolType::C245 => C245_PROPERTIES,
        }
    }
}

struct Tool {
    tool_properties: ToolProperties,
    temperature_pid: Pid<f32>,
    current_pid: Pid<f32>,
    pwm_percentage: PwmPercentage,
}

impl Tool {
    pub fn new(tool_measurement: ToolMeasurement) -> Result<Self, Error> {
        let tool_properties = Tool::detect(tool_measurement)?;

        let mut tool = Self {
            tool_properties,
            temperature_pid: Pid::new(0.0, tool_properties.max_current_a),
            current_pid: Pid::new(0.0, 100.0),
            pwm_percentage: 0,
        };

        tool.temperature_pid
            .p(tool_properties.p, tool_properties.max_current_a)
            .i(tool_properties.i, tool_properties.max_current_a)
            .d(tool_properties.d, tool_properties.max_current_a);

        tool.current_pid.i(0.5 * tool_properties.resistance_ohm / 2.0, 100.0);

        Ok(tool)
    }

    fn detect(tool_measurement: ToolMeasurement) -> Result<ToolProperties, Error> {
        for tool_properties in ALL_TOOL_PROPERTIES {
            if (tool_measurement.detect_v - tool_properties.detect_v).abs() < DETECT_THRESHOLD_V {
                return Ok(tool_properties);
            }
        }

        Err(Error::UnknownTool)
    }

    fn update_temperature(&mut self, tool_measurement: ToolMeasurement) -> Result<(), Error> {
        let tool_properties = Tool::detect(tool_measurement)?;
        if tool_properties.tool_type != self.tool_properties.tool_type {
            return Err(Error::ToolMismatch);
        }

        self.current_pid.setpoint(
            self.temperature_pid
                .next_control_output(tool_measurement.temperature_v * self.tool_properties.temperature_scale_c_per_v)
                .output,
        );

        Ok(())
    }

    fn update_power(&mut self, tool_power: PowerMeasurement) {
        self.pwm_percentage = self.current_pid.next_control_output(tool_power.current_a).output as u8;
    }
}

async fn control(tool_resources: &mut ToolResources) -> Result<(), Error> {
    const CURRENT_CONTROL_CYCLE_COUNT: usize = 10;
    const LOOP_TIME_MS: u64 = 100;
    const CURRENT_LOOP_PERIOD_MS: u64 = LOOP_TIME_MS / 10;

    const DAC_VOLTAGE: u16 = 123;
    tool_resources
        .dac_current_limit
        .set(dac::Value::Bit12Right(DAC_VOLTAGE));

    let pwm_heater_channel = &mut tool_resources.pwm_heater.ch4();
    let mut tool: Option<Tool> = None;

    loop {
        // Wait for temperature sensor value to settle after disabling the heating element.
        Timer::after_millis(1).await;
        let tool_measurement = measure_tool(&mut tool_resources.adc_tool_resources).await?;

        if tool.is_none() {
            tool = Some(Tool::new(tool_measurement)?)
        }

        let tool = tool.as_mut().unwrap();
        tool.update_temperature(tool_measurement)?;

        pwm_heater_channel.set_duty_cycle_fully_off();
        pwm_heater_channel.enable();

        let mut current_loop_ticker = Ticker::every(Duration::from_millis(CURRENT_LOOP_PERIOD_MS));
        for _ in 0..CURRENT_CONTROL_CYCLE_COUNT {
            pwm_heater_channel.set_duty_cycle_percent(tool.pwm_percentage);

            let tool_power_fut = async {
                // Measure current and voltage after the low-pass filter settles - in the middle of the loop period.
                Timer::after_millis(CURRENT_LOOP_PERIOD_MS / 2).await;

                let tool_power = measure_power(&mut tool_resources.adc_power_resources).await;

                // Wait for the end of this cycle.
                current_loop_ticker.next().await;

                tool_power
            };

            match select(tool_power_fut, tool_resources.exti_current_alert.wait_for_low()).await {
                Either::First(tool_power) => tool.update_power(tool_power),
                Either::Second(_) => {
                    // Break and switch off heater on current alert.
                    break;
                }
            };
        }

        pwm_heater_channel.disable();
    }
}

/// Control the tool's heating element.
///
/// Takes current and temperature measurements, and adjusts the heater PWM duty cycle accordingly.
#[embassy_executor::task]
pub async fn tool_task(mut tool_resources: ToolResources) {
    loop {
        let result = control(&mut tool_resources).await;

        if result.is_err() {
            warn!("Tool error: {}", result);
            Timer::after_millis(500).await
        }
    }
}
