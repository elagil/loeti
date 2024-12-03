use defmt::info;
use embassy_stm32::{adc, dac, peripherals};
use embassy_time::{Duration, Ticker};

pub struct IronResources {
    pub adc_temp: adc::Adc<'static, peripherals::ADC2>,
    pub adc_pin_temp_a: adc::AnyAdcChannel<peripherals::ADC2>,
    pub adc_pin_temp_b: adc::AnyAdcChannel<peripherals::ADC2>,
    pub adc_temp_dma: peripherals::DMA1_CH4,

    pub adc_power: adc::Adc<'static, peripherals::ADC1>,
    pub adc_pin_voltage: adc::AnyAdcChannel<peripherals::ADC1>,
    pub adc_pin_current: adc::AnyAdcChannel<peripherals::ADC1>,
    pub adc_power_dma: peripherals::DMA1_CH6,

    pub dac_current_limit: dac::DacChannel<'static, peripherals::DAC1, 1, peripherals::DMA1_CH5>,
}

#[embassy_executor::task]
pub async fn iron_task(mut iron_resources: IronResources) {
    const DAC_VOLTAGE: u16 = 123;
    iron_resources
        .dac_current_limit
        .set(dac::Value::Bit12Right(DAC_VOLTAGE));

    const SAMPLE_TIME: adc::SampleTime = adc::SampleTime::CYCLES47_5;
    let mut adc_buffer = [0u16; 2];
    let mut ticker = Ticker::every(Duration::from_hz(1));

    loop {
        ticker.next().await;

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
