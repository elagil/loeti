//! Handles user inputs by means of a rotary encoder.
use defmt::info;
use embassy_stm32::gpio::Input;
use embassy_time::{Duration, Instant, Ticker};
use rotary_encoder_embedded::{Direction, RotaryEncoder};

use crate::{ui::MENU_STEPS_SIG, OPERATIONAL_STATE_MUTEX, PERSISTENT_MUTEX, STORE_PERSISTENT_SIG};

/// The state of the user interface (controlled instance).
#[derive(Debug, Clone, Copy)]
enum UiState {
    /// Default control: idle.
    Idle,
    /// Temperature is being controlled.
    Temperature,
    /// The menu is being controlled.
    Menu,
}

/// Resources for reading a rotary encoder.
pub struct RotaryEncoderResources {
    /// The pin for the encoder's push button.
    pub pin_sw: Input<'static>,
    /// The pin for phase A.
    pub pin_a: Input<'static>,
    /// The pin for phase B.
    pub pin_b: Input<'static>,
}

/// The state of the switch.
#[derive(Clone, Copy)]
enum SwitchState {
    /// Switch is released.
    Released,
    /// Switch is pressed and waiting to be released.
    WaitForRelease,
    /// Switch is being pressed long.
    LongPress,
}

/// Events that can be detected for the switch.
enum SwitchEvent {
    /// No interaction.
    None,
    /// A short press was registered (at least 25 ms).
    ShortPress,
    /// A long press was registered (at least 500 ms).
    LongPress,
}

/// Reads the rotary encoder and switch.
#[embassy_executor::task]
pub async fn rotary_encoder_task(resources: RotaryEncoderResources) {
    let mut ui_state = UiState::Idle;
    let mut rotary_encoder =
        RotaryEncoder::new(resources.pin_a, resources.pin_b).into_standard_mode();
    rotary_encoder.update();

    let mut ticker = Ticker::every(Duration::from_millis(1));

    const SHORT_PRESS_DURATION: Duration = Duration::from_millis(25);
    const LONG_PRESS_DURATION: Duration = Duration::from_millis(500);

    let mut release_instant = Instant::now();
    let mut switch_state = SwitchState::Released;

    loop {
        let direction = if PERSISTENT_MUTEX.lock(|x| x.borrow().display_is_rotated) {
            1
        } else {
            -1
        };

        let steps = match rotary_encoder.update() {
            Direction::Clockwise => 1,
            Direction::Anticlockwise => -1,
            _ => 0,
        } * direction;

        if steps != 0 {
            ui_state = match ui_state {
                UiState::Idle | UiState::Temperature => {
                    let set_temperature_pending = PERSISTENT_MUTEX.lock(|x| {
                        let mut persistent = x.borrow_mut();

                        if persistent.operational_temperature_deg_c >= 450 && steps > 0 {
                            // Upper temperature limit.
                            false
                        } else if persistent.operational_temperature_deg_c <= 100 && steps < 0 {
                            // Lower temperature limit.
                            false
                        } else {
                            persistent.operational_temperature_deg_c += steps * 10;
                            true
                        }
                    });

                    OPERATIONAL_STATE_MUTEX.lock(|x| {
                        x.borrow_mut().set_temperature_is_pending = set_temperature_pending;
                    });

                    UiState::Temperature
                }
                UiState::Menu => {
                    MENU_STEPS_SIG.signal(steps as isize);

                    ui_state
                }
            };
        }

        let pressed = resources.pin_sw.is_low();
        let mut switch_event: SwitchEvent = SwitchEvent::None;

        if matches!(
            switch_state,
            SwitchState::Released | SwitchState::WaitForRelease
        ) && pressed
        {
            let press_duration = Instant::now().duration_since(release_instant);

            if press_duration >= LONG_PRESS_DURATION {
                switch_state = SwitchState::LongPress;
                switch_event = SwitchEvent::LongPress;
            } else if press_duration >= SHORT_PRESS_DURATION {
                switch_state = SwitchState::WaitForRelease;
            }
        } else if !pressed {
            release_instant = Instant::now();

            if matches!(switch_state, SwitchState::WaitForRelease) {
                switch_event = SwitchEvent::ShortPress;
            }

            switch_state = SwitchState::Released;
        };

        ui_state = match (switch_event, ui_state) {
            (SwitchEvent::ShortPress, UiState::Temperature) => {
                OPERATIONAL_STATE_MUTEX.lock(|x| {
                    x.borrow_mut().set_temperature_is_pending = false;
                });
                STORE_PERSISTENT_SIG.signal(());
                info!("store temperature");

                UiState::Idle
            }
            (SwitchEvent::ShortPress, UiState::Idle) => {
                let manual_sleep = OPERATIONAL_STATE_MUTEX.lock(|x| {
                    let mut operational_state = x.borrow_mut();
                    operational_state.tool_is_off = !operational_state.tool_is_off;
                    operational_state.tool_is_off
                });
                info!("toggle manual sleep ({})", manual_sleep);

                ui_state
            }
            (SwitchEvent::ShortPress, UiState::Menu) => {
                OPERATIONAL_STATE_MUTEX.lock(|x| {
                    x.borrow_mut().menu_state.toggle_pending = true;
                });

                ui_state
            }
            (SwitchEvent::LongPress, UiState::Idle) => {
                OPERATIONAL_STATE_MUTEX.lock(|x| {
                    x.borrow_mut().menu_state.is_open = true;
                });
                info!("open menu");

                UiState::Menu
            }
            (SwitchEvent::LongPress, UiState::Menu) => {
                OPERATIONAL_STATE_MUTEX.lock(|x| {
                    x.borrow_mut().menu_state.is_open = false;
                });
                info!("close menu");

                UiState::Idle
            }
            _ => ui_state,
        };

        ticker.next().await;
    }
}
