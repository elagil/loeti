//! Handles user inputs by means of a rotary encoder.
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

/// Load persistent data from EEPROM.
pub async fn load_persistent(eeprom: &mut Eeprom) {
    let mut buf = [0u8; SIZE];

    while eeprom.read_data(0, &mut buf).is_err() {
        debug!("Retry EEPROM read");
        Timer::after_millis(10).await;
    }

    debug!("EEPROM read: {}", buf);

    let data: Persistent = match from_bytes_cobs(&mut buf) {
        Ok(x) => {
            debug!("Loaded persistent storage {}", x);
            x
        }
        Err(_) => {
            debug!("Initialize new persistent storage");
            Persistent::default()
        }
    };

    PERSISTENT_MUTEX.lock(|x| x.replace(data));
}

/// Store persistent data to EEPROM.
async fn store_persistent(eeprom: &mut Eeprom) {
    let data = PERSISTENT_MUTEX.lock(|x| *x.borrow());

    let mut buf = [0u8; SIZE];
    let used = postcard::to_slice_cobs(&data, &mut buf).unwrap();

    while eeprom.write_page(0, used).is_err() {
        debug!("Retry EEPROM write");
        Timer::after_millis(10).await;
    }
    debug!("EEPROM wrote: {}", used);
}

/// Handles reading and writing EEPROM.
#[embassy_executor::task]
pub async fn eeprom_task(mut eeprom: Eeprom) {
    loop {
        STORE_PERSISTENT_SIG.wait().await;
        store_persistent(&mut eeprom).await;
    }
}
