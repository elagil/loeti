//! Handles user inputs by means of a rotary encoder.
use defmt::info;
use embassy_stm32::gpio::Input;
use embassy_time::{Duration, Instant, Ticker};
use rotary_encoder_embedded::{Direction, RotaryEncoder};

use crate::{PERSISTENT, STORE_PERSISTENT_SIG};

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
    let mut rotary_encoder = RotaryEncoder::new(resources.pin_a, resources.pin_b).into_standard_mode();
    rotary_encoder.update();

    let mut ticker = Ticker::every(Duration::from_millis(1));

    const SHORT_PRESS_DURATION: Duration = Duration::from_millis(25);
    const LONG_PRESS_DURATION: Duration = Duration::from_millis(500);

    let mut release_instant = Instant::now();

    let mut switch_state = SwitchState::Released;

    loop {
        let step = match rotary_encoder.update() {
            Direction::Clockwise => 1,
            Direction::Anticlockwise => -1,
            _ => 0,
        };

        if step != 0 {
            PERSISTENT.lock(|x| {
                let mut persistent = x.borrow_mut();

                if persistent.set_temperature_deg_c >= 450 && step > 0 {
                    // Upper temperature limit.
                } else if persistent.set_temperature_deg_c <= 100 && step < 0 {
                    // Lower temperature limit.
                } else {
                    persistent.set_temperature_deg_c += step * 10;
                    persistent.set_temperature_pending = true;
                }
            });
        }

        let pressed = resources.pin_sw.is_low();

        let mut switch_event: SwitchEvent = SwitchEvent::None;

        if matches!(switch_state, SwitchState::Released | SwitchState::WaitForRelease) && pressed {
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

        match switch_event {
            SwitchEvent::ShortPress => {
                info!("short");
                PERSISTENT.lock(|x| {
                    let mut persistent = x.borrow_mut();
                    persistent.set_temperature_pending = false;
                });
                STORE_PERSISTENT_SIG.signal(true);
            }
            SwitchEvent::LongPress => info!("long"),
            _ => (),
        }

        ticker.next().await;
    }
}
