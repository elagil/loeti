#![no_std]
#![no_main]

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::ucpd::{self, Ucpd};
use embassy_stm32::{bind_interrupts, peripherals, spi, Config};
use loeti::{display, usb_pd};
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
    let ucpd = Ucpd::new(p.UCPD1, Irqs {}, p.PB6, p.PB4, Default::default());
    unwrap!(spawner.spawn(usb_pd::ucpd_task(ucpd, p.DMA1_CH1, p.DMA1_CH2)));

    // Launch display
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
