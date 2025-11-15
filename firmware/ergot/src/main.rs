#![no_std]
#![no_main]

use ergot::{
    exports::bbq2::{prod_cons::framed::FramedConsumer, traits::coordination::cs::CsCoord},
    fmt,
    interface_manager::InterfaceState,
    interface_manager::Profile,
    toolkits::embassy_usb_v0_5 as kit,
};

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, OutputType, Pull, Speed};
use embassy_stm32::peripherals::USB;
use embassy_stm32::time::Hertz;
use embassy_stm32::{Config, bind_interrupts, i2c, peripherals, usb};
use embassy_time::{Duration, Ticker, WithTimeout};
use loeti_protocol::{Measurement, MeasurementTopic};
use mutex::raw_impls::cs::CriticalSectionRawMutex;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USB_LP => usb::InterruptHandler<peripherals::USB>;
    I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
});

const OUT_QUEUE_SIZE: usize = 1024;
const MAX_PACKET_SIZE: usize = 128;

/// Our USB driver
pub type AppDriver = usb::Driver<'static, USB>;
/// The type of our netstack
type Stack = kit::Stack<&'static Queue, CriticalSectionRawMutex>;
/// The type of our outgoing queue
type Queue = kit::Queue<OUT_QUEUE_SIZE, CsCoord>;

/// Statically store our outgoing packet buffer
static OUT_QUEUE: Queue = kit::Queue::new();
/// Statically store our USB app buffers
static STORAGE: kit::WireStorage<256, 256, 64, 256> = kit::WireStorage::new();
/// Statically store our netstack
static STACK: Stack = kit::new_target_stack(OUT_QUEUE.framed_producer(), MAX_PACKET_SIZE as u16);

fn usb_config(serial: &'static str) -> embassy_usb::Config<'static> {
    let mut config = embassy_usb::Config::new(0x16c0, 0x27DD);
    config.manufacturer = Some("OneVariable");
    config.product = Some("ergot-pico");
    config.serial_number = Some(serial);

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;

    config
}

/// This handles the low level USB management
#[embassy_executor::task]
pub async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, AppDriver>) {
    usb.run().await;
}

#[embassy_executor::task]
async fn run_tx(
    mut ep_in: <AppDriver as embassy_usb::driver::Driver<'static>>::EndpointIn,
    rx: FramedConsumer<&'static Queue>,
) {
    kit::tx_worker::<AppDriver, OUT_QUEUE_SIZE, CsCoord>(
        &mut ep_in,
        rx,
        kit::DEFAULT_TIMEOUT_MS_PER_FRAME,
        kit::USB_FS_MAX_PACKET_SIZE,
    )
    .await;
}

#[embassy_executor::task]
async fn pingserver() {
    STACK.services().ping_handler::<4>().await;
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hi");

    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = true;
        config.rcc.hse = None;
        config.rcc.pll = Some(Pll {
            source: PllSource::HSI,
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL85,
            divp: Some(PllPDiv::DIV20), // 17 MHz ADC clock
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

    // USB/RPC INIT
    {
        let driver = usb::Driver::new(p.USB, Irqs, p.PA12, p.PA11);
        let config = usb_config("123");
        let (device, tx_impl, _ep_out) = STORAGE.init_ergot(driver, config);

        spawner.must_spawn(usb_task(device));
        spawner.must_spawn(pingserver());
        spawner.must_spawn(run_tx(tx_impl, OUT_QUEUE.framed_consumer()));

        // Wait for connection
        let mut ticker = Ticker::every(Duration::from_millis(500));
        loop {
            ticker.next().await;

            let has_addr = STACK.manage_profile(|p| {
                let Some(state) = p.interface_state(()) else {
                    return false;
                };

                info!("{:?}", state);

                let InterfaceState::Active { net_id, .. } = state else {
                    return false;
                };
                net_id != 0
            });

            if has_addr {
                STACK.info_fmt(fmt!("Connected"));
                break;
            }
        }

        loop {
            ticker.next().await;
            // Publish data
            let measurement = Measurement {
                seq: 10,
                p: 0.1,
                i: 0.2,
                d: 0.3,
                temperature_deg_c: 123.0,
            };
            _ = STACK
                .topics()
                .broadcast::<MeasurementTopic>(&measurement, None);
        }
    }
}
