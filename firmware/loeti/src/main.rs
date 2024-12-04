#![no_std]
#![no_main]

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, OutputType, Pull, Speed};
use embassy_stm32::ucpd::{self, Ucpd};
use embassy_stm32::{bind_interrupts, peripherals, Config};
use loeti::tool::{AdcPowerResources, AdcTemperatureResources, ToolResources};
use loeti::ui::RotaryEncoderResources;
use loeti::{display, tool, usb_pd};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    UCPD1 => ucpd::InterruptHandler<peripherals::UCPD1>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let config = Config::default();
    let p = embassy_stm32::init(config);

    info!("Hi");

    // Launch USB PD power negotiation
    {
        let ucpd = Ucpd::new(p.UCPD1, Irqs {}, p.PB6, p.PB4, Default::default());
        unwrap!(spawner.spawn(usb_pd::ucpd_task(ucpd, p.DMA1_CH1, p.DMA1_CH2)));
    }

    // Launch display
    {
        use embassy_stm32::spi;

        let display_resources = {
            let spi_config = spi::Config::default();
            display::DisplayResources {
                spi: spi::Spi::new(p.SPI2, p.PB13, p.PB15, p.PB14, p.DMA2_CH1, p.DMA2_CH2, spi_config),
                pin_dc: Output::new(p.PA10, Level::Low, Speed::High),
                pin_reset: Output::new(p.PA9, Level::Low, Speed::High),
                pin_cs: Output::new(p.PB12, Level::Low, Speed::High),
            }
        };
        unwrap!(spawner.spawn(display::display_task(display_resources)));
    }

    // Launch UI with rotary encoder control
    {
        let _rotary_encoder_resources = RotaryEncoderResources {
            pin_sw: Input::new(p.PB0, Pull::None),
            pin_a: Input::new(p.PB1, Pull::None),
            pin_b: Input::new(p.PB2, Pull::None),
        };
    }

    // Launch iron control
    {
        use embassy_stm32::adc::{Adc, AdcChannel};
        use embassy_stm32::dac::DacCh1;
        use embassy_stm32::time::khz;
        use embassy_stm32::timer::simple_pwm::PwmPin;
        use embassy_stm32::timer::simple_pwm::SimplePwm;

        let adc_temp = Adc::new(p.ADC2);
        let adc_power = Adc::new(p.ADC1);
        let dac_current_limit = DacCh1::new(p.DAC1, p.DMA1_CH5, p.PA4);
        let pwm_pin = PwmPin::new_ch4(p.PB11, OutputType::PushPull);

        let iron_resources = ToolResources {
            adc_temperature_resources: AdcTemperatureResources {
                adc_temp,
                adc_pin_temperature_a: p.PA0.degrade_adc(),
                adc_pin_temperature_b: p.PA1.degrade_adc(),
                adc_temperature_dma: p.DMA1_CH4,
            },

            adc_power_resources: AdcPowerResources {
                adc_power,
                adc_pin_voltage: p.PA2.degrade_adc(),
                adc_pin_current: p.PA3.degrade_adc(),
                adc_power_dma: p.DMA1_CH6,
            },

            dac_current_limit,

            exti_current_alert: ExtiInput::new(p.PC15, p.EXTI15, Pull::None),

            pwm_heater: SimplePwm::new(p.TIM2, None, None, None, Some(pwm_pin), khz(48), Default::default()),

            pin_sleep: Input::new(p.PA5, Pull::Up),
        };
        unwrap!(spawner.spawn(tool::tool_task(iron_resources)));
    }
}
