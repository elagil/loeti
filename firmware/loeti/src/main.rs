#![no_std]
#![no_main]

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, OutputType, Pull, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::ucpd::{self, Ucpd};
use embassy_stm32::{bind_interrupts, dma, i2c, peripherals, usb, Config};
use loeti::eeprom;
use loeti::tool::{AdcPowerResources, AdcToolResources, ToolResources};
use loeti::ui::{self, RotaryEncoderResources};
use loeti::{display, tool, usb_pd};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    UCPD1 => ucpd::InterruptHandler<peripherals::UCPD1>;
    USB_LP => usb::InterruptHandler<peripherals::USB>;
    I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = true;
        config.rcc.hse = None;
        config.rcc.pll = Some(Pll {
            source: PllSource::HSI,
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL48,
            divp: Some(PllPDiv::DIV30), // 6.4 MHz ADC sampling clock
            divq: Some(PllQDiv::DIV4),
            divr: Some(PllRDiv::DIV2), // 96 MHz system clock
        });
        config.rcc.hsi48 = Some(Hsi48Config { sync_from_usb: true });
        config.rcc.mux.adc12sel = mux::Adcsel::PLL1_P;
        config.rcc.mux.clk48sel = mux::Clk48sel::PLL1_Q;
        config.rcc.sys = Sysclk::PLL1_R;
        config.enable_debug_during_sleep = true;
    }
    let p = embassy_stm32::init(config);

    // Launch USB PD power negotiation
    {
        let ucpd = Ucpd::new(p.UCPD1, Irqs, p.PB6, p.PB4, Default::default());
        let ndb_pin = Output::new(p.PB5, Level::Low, Speed::Low);
        unwrap!(spawner.spawn(usb_pd::ucpd_task(ucpd, p.DMA1_CH1, p.DMA1_CH2, ndb_pin)));
    }

    // Launch EEPROM config storage
    {
        let i2c = i2c::I2c::new(
            p.I2C1,
            p.PA15,
            p.PB7,
            Irqs,
            p.DMA1_CH5,
            p.DMA1_CH3,
            Hertz(100_000),
            Default::default(),
        );
        let eeprom = eeprom24x::Eeprom24x::new_24x64(i2c, eeprom24x::SlaveAddr::Default);

        unwrap!(spawner.spawn(eeprom::eeprom_task(eeprom)));
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
        let rotary_encoder_resources = RotaryEncoderResources {
            pin_sw: Input::new(p.PB0, Pull::None),
            pin_a: Input::new(p.PB1, Pull::None),
            pin_b: Input::new(p.PB2, Pull::None),
        };

        unwrap!(spawner.spawn(ui::rotary_encoder_task(rotary_encoder_resources)));
    }

    // Launch iron control
    {
        use embassy_stm32::adc::{Adc, AdcChannel};
        use embassy_stm32::dac::DacCh1;
        use embassy_stm32::pac::VREFBUF;
        use embassy_stm32::time::khz;
        use embassy_stm32::timer::simple_pwm::PwmPin;
        use embassy_stm32::timer::simple_pwm::SimplePwm;

        VREFBUF.csr().write(|w| {
            w.set_envr(true);
            w.set_hiz(embassy_stm32::pac::vrefbuf::vals::Hiz::CONNECTED);
            w.set_vrs(embassy_stm32::pac::vrefbuf::vals::Vrs::VREF2);
        });

        let adc_power = Adc::new(p.ADC1);
        let adc_temp = Adc::new(p.ADC2);

        let dac_current_limit = DacCh1::new(p.DAC1, dma::NoDma, p.PA4);

        let pwm_pin = PwmPin::new_ch1(p.PA8, OutputType::PushPull);

        let tool_resources = ToolResources {
            adc_tool_resources: AdcToolResources {
                adc_temp,
                adc_pin_temperature: p.PA0.degrade_adc(),
                adc_pin_detect: p.PA1.degrade_adc(),
                adc_temperature_dma: p.DMA1_CH4,
            },

            adc_power_resources: AdcPowerResources {
                adc_power,
                adc_pin_voltage: p.PA2.degrade_adc(),
                adc_pin_current: p.PA3.degrade_adc(),
                adc_power_dma: p.DMA1_CH6,
            },

            dac_current_limit,

            exti_current_alert: ExtiInput::new(p.PB11, p.EXTI11, Pull::None),

            pwm_heater: SimplePwm::new(p.TIM1, Some(pwm_pin), None, None, None, khz(34), Default::default()),

            pin_sleep: Input::new(p.PB10, Pull::None),
        };
        unwrap!(spawner.spawn(tool::tool_task(tool_resources)));
    }
}
