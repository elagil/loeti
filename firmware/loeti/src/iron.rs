use defmt::info;
use embassy_stm32::{adc, dac, peripherals};
use embassy_time::{Duration, Ticker};

pub struct IronResources<T: adc::Instance> {
    pub adc: adc::Adc<'static, T>,
    pub adc_pin_a: adc::AnyAdcChannel<T>,
    pub adc_pin_b: adc::AnyAdcChannel<T>,
    pub adc_dma: peripherals::DMA1_CH4,

    pub dac: dac::DacChannel<'static, peripherals::DAC1, 1, peripherals::DMA1_CH5>,
}

#[embassy_executor::task]
pub async fn iron_task(mut iron_resources: IronResources<peripherals::ADC2>) {
    const DAC_VOLTAGE: u16 = 123;
    iron_resources.dac.set(dac::Value::Bit12Right(DAC_VOLTAGE));

    const SAMPLE_TIME: adc::SampleTime = adc::SampleTime::CYCLES47_5;
    let mut adc_buffer = [0u16; 2];

    let mut ticker = Ticker::every(Duration::from_hz(1));
    loop {
        ticker.next().await;

        iron_resources
            .adc
            .read(
                &mut iron_resources.adc_dma,
                [
                    (&mut iron_resources.adc_pin_a, SAMPLE_TIME),
                    (&mut iron_resources.adc_pin_b, SAMPLE_TIME),
                ]
                .into_iter(),
                &mut adc_buffer,
            )
            .await;

        info!("ADC values (A/B) {},{}", adc_buffer[0], adc_buffer[1]);
    }
}
