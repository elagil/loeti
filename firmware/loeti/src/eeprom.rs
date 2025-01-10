//! Handles user inputs by means of a rotary encoder.
use defmt::{debug, trace};
use embassy_stm32::i2c::{self};
use embassy_time::Timer;
use postcard::from_bytes_cobs;

use crate::{Persistent, PERSISTENT, STORE_PERSISTENT};

type Eeprom = eeprom24x::Eeprom24x<
    i2c::I2c<'static, embassy_stm32::mode::Async>,
    eeprom24x::page_size::B32,
    eeprom24x::addr_size::TwoBytes,
    eeprom24x::unique_serial::No,
>;

async fn load_persistent(eeprom: &mut Eeprom) {
    let mut buf = [0u8; 32];

    while eeprom.read_data(0, &mut buf).is_err() {
        debug!("Retry EEPROM read.");
        Timer::after_millis(10).await;
    }

    trace!("EEPROM read: {}", buf);

    let persistent: Persistent = match from_bytes_cobs(&mut buf) {
        Ok(x) => {
            debug!("Loaded persistent storage {}.", x);
            x
        }
        Err(_) => {
            debug!("Initialize new persistent storage.");
            Persistent::default()
        }
    };

    PERSISTENT.lock(|x| x.replace(persistent));
}

async fn store_persistent(eeprom: &mut Eeprom) {
    let persistent = PERSISTENT.lock(|x| *x.borrow());

    let mut buf = [0u8; 32];
    postcard::to_slice_cobs(&persistent, &mut buf).unwrap();
    trace!("EEPROM write: {}", buf);

    while eeprom.write_page(0, &buf).is_err() {
        debug!("Retry EEPROM write.");
        Timer::after_millis(10).await;
    }
    debug!("Wrote persistent storage.");
}

/// Handles reading and writing EEPROM.
#[embassy_executor::task]
pub async fn eeprom_task(mut eeprom: Eeprom) {
    load_persistent(&mut eeprom).await;

    loop {
        let _ = STORE_PERSISTENT.wait().await;
        store_persistent(&mut eeprom).await;
    }
}
