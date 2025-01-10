#![no_std]
#![no_main]

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_stm32::gpio::Output;
use embassy_stm32::ucpd::{self, Ucpd};
use embassy_stm32::{bind_interrupts, peripherals, Config};
use usb_pd_testing::power;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    UCPD1 => ucpd::InterruptHandler<peripherals::UCPD1>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let config = Config::default();
    let p = embassy_stm32::init(config);

    info!("Hello World!");

    // This pin controls the dead-battery mode on the attached TCPP01-M12.
    let tcpp01_m12_ndb = Output::new(p.PA9, embassy_stm32::gpio::Level::Low, embassy_stm32::gpio::Speed::Low);

    let ucpd = Ucpd::new(p.UCPD1, Irqs {}, p.PB13, p.PB14, Default::default());
    unwrap!(spawner.spawn(power::ucpd_task(ucpd, p.GPDMA1_CH0, p.GPDMA1_CH1, tcpp01_m12_ndb)));
}
