//! A soldering controller library.
#![no_std]
#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

use core::cell::RefCell;

use crate::control::tool::Error;
use crate::control::tool::ToolState;
use defmt::Format;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};
use serde::{Deserialize, Serialize};

pub mod app;

#[cfg(feature = "comm")]
pub mod comm;
pub mod control;
pub mod dfu;
pub mod eeprom;
pub mod power;
pub mod ui;

/// Auto-sleep modes.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Format, Clone, Copy)]
pub enum AutoSleep {
    /// The tool goes to sleep after the specified number of seconds in the stand.
    AfterDurationS(u16),
    /// The tool never goes to sleep in the stand.
    Never,
}

/// Persistent storage data.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Format, Clone, Copy)]
pub struct Persistent {
    /// The max. set temperature if the tool is in its stand.
    pub stand_temperature_deg_c: i16,
    /// The operational temperature set point in °C.
    pub set_temperature_deg_c: i16,
    /// Current margin to leave until max. supply current in mA.
    pub current_margin_ma: u16,
    /// Auto-sleep behaviour when the tool is in the stand.
    pub auto_sleep: AutoSleep,
    /// If true, display is rotated 180°.
    pub display_is_rotated: bool,
    /// If true, start the controller with heating switched off after power on.
    pub off_on_power: bool,
    /// If true, switch off heating when the tip or iron was removed/changed.
    pub off_on_change: bool,
}

impl Persistent {
    /// Default persistent settings.
    const fn default() -> Self {
        Self {
            stand_temperature_deg_c: 150,
            set_temperature_deg_c: 350,
            current_margin_ma: 200,
            auto_sleep: AutoSleep::AfterDurationS(600),
            display_is_rotated: false,
            off_on_power: false,
            off_on_change: false,
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
    /// The tool's name, or the control error.
    pub tool: Result<&'static str, Error>,
    /// The state of the tool (e.g. active, in stand).
    pub tool_state: Option<ToolState>,
    /// If true, the tool is off (manual sleep).
    pub tool_is_off: bool,
    /// If true, the new set temperature was not confirmed yet.
    pub set_temperature_is_pending: bool,
    /// The negotiated power in W.
    pub negotiated_power_w: f32,
}

impl OperationalState {
    /// Generate a default operational state.
    pub const fn default() -> Self {
        Self {
            menu_state: MenuState {
                is_open: false,
                toggle_pending: false,
            },
            tool: Err(Error::NoTool),
            tool_state: None,
            tool_is_off: true,
            set_temperature_is_pending: false,
            negotiated_power_w: f32::NAN,
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
