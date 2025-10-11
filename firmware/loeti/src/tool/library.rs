//! A library of supported tools (soldering irons).

/// Supported tool types.
#[derive(Clone, Copy, PartialEq)]
pub enum ToolType {
    /// The JBC C210.
    JbcC210,
    /// The JBC C245.
    JbcC245,
}

/// Temperature calibration settings.
#[derive(Clone, Copy)]
pub struct TemperatureCalibration {
    /// Conversion factor from thermocouple voltage to temperature.
    slope_k_per_v: f32,
    /// A temperature offset.
    offset_c: f32,
}

impl TemperatureCalibration {
    /// Calculate temperature from thermocouple voltage.
    pub fn calc_temperature_c(&self, tc_potential_v: f32) -> f32 {
        self.slope_k_per_v * tc_potential_v + self.offset_c
    }
}

/// Properties of a tool (soldering iron).
#[derive(Clone, Copy)]
pub struct ToolProperties {
    /// The tool's name.
    pub name: &'static str,
    /// The type of a tool.
    pub tool_type: ToolType,
    /// Maximum allowed current.
    pub max_current_a: f32,
    /// Heater resistance in Ohm.
    pub heater_resistance_ohm: f32,
    /// The detection ratio for distinguishing between tools.
    pub detect_ratio: f32,
    /// Temperature calibration settings.
    pub temperature_calibration: TemperatureCalibration,

    /// Temperature control P-value.
    pub p: f32,
    /// Temperature control I-value in units of 1/(°C * ms)
    pub i: f32,
    /// Temperature control D-value.
    pub d: f32,
}

impl ToolProperties {
    /// A list of all supported tools.
    pub const fn all() -> &'static [Self] {
        &[
            Self {
                name: "JBC C210",
                tool_type: ToolType::JbcC210,
                max_current_a: 1.0,
                heater_resistance_ohm: 2.0,
                detect_ratio: 0.7,
                temperature_calibration: TemperatureCalibration {
                    slope_k_per_v: 180.0,
                    offset_c: 4.4,
                },

                p: 0.025,
                i: 0.005,
                d: 0.0,
            },
            Self {
                name: "JBC C245",
                tool_type: ToolType::JbcC245,
                max_current_a: 6.0,
                heater_resistance_ohm: 2.5,
                detect_ratio: 0.5,
                temperature_calibration: TemperatureCalibration {
                    slope_k_per_v: 180.0,
                    offset_c: 4.4, // Compensates for heat up of the handle and cold junction itself - around 20 °C
                },

                p: 0.1,
                i: 0.125,
                d: 0.0,
            },
        ]
    }
}
