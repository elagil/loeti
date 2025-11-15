#![no_std]

use ergot::topic;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

/// PID information.
#[derive(Default, Clone, Schema, Serialize, Deserialize, Debug)]
pub struct Pid {
    pub output: f32,
    pub p: f32,
    pub i: f32,
    pub d: f32,
}

/// Periodic measurement data from the device.
#[derive(Default, Clone, Schema, Serialize, Deserialize, Debug)]
pub struct Measurement {
    pub time_ms: u64,
    pub pid: Option<Pid>,
    pub set_temperature_deg_c: Option<f32>,
    pub temperature_deg_c: Option<f32>,
}

// Device -> Host Measurement endpoint (no response expected)
topic!(MeasurementTopic, Measurement, "loeti/measurement");
