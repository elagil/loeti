//! A soldering controller library.
#![no_std]
#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

use core::cell::RefCell;

use defmt::Format;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};
use serde::{Deserialize, Serialize};

pub mod eeprom;
pub mod power;
pub mod tool;
pub mod ui;

/// Persistent storage data.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Format, Clone, Copy)]
pub struct Persistent {
    /// The operational temperature set point in °C.
    pub set_temperature_deg_c: i16,
    /// Current margin to leave until max. supply current in mA.
    pub current_margin_ma: u16,
    /// If true, display is rotated 180°.
    pub display_is_rotated: bool,
    /// If true, start the controller with heating switched off after power on.
    pub sleep_on_power: bool,
    /// If true, switch off heating when the tip or iron was removed/changed.
    pub sleep_on_change: bool,
}

impl Persistent {
    /// Default persistent settings.
    const fn default() -> Self {
        Self {
            set_temperature_deg_c: 300,
            current_margin_ma: 200,
            display_is_rotated: false,
            sleep_on_power: true,
            sleep_on_change: true,
        }
    }
}

/// The state of the setup menu.
#[derive(Debug, Format, Clone, Copy, Default)]
pub struct MenuState {
    /// The menu is currently open.
    pub is_open: bool,
    /// An item was toggled and evaluation is pending.
    pub toggle_pending: bool,
}

/// The operational state of the soldering station (not persistent).
#[derive(Debug, Format, Clone, Copy)]
pub struct OperationalState {
    /// The state of the control menu.
    pub menu_state: MenuState,
    /// If true, the tool is in its stand.
    pub tool_in_stand: bool,
    /// If true, the tool is off (manual sleep).
    pub tool_is_off: bool,
    /// If true, the new set temperature was not confirmed yet.
    pub set_temperature_is_pending: bool,
}

impl OperationalState {
    /// Generate a default operational state.
    pub const fn default() -> Self {
        Self {
            menu_state: MenuState {
                is_open: false,
                toggle_pending: false,
            },
            tool_in_stand: false,
            tool_is_off: true,
            set_temperature_is_pending: false,
        }
    }
}

/// Signals a change in the negotiated supply (potential/mV, current/mA).
pub static NEGOTIATED_SUPPLY_SIG: Signal<ThreadModeRawMutex, (u32, u32)> = Signal::new();

/// Signals storage of persistent data.
static STORE_PERSISTENT_SIG: Signal<ThreadModeRawMutex, ()> = Signal::new();

/// Persistently stored data (on EEPROM).
pub static PERSISTENT_MUTEX: Mutex<ThreadModeRawMutex, RefCell<Persistent>> =
    Mutex::new(RefCell::new(Persistent::default()));

/// Operational state (not persistent).
pub static OPERATIONAL_STATE_MUTEX: Mutex<ThreadModeRawMutex, RefCell<OperationalState>> =
    Mutex::new(RefCell::new(OperationalState::default()));
