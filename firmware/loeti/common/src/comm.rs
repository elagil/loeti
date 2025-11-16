//! Enables communication with a host.
use ergot::exports::bbq2::{prod_cons::framed::FramedConsumer, traits::coordination::cs::CsCoord};
pub use ergot::toolkits::embassy_usb_v0_5 as kit;

use embassy_stm32::{peripherals::USB, usb};
use mutex::raw_impls::single_core_thread_mode::ThreadModeRawMutex;

/// Output queue size.
pub const OUT_QUEUE_SIZE: usize = 1024;

/// Maximum packet size.
pub const MAX_PACKET_SIZE: usize = 128;

/// Our USB driver
pub type AppDriver = usb::Driver<'static, USB>;
/// The type of our netstack
pub type Stack = kit::Stack<&'static Queue, ThreadModeRawMutex>;
/// The type of our outgoing queue
pub type Queue = kit::Queue<OUT_QUEUE_SIZE, CsCoord>;
/// The type of our RX Worker
pub type RxWorker = kit::RxWorker<&'static Queue, ThreadModeRawMutex, AppDriver>;

/// Statically store our outgoing packet buffer
pub static OUT_QUEUE: Queue = kit::Queue::new();
/// Statically store our USB app buffers
pub static STORAGE: kit::WireStorage<256, 256, 64, 256> = kit::WireStorage::new();
/// Statically store our netstack
pub static STACK: Stack =
    kit::new_target_stack(OUT_QUEUE.framed_producer(), MAX_PACKET_SIZE as u16);

/// Configure the USB device.
pub fn usb_config(serial: &'static str) -> embassy_usb::Config<'static> {
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

/// Send a measurement to the host.
pub fn send_measurement(measurement: &loeti_protocol::Measurement) {
    _ = crate::comm::STACK
        .topics()
        .broadcast::<loeti_protocol::MeasurementTopic>(measurement, None);
}

/// Send status info to the host.
pub fn send_status(status: &loeti_protocol::Status) {
    _ = crate::comm::STACK
        .topics()
        .broadcast::<loeti_protocol::StatusTopic>(status, None);
}

/// Handles the low level USB management
#[embassy_executor::task]
pub async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, AppDriver>) {
    usb.run().await;
}

/// TX handler for the communication stack.
#[embassy_executor::task]
pub async fn run_tx(
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

/// RX handler for the communication stack.
#[embassy_executor::task]
pub async fn run_rx(rcvr: RxWorker, recv_buf: &'static mut [u8]) {
    rcvr.run(recv_buf, kit::USB_FS_MAX_PACKET_SIZE).await;
}

/// Ping server.
#[embassy_executor::task]
pub async fn pingserver() {
    STACK.services().ping_handler::<4>().await;
}
