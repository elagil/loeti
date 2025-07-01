//! A soldering controller library.
#![no_std]
#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

use core::cell::RefCell;

use defmt::Format;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};
use serde::{Deserialize, Serialize};

pub mod display;
pub mod eeprom;
pub mod power;
pub mod tool;
pub mod ui;

/// Persistent storage data.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Format, Clone, Copy)]
struct Persistent {
    /// The temperature set point in °C.
    set_temperature_deg_c: isize,
    /// If true, the new set temperature was not confirmed yet.
    set_temperature_pending: bool,
}

impl Persistent {
    /// Default persistent settings.
    const fn default() -> Self {
        Self {
            set_temperature_deg_c: 300,
            set_temperature_pending: false,
        }
    }
}

/// Signals a change in the maximum supply current in mA.
static MAX_SUPPLY_CURRENT_MA_SIG: Signal<ThreadModeRawMutex, f32> = Signal::new();

/// Signals a new tool power measurement (power/W, potential/V).
static POWER_MEASUREMENT_SIG: Signal<ThreadModeRawMutex, (f32, f32)> = Signal::new();

/// Signals a new tool temperature.
static TEMPERATURE_MEASUREMENT_DEG_C_SIG: Signal<ThreadModeRawMutex, f32> = Signal::new();

/// Signals a new power bargraph value.
static POWER_BARGRAPH_SIG: Signal<ThreadModeRawMutex, f32> = Signal::new();

/// Signals a new message to display.
static MESSAGE_SIG: Signal<ThreadModeRawMutex, &str> = Signal::new();

/// Persistently stored data (on EEPROM)
static PERSISTENT: Mutex<ThreadModeRawMutex, RefCell<Persistent>> = Mutex::new(RefCell::new(Persistent::default()));

/// Signals storage of persistent data.
static STORE_PERSISTENT_SIG: Signal<ThreadModeRawMutex, bool> = Signal::new();
