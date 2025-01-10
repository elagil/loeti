//! A soldering controller library.
#![no_std]
#![warn(missing_docs)]

use core::cell::RefCell;

use defmt::Format;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};
use serde::{Deserialize, Serialize};
use uom::si::f32::ElectricCurrent;

pub mod display;
pub mod eeprom;
pub mod tool;
pub mod ui;
pub mod usb_pd;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Format, Clone, Copy)]
struct Persistent {
    set_temperature_deg_c: isize,
    set_temperature_pending: bool,
}

impl Persistent {
    const fn default() -> Self {
        Self {
            set_temperature_deg_c: 300,
            set_temperature_pending: false,
        }
    }
}

static MAX_SUPPLY_CURRENT_SIG: Signal<ThreadModeRawMutex, Option<ElectricCurrent>> = Signal::new();

static POWER_MEASUREMENT_W_SIG: Signal<ThreadModeRawMutex, f32> = Signal::new();
static TEMPERATURE_MEASUREMENT_DEG_C_SIG: Signal<ThreadModeRawMutex, f32> = Signal::new();
static POWER_RATIO_SIG: Signal<ThreadModeRawMutex, f32> = Signal::new();
static TOOL_NAME_SIG: Signal<ThreadModeRawMutex, &str> = Signal::new();

static PERSISTENT: Mutex<ThreadModeRawMutex, RefCell<Persistent>> = Mutex::new(RefCell::new(Persistent::default()));
static STORE_PERSISTENT: Signal<ThreadModeRawMutex, bool> = Signal::new();
