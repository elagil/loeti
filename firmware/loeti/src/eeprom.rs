//! Handles user inputs by means of a rotary encoder.
use crc;
use defmt::debug;
use embassy_stm32::i2c::{self};
use embassy_time::Timer;
use postcard::from_bytes_cobs;

use crate::{Persistent, PERSISTENT_MUTEX, STORE_PERSISTENT_SIG};

/// The type of EEPROM on this device.
type Eeprom = eeprom24x::Eeprom24x<
    i2c::I2c<'static, embassy_stm32::mode::Async, embassy_stm32::i2c::mode::Master>,
    eeprom24x::page_size::B32,
    eeprom24x::addr_size::TwoBytes,
    eeprom24x::unique_serial::No,
>;

/// Size of a page.
const SIZE: usize = 32;

/// Size of the checksum.
const CHECKSUM_SIZE: usize = 4;

/// Calculate checksum bytes for provided data.
fn calculate_checksum_bytes(data: &[u8]) -> [u8; 4] {
    let crc = crc::Crc::<u32>::new(&crc::CRC_32_CKSUM);
    crc.checksum(data).to_le_bytes()
}

/// Load persistent data from EEPROM.
pub async fn load_persistent(eeprom: &mut Eeprom) {
    let mut expected_checksum_bytes = [0u8; CHECKSUM_SIZE];
    while eeprom.read_data(0, &mut expected_checksum_bytes).is_err() {
        debug!("Retry EEPROM read");
        Timer::after_millis(10).await;
    }
    debug!("Expected checksum bytes: {}", expected_checksum_bytes);

    let mut buf = [0u8; SIZE];
    while eeprom.read_data(SIZE as u32, &mut buf).is_err() {
        debug!("Retry EEPROM read");
        Timer::after_millis(10).await;
    }

    let mut data = if let Some(zero_index) = buf.iter().position(|&x| x == 0) {
        let checksum_bytes = calculate_checksum_bytes(&buf[..=zero_index]);
        debug!("Read raw data: {}", buf);
        debug!("Calculated checksum bytes: {}", checksum_bytes);

        if checksum_bytes != expected_checksum_bytes {
            None
        } else {
            from_bytes_cobs(&mut buf).ok()
        }
    } else {
        None
    };

    if data.is_none() {
        store_defaults(eeprom).await;
        data = Some(Persistent::default())
    }

    PERSISTENT_MUTEX.lock(|x| x.replace(data.unwrap()));
}

/// Store provided persistent data to EEPROM.
async fn store(eeprom: &mut Eeprom, data: &Persistent) {
    let mut data_buffer = [0u8; SIZE];
    let encoded_data = postcard::to_slice_cobs(&data, &mut data_buffer).unwrap();

    let checksum_bytes: [u8; CHECKSUM_SIZE] = calculate_checksum_bytes(encoded_data);
    if eeprom.write_page(0, &checksum_bytes).is_err() {
        debug!("Retry EEPROM checksum write");
        Timer::after_millis(10).await;
    }
    debug!("EEPROM wrote checksum bytes: {}", checksum_bytes);

    // Maximum write delay.
    Timer::after_millis(5).await;

    if eeprom.write_page(SIZE as u32, encoded_data).is_err() {
        debug!("Retry EEPROM data write");
        Timer::after_millis(10).await;
    }
    debug!("EEPROM wrote data bytes: {}", encoded_data);
}

/// Store default persistent data to EEPROM (reset);
pub async fn store_defaults(eeprom: &mut Eeprom) {
    debug!("EEPROM store defaults");
    store(eeprom, &Persistent::default()).await;
}

/// Store current persistent data to EEPROM.
async fn store_persistent(eeprom: &mut Eeprom) {
    store(eeprom, &PERSISTENT_MUTEX.lock(|x| *x.borrow())).await;
}

/// Handles reading and writing EEPROM.
#[embassy_executor::task]
pub async fn eeprom_task(mut eeprom: Eeprom) {
    loop {
        STORE_PERSISTENT_SIG.wait().await;
        store_persistent(&mut eeprom).await;
    }
}
