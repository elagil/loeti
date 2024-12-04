//! Handles user inputs by means of a rotary encoder.
use embassy_stm32::gpio::Input;

/// Resources for reading a rotary encoder.
pub struct RotaryEncoderResources {
    /// The pin for the encoder's push button.
    pub pin_sw: Input<'static>,
    /// The pin for phase A.
    pub pin_a: Input<'static>,
    /// The pin for phase B.
    pub pin_b: Input<'static>,
}
