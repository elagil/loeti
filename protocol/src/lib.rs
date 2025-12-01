//! Protocol for communication between embedded device and host.
#![no_std]
#![warn(missing_docs)]

use ergot::topic;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

/// PID information.
///
/// Can be used for setup or reporting.
#[derive(Clone, Copy, Schema, Serialize, Deserialize, Debug, defmt::Format)]
pub struct PidParameters {
    /// PID P-component.
    pub p: f32,
    /// PID I-component.
    pub i: f32,
    /// PID D-component.
    pub d: f32,
}

/// Periodic measurement data from the device.
#[derive(Default, Clone, Schema, Serialize, Deserialize, Debug)]
pub struct Measurement {
    /// Timestamp in milliseconds of the measurement.
    pub time_ms: u64,
    /// PID control components and total output.
    pub pid_state: Option<(PidParameters, f32)>,
    /// The set temperature in °C.
    pub set_temperature_deg_c: Option<f32>,
    /// The current tool temperature in °C.
    pub temperature_deg_c: Option<f32>,
}

/// The state of the tool.
#[derive(Clone, Schema, Serialize, Deserialize, Debug)]
pub enum ToolState {
    /// The tool is active.
    ///
    /// Also provide current PID parameters for the tool.
    Active(PidParameters),
    /// The tool was placed in its stand at the recorded timestamp in ms.
    InStand(u64),
    /// The tool was automatically switched to sleep mode.
    Sleeping,
}

/// State of the iron control.
#[derive(Default, Clone, Schema, Serialize, Deserialize, Debug)]
pub enum ControlState {
    /// No tool is present.
    #[default]
    NoTool,
    /// A tool is present, but no tip (by thermocouple).
    NoTip,
    /// The tool is not known.
    UnknownTool,
    /// Tool mismatch during execution of the control loop.
    ToolMismatch,
    /// A tool is present (with state).
    Tool(ToolState),
}

/// Device status info.
#[derive(Default, Clone, Schema, Serialize, Deserialize, Debug)]
pub struct Status {
    /// Timestamp in milliseconds of the status message.
    pub time_ms: u64,
    /// The state of the tool control.
    pub control_state: ControlState,
}

// Measurement topic
topic!(MeasurementTopic, Measurement, "topic/measurement");

// Status topic
topic!(StatusTopic, Status, "topic/status");
