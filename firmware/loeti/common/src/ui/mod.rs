//! User-interface components.

use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};

#[cfg(feature = "display")]
pub mod display;
pub mod encoder;

/// Signals encoder steps for menu operation.
static MENU_STEPS_SIG: Signal<ThreadModeRawMutex, isize> = Signal::new();
