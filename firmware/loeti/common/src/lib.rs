//! A soldering controller library.
#![no_std]
#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

use core::cell::RefCell;

use defmt::Format;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};
use serde::{Deserialize, Serialize};

use crate::tool::{Error as ToolError, ToolState};

pub mod app;

#[cfg(feature = "comm")]
pub(crate) mod comm;
pub(crate) mod dfu;
pub(crate) mod eeprom;
pub(crate) mod power;
pub(crate) mod tool;
pub(crate) mod ui;

/// Auto-sleep modes.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Format, Clone, Copy)]
pub(crate) enum AutoSleep {
    /// The tool goes to sleep after the specified number of seconds in the stand.
    AfterDurationS(u16),
    /// The tool never goes to sleep in the stand.
    Never,
}

/// Persistent storage data.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Format, Clone, Copy)]
pub(crate) struct Persistent {
    /// The max. set temperature if the tool is in its stand.
    pub(crate) stand_temperature_deg_c: i16,
    /// The operational temperature set point in °C.
    pub(crate) set_temperature_deg_c: i16,
    /// Current margin to leave until max. supply current in mA.
    pub(crate) current_margin_ma: u16,
    /// Auto-sleep behaviour when the tool is in the stand.
    pub(crate) auto_sleep: AutoSleep,
    /// If true, display is rotated 180°.
    pub(crate) display_is_rotated: bool,
    /// If true, start the controller with heating switched off after power on.
    pub(crate) off_on_power: bool,
    /// If true, switch off heating when the tip or iron was removed/changed.
    pub(crate) off_on_change: bool,
}

impl Persistent {
    /// Default persistent settings.
    const fn default() -> Self {
        Self {
            stand_temperature_deg_c: 180,
            set_temperature_deg_c: 300,
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
pub(crate) struct MenuState {
    /// The menu is currently open.
    pub(crate) is_open: bool,
    /// An item was toggled and evaluation is pending.
    pub(crate) toggle_pending: bool,
}

/// The operational state of the soldering station (not persistent).
#[derive(Debug, Format, Clone, Copy)]
pub(crate) struct OperationalState {
    /// The state of the control menu.
    pub(crate) menu_state: MenuState,
    /// The tool's name, or a tool error.
    pub(crate) tool: Result<&'static str, ToolError>,
    /// The state of the tool (e.g. active, in stand).
    pub(crate) tool_state: Option<ToolState>,
    /// If true, the tool is off (manual sleep).
    pub(crate) tool_is_off: bool,
    /// If true, the new set temperature was not confirmed yet.
    pub(crate) set_temperature_is_pending: bool,
}

impl OperationalState {
    /// Generate a default operational state.
    pub(crate) const fn default() -> Self {
        Self {
            menu_state: MenuState {
                is_open: false,
                toggle_pending: false,
            },
            tool: Err(ToolError::NoTool),
            tool_state: None,
            tool_is_off: true,
            set_temperature_is_pending: false,
        }
    }
}

/// Signals a change in the negotiated supply (potential/mV, current/mA).
pub(crate) static NEGOTIATED_SUPPLY_SIG: Signal<ThreadModeRawMutex, (u32, u32)> = Signal::new();

/// Signals storage of persistent data.
static STORE_PERSISTENT_SIG: Signal<ThreadModeRawMutex, ()> = Signal::new();

/// Persistently stored data (on EEPROM).
pub(crate) static PERSISTENT_MUTEX: Mutex<ThreadModeRawMutex, RefCell<Persistent>> =
    Mutex::new(RefCell::new(Persistent::default()));

/// Operational state (not persistent).
pub(crate) static OPERATIONAL_STATE_MUTEX: Mutex<ThreadModeRawMutex, RefCell<OperationalState>> =
    Mutex::new(RefCell::new(OperationalState::default()));
