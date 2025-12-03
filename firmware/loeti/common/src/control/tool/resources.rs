//! Drives the tool's heating element, based on target and actual temperature.

use super::sensors;
use core::f32;
use defmt::Format;
use embassy_stm32::dac::Ch1;
use embassy_stm32::mode::Blocking;
use embassy_stm32::timer::simple_pwm::SimplePwm;
use embassy_stm32::{dac, exti::ExtiInput, gpio::Input, peripherals};
use embassy_time::{Duration, WithTimeout};
use micromath::F32Ext;
use uom::si::electric_current;
use uom::si::f32::ElectricCurrent;

/// The type for the PWM heater channel.
type PwmHeaterChannel<'d> =
    embassy_stm32::timer::simple_pwm::SimplePwmChannel<'d, peripherals::TIM1>;

/// Peak current limit for the tool.
pub const PEAK_CURRENT_LIMIT_A: f32 = 10.0;
/// Current monitor total gain (shunt + amplifier): 0.2 V/A
pub const CURRENT_MONITOR_GAIN_V_PER_A: f32 = 0.2;

/// Errors during tool detection.

#[derive(Debug, Format, Clone, Copy)]
pub enum Error {
    /// No tool was found.
    NoTool,
    /// Tool was detected, but no tip.
    NoTip,
    /// The detected tool is unknown.
    UnknownTool,
    /// Tool type mismatch during control loop operation.
    ToolMismatch,
}

/// Resources for driving the tool's heater and taking associated measurements.
pub struct ToolResources {
    /// Resources for measuring tool properties.
    pub sensors: sensors::Sensors,

    /// The DAC that sets the current limit for the current sensing IC (INA301A).
    pub dac_current_limit: dac::DacChannel<'static, peripherals::DAC1, Ch1, Blocking>,

    /// External interrupt for current alerts.
    pub exti_current_alert: ExtiInput<'static>,

    /// The PWM for driving the tool's heating element.
    pub pwm_heater: SimplePwm<'static, peripherals::TIM1>,

    /// A pin for detecting the tool sleep position (in holder).
    pub pin_sleep: Input<'static>,
}

impl ToolResources {
    /// Initial setup of internal resources.
    pub fn init(&mut self) {
        self.dac_current_limit
            .set_mode(dac::Mode::NormalExternalBuffered);
        self.dac_current_limit.enable();
        self.set_peak_current_limit(ElectricCurrent::new::<electric_current::ampere>(
            PEAK_CURRENT_LIMIT_A,
        ));
    }

    /// Get the PWM heater channel.
    pub fn pwm_heater_channel(&mut self) -> PwmHeaterChannel<'_> {
        self.pwm_heater.ch1()
    }

    /// Set the PWM duty cycle ratio (from 0 to 1).
    pub fn set_pwm_duty_cycle(&mut self, ratio: f32) {
        let duty = ratio * self.pwm_heater_channel().max_duty_cycle() as f32;
        self.pwm_heater_channel().set_duty_cycle(duty as u16);
    }

    /// Set a peak current limit for the current-sense amplifier, by means of a DAC voltage output.
    pub fn set_peak_current_limit(&mut self, current: ElectricCurrent) {
        let dac_voltage = (sensors::DAC_MAX / sensors::VREFBUF_V
            * CURRENT_MONITOR_GAIN_V_PER_A
            * current.get::<electric_current::ampere>())
        .round();

        self.dac_current_limit
            .set(dac::Value::Bit12Right(dac_voltage as u16));
    }

    /// Check if a tip is inserted by passing a small test current.
    ///
    /// Use the current-sensor overcurrent interrupt for a quick evaluation.
    /// Before passing the test current, the current limit is set to just 100 mA.
    pub async fn test_tip_current(&mut self) -> Result<(), Error> {
        {
            // Set a low current limit.
            self.set_peak_current_limit(ElectricCurrent::new::<electric_current::milliampere>(
                100.0,
            ));

            // Enable heater power output at low duty cycle.
            self.set_pwm_duty_cycle(0.1);

            // React to overcurrent event. Times out if no tip is present.
            let tip_present = self
                .exti_current_alert
                .wait_for_low()
                .with_timeout(Duration::from_micros(100))
                .await
                .is_ok();

            self.pwm_heater_channel().set_duty_cycle_fully_off();

            // Restore original current limit.
            self.set_peak_current_limit(ElectricCurrent::new::<electric_current::ampere>(
                PEAK_CURRENT_LIMIT_A,
            ));

            if !tip_present {
                return Err(Error::NoTip);
            }

            Ok(())
        }
    }
}
