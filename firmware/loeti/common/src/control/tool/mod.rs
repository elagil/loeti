//! Control of a connected tool.

use core::f32;

use super::{AutoSleep, Supply, TEMPERATURE_CONTROL_LOOP_PERIOD_MS, TOOLS, ToolProperties};
use defmt::{Format, debug, info, trace};
use embassy_time::{Duration, Instant};
use pid::{self, Pid};
use uom::si::electric_current;
use uom::si::f32::Ratio;
use uom::si::ratio;
use uom::si::ratio::percent;
use uom::si::thermodynamic_temperature;

pub mod resources;
pub mod sensors;

/// Errors related to tool detection.

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

/// The state of the tool.
#[derive(Debug, Clone, Copy, Format)]
pub enum ToolState {
    /// The tool is active.
    Active,
    /// The tool was placed in its stand at the recorded instant.
    InStand(Instant),
    /// The tool was automatically switched to sleep mode.
    Sleeping,
}

/// A tool (soldering iron).
pub struct Tool {
    /// Unique properties of the tool.
    pub(super) properties: &'static ToolProperties,
    /// The temperature control.
    pub(super) temperature_pid: Pid<f32>,
    /// The current control.
    pub(super) current_pid: Pid<f32>,
    /// The current  PWM ratio of the heater switch (MOSFET).
    pub(super) pwm_ratio: Ratio,
    /// The current temperature.
    ///
    /// Can be `None`, if the ADC reading was invalid.
    pub(super) temperature_deg_c: Option<f32>,
    /// The tool supply's characteristics.
    pub(super) supply: Supply,
    /// The state of the tool.
    pub(super) state: ToolState,
}

impl Tool {
    /// Create a new tool from a measurement.
    ///
    /// Limits the tool's current capability to the maximum available supply current.
    pub(super) fn new(
        tool_measurement: sensors::RawToolMeasurement,
        supply: Supply,
    ) -> Result<Self, Error> {
        let properties = Tool::detect(tool_measurement)?;
        info!("New tool with properties: {:?}", properties);

        let mut tool = Self {
            properties,
            temperature_pid: Pid::new(0.0, 0.0),
            current_pid: Pid::new(0.0, 1.0),
            pwm_ratio: Ratio::new::<percent>(0.0),
            temperature_deg_c: None,
            supply,
            state: ToolState::Active,
        };

        let gain = {
            // Use a scale between 0.3 (fast) and 0.1 (slow).
            const SCALE: f32 = 0.2;
            let gain = SCALE * properties.heater_resistance_ohm / tool.supply.potential_v();
            debug!(
                "Using current loop I-gain of {} (for {} V, {} Ω, scale {})",
                gain,
                tool.supply.potential_v(),
                properties.heater_resistance_ohm,
                SCALE
            );

            gain
        };

        tool.current_pid.i(gain, f32::INFINITY);

        Ok(tool)
    }

    /// If true, the tool is in its stand.
    pub(super) fn in_stand(&self) -> bool {
        !matches!(self.state, ToolState::Active)
    }

    /// Update the tool's state, based on whether it is currently in its stand, and the auto sleep duration.
    pub(super) fn update_tool_state(&mut self, in_stand: bool, auto_sleep: AutoSleep) -> ToolState {
        if !in_stand {
            self.state = ToolState::Active;
        } else if matches!(self.state, ToolState::Active) {
            debug!("Tool in stand");
            self.state = match auto_sleep {
                AutoSleep::AfterDurationS(0) => ToolState::Sleeping,
                _ => ToolState::InStand(Instant::now()),
            };
        } else if let ToolState::InStand(instant) = self.state {
            self.state = match auto_sleep {
                AutoSleep::Never => self.state,
                AutoSleep::AfterDurationS(duration_s) => {
                    if let Some(passed_duration) = Instant::now().checked_duration_since(instant)
                        && passed_duration >= Duration::from_secs(duration_s as u64)
                    {
                        ToolState::Sleeping
                    } else {
                        self.state
                    }
                }
            };
        }

        self.state
    }

    /// The tool's name.
    pub(super) fn name(&self) -> &'static str {
        self.properties.name
    }

    /// Calculate the tool's power limit in W, including effective power limit from the supply.
    #[allow(unused)]
    pub(super) fn power_limit_w(&self) -> f32 {
        self.properties
            .max_power_w
            .min(self.supply.effective_power_limit_w())
    }

    /// Calculate the tool's current limit in Ampere, including effective current limit from the supply.
    ///
    /// Takes into account
    /// - the tool's maximum allowed power,
    /// - the max. supply current, and
    /// - the supply potential.
    fn current_limit_a(&self) -> f32 {
        self.properties
            .max_current_a(self.supply.potential_v())
            .min(self.supply.effective_current_limit_a())
    }

    /// Detect a tool, based on a measurement.
    fn detect(
        tool_measurement: sensors::RawToolMeasurement,
    ) -> Result<&'static ToolProperties, Error> {
        const DETECTION_RATIO_ABS_TOLERANCE: f32 = 0.05;

        for tool_properties in TOOLS {
            if (tool_measurement.detect_ratio.get::<ratio::ratio>() - tool_properties.detect_ratio)
                .abs()
                < DETECTION_RATIO_ABS_TOLERANCE
            {
                return Ok(tool_properties);
            }
        }

        Err(Error::UnknownTool)
    }

    /// Calculate tool temperature from a raw tool measurement.
    ///
    /// Checks for unexpected tool changes during the control cycle.
    pub(super) fn detect_and_calculate_temperature(
        &mut self,
        tool_measurement: sensors::RawToolMeasurement,
    ) -> Result<(), Error> {
        let new_properties = Tool::detect(tool_measurement)?;
        if new_properties.name != self.name() {
            return Err(Error::ToolMismatch);
        }

        self.temperature_deg_c = tool_measurement
            .temperature(self.properties)
            .map(|v| v.get::<thermodynamic_temperature::degree_celsius>());

        Ok(())
    }

    /// Runs a temperature control step.
    pub(super) fn run_temperature_control(
        &mut self,
        set_temperature_deg_c: f32,
    ) -> Result<loeti_protocol::Measurement, Error> {
        let current_limit_a = self.current_limit_a();
        self.temperature_pid.output_limit = current_limit_a;

        if self.temperature_pid.setpoint != set_temperature_deg_c {
            self.temperature_pid.reset_integral_term();
            self.temperature_pid.setpoint(set_temperature_deg_c);
        }

        // Assume 0°C, if the measurement was invalid (e.g. negative thermocouple voltage).
        let control_output = self
            .temperature_pid
            .next_control_output(self.temperature_deg_c.unwrap_or_default());

        let current_setpoint_a = control_output.output;
        self.current_pid.setpoint(current_setpoint_a);

        const UNLIMITED_OUTPUT: f32 = f32::INFINITY;

        let pid_parameters = &self.properties.pid_parameters;
        self.temperature_pid
            .p(pid_parameters.p, UNLIMITED_OUTPUT)
            .d(
                pid_parameters.d * TEMPERATURE_CONTROL_LOOP_PERIOD_MS as f32,
                UNLIMITED_OUTPUT,
            );

        // The I-component is capped at the current limit to avoid excessive windup.
        let is_current_limited = current_setpoint_a >= current_limit_a || current_setpoint_a == 0.0;
        if is_current_limited {
            self.temperature_pid.i(0.0, current_limit_a);
        } else {
            self.temperature_pid.i(
                pid_parameters.i / TEMPERATURE_CONTROL_LOOP_PERIOD_MS as f32,
                current_limit_a,
            );
        }

        // Mitigate downward setpoint steps to cause undershoot.
        if control_output.output <= 0.0 && control_output.i < 0.0 {
            self.temperature_pid.reset_integral_term();
        }

        trace!(
            "Temperature control, current limited={}: P {}, I {}, D {} => {} A",
            is_current_limited,
            control_output.p,
            control_output.i,
            control_output.d,
            current_setpoint_a
        );

        Ok(loeti_protocol::Measurement {
            time_ms: Instant::now().as_millis(),
            pid_state: Some((
                loeti_protocol::PidParameters {
                    p: control_output.p,
                    i: control_output.i,
                    d: control_output.d,
                },
                control_output.output,
            )),
            set_temperature_deg_c: Some(set_temperature_deg_c),
            temperature_deg_c: self.temperature_deg_c,
        })
    }

    /// The ratio of the current setpoint and the maximum current that can be practically supplied.
    ///
    /// This is a measure for relative output power, referred to the tool, not only the supply.
    #[allow(unused)]
    pub(super) fn power_ratio(&self) -> f32 {
        (self.current_pid.setpoint / self.current_limit_a()).max(0.0)
    }

    /// The PWM ratio to use for driving the heater.
    pub(super) fn pwm_ratio(&self) -> f32 {
        self.pwm_ratio.get::<ratio::ratio>()
    }

    /// Runs a power control step.
    pub(super) fn run_current_control(
        &mut self,
        power_measurement: &sensors::ToolPowerMeasurement,
    ) {
        let control_output = self
            .current_pid
            .next_control_output(power_measurement.current.get::<electric_current::ampere>());

        // Mitigate downward setpoint steps to cause undershoot.
        if control_output.output <= 0.0 && control_output.i < 0.0 {
            self.current_pid.reset_integral_term();
        }

        self.pwm_ratio = Ratio::new::<ratio::ratio>(control_output.output);
    }

    /// Get the current temperature PID parameters.
    #[allow(unused)]
    pub(super) fn get_temperature_pid_parameters(&self) -> loeti_protocol::PidParameters {
        let pid_parameters = &self.properties.pid_parameters;
        loeti_protocol::PidParameters {
            p: pid_parameters.p,
            i: pid_parameters.i,
            d: pid_parameters.d,
        }
    }
}
