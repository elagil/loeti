#![no_std]
#![no_main]

use defmt::unwrap;
use embassy_executor::Spawner;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, OutputType, Pull, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::{bind_interrupts, i2c, peripherals, usb, Config};
use embassy_time::Timer;
use loeti::power::{AssignedResources, UcpdResources};
use loeti::tool::{AdcResources, ToolResources};
use loeti::ui::{self, encoder::RotaryEncoderResources};
use loeti::{eeprom, power};
use loeti::{split_resources, tool, ui::display};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
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
            mul: PllMul::MUL85,
            divp: Some(PllPDiv::DIV30),
            divq: None,
            divr: Some(PllRDiv::DIV2), // 170 MHz system clock
        });
        config.rcc.hsi48 = Some(Hsi48Config {
            sync_from_usb: true,
        });
        config.rcc.mux.adc12sel = mux::Adcsel::PLL1_P;
        config.rcc.mux.clk48sel = mux::Clk48sel::HSI48;
        config.rcc.sys = Sysclk::PLL1_R;
        config.enable_debug_during_sleep = true;
    }
    let p = embassy_stm32::init(config);
    let mut core_peri = cortex_m::Peripherals::take().unwrap();

    // Enable instruction cache.
    core_peri.SCB.enable_icache();

    // Launch USB PD power negotiation
    {
        let resources = split_resources!(p);
        let ndb_pin = Output::new(p.PB5, Level::Low, Speed::Low);
        unwrap!(spawner.spawn(power::ucpd_task(resources.ucpd, ndb_pin)));
    }

    Timer::after_millis(500).await;

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
        let mut eeprom = eeprom24x::Eeprom24x::new_24x64(i2c, eeprom24x::SlaveAddr::Default);

        // Load data before any other tasks access persistent storage.
        eeprom::load_persistent(&mut eeprom).await;
        unwrap!(spawner.spawn(eeprom::eeprom_task(eeprom)));
    }

    // Launch display
    {
        use embassy_stm32::spi;

        let display_resources = {
            let mut spi_config = spi::Config::default();
            spi_config.frequency = Hertz(10_000_000);

            let spi = spi::Spi::new_txonly(p.SPI2, p.PB13, p.PB15, p.DMA2_CH1, spi_config);

            display::DisplayResources {
                spi,
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

        unwrap!(spawner.spawn(ui::encoder::rotary_encoder_task(rotary_encoder_resources)));
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

        let tool_resources = ToolResources {
            adc_resources: AdcResources {
                adc: Adc::new(p.ADC1),
                pin_temperature: p.PA0.degrade_adc(),
                pin_detect: p.PA1.degrade_adc(),
                pin_voltage: p.PA2.degrade_adc(),
                pin_current: p.PA3.degrade_adc(),
                adc_dma: p.DMA1_CH6,
            },
            dac_current_limit: DacCh1::new_blocking(p.DAC1, p.PA4),
            exti_current_alert: ExtiInput::new(p.PB11, p.EXTI11, Pull::None),
            pwm_heater: SimplePwm::new(
                p.TIM1,
                Some(PwmPin::new(p.PA8, OutputType::PushPull)),
                None,
                None,
                None,
                khz(34),
                Default::default(),
            ),
            pin_sleep: Input::new(p.PB10, Pull::None),
        };
        unwrap!(spawner.spawn(tool::tool_task(tool_resources)));
    }
}
