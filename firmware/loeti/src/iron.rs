//! Drives the iron's heater element, based on target and actual temperature.
use defmt::info;
use embassy_stm32::{adc, dac, exti::ExtiInput, gpio::Input, peripherals, timer::simple_pwm::SimplePwm};
use embassy_time::{Duration, Ticker};
use pid::{self, Pid};

/// Resources for driving the iron's heater and taking associated measurements.
pub struct IronResources {
    /// The ADC for temperature measurements.
    pub adc_temp: adc::Adc<'static, peripherals::ADC2>,
    /// The first ADC input pin (used for C245 irons).
    pub adc_pin_temp_a: adc::AnyAdcChannel<peripherals::ADC2>,
    /// The second ADC input pin (used for C210 irons).
    pub adc_pin_temp_b: adc::AnyAdcChannel<peripherals::ADC2>,
    /// The DMA for the temperature ADC.
    pub adc_temp_dma: peripherals::DMA1_CH4,

    /// The ADC for power (voltage and current) measurements.
    pub adc_power: adc::Adc<'static, peripherals::ADC1>,
    /// The ADC input for voltage on the bus.
    pub adc_pin_voltage: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The ADC input for heater current.
    pub adc_pin_current: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The DMA for the power ADC.
    pub adc_power_dma: peripherals::DMA1_CH6,

    /// The DAC that sets the current limit for the current sensing IC (INA301).
    pub dac_current_limit: dac::DacChannel<'static, peripherals::DAC1, 1, peripherals::DMA1_CH5>,

    /// External interrupt for current alerts.
    pub exti_current_alert: ExtiInput<'static>,

    /// The PWM for driving the iron's heater element.
    pub pwm_heater: SimplePwm<'static, peripherals::TIM2>,

    /// A pin for detecting the iron sleep position (in holder).
    pub pin_sleep: Input<'static>,
}

/// Control the iron's heater element.
///
/// Takes voltage and current measurements, as well as temperature readings, and adjusts the heater PWM accordingly.
#[embassy_executor::task]
pub async fn iron_task(mut iron_resources: IronResources) {
    let _pwm_heater_channel = iron_resources.pwm_heater.ch4();

    const DAC_VOLTAGE: u16 = 123;
    iron_resources
        .dac_current_limit
        .set(dac::Value::Bit12Right(DAC_VOLTAGE));

    const SAMPLE_TIME: adc::SampleTime = adc::SampleTime::CYCLES47_5;
    let mut adc_buffer = [0u16; 2];
    let mut ticker = Ticker::every(Duration::from_hz(1));

    let mut current_pid: Pid<f32> = Pid::new(0.0, 1.0);
    current_pid.p(10.0, 1.0);

    loop {
        ticker.next().await;

        // Switch off heater on current alert.
        // iron_resources.exti_current_alert.wait_for_low()

        iron_resources
            .adc_temp
            .read(
                &mut iron_resources.adc_temp_dma,
                [
                    (&mut iron_resources.adc_pin_temp_a, SAMPLE_TIME),
                    (&mut iron_resources.adc_pin_temp_b, SAMPLE_TIME),
                ]
                .into_iter(),
                &mut adc_buffer,
            )
            .await;

        info!("ADC values (A/B): {}, {}", adc_buffer[0], adc_buffer[1]);

        iron_resources
            .adc_power
            .read(
                &mut iron_resources.adc_power_dma,
                [
                    (&mut iron_resources.adc_pin_current, SAMPLE_TIME),
                    (&mut iron_resources.adc_pin_voltage, SAMPLE_TIME),
                ]
                .into_iter(),
                &mut adc_buffer,
            )
            .await;

        info!("ADC values (current/voltage): {}, {}", adc_buffer[0], adc_buffer[1]);
    }
}
