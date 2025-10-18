//! A library of supported tools (soldering irons).

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

macro_rules! unique_items {
    // Main form: explicit `id`, and all fields as key: value pairs
    ( $( { id: $id:ident, $($field:ident : $value:expr),* $(,)? }),* $(,)?) => {{
        // Compile-time duplicate detection (E0428 on duplicate `id`)
        const _: () = { $( #[allow(dead_code)] const $id: () = ();)* };
        &[ $( ToolProperties { $($field : $value,)* },)* ]
    }};

    // Fallback to improve error messages
    ( $($tt:tt)* ) => {
        compile_error!(
            "unique_items! expects entries like:
             { id: <ident>, field1: <expr>, field2: <expr>, ... }"
        );
    };
}

/// List of all supported tools.
pub const TOOLS: &[ToolProperties] = unique_items![
    {
        id: JBC_C210,
        name: "JBC C210",
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
    {
        id: JBC_C245,
        name: "JBC C245",
        max_current_a: 6.0,
        heater_resistance_ohm: 2.5,
        detect_ratio: 0.5,
        temperature_calibration: TemperatureCalibration {
            slope_k_per_v: 180.0,
            offset_c: 4.4,
        },

        p: 0.1,
        i: 0.125,
        d: 0.0,
    },
];
