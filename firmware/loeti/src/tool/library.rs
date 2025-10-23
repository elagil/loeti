//! A library of supported tools (soldering irons).

/// Temperature calibration settings.
#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct TemperatureCalibration {
    /// Quadratic term for temperature calculation.
    quadratic_c_per_vv: f32,
    /// Linear term for temperature calculation.
    linear_c_per_v: f32,
    /// Constant term for temperature calculation.
    constant_c: f32,
}

impl TemperatureCalibration {
    /// Calculate temperature from thermocouple voltage.
    pub fn calc_temperature_c(&self, tc_potential_v: f32) -> f32 {
        self.quadratic_c_per_vv * tc_potential_v * tc_potential_v
            + self.linear_c_per_v * tc_potential_v
            + self.constant_c
    }
}

/// Properties of a tool (soldering iron).
#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct ToolProperties {
    /// The tool's name.
    pub name: &'static str,
    /// Maximum allowed power in Watt.
    pub max_power_w: f32,
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
    /// Calculate maximum supported current, based on available voltage.
    pub fn max_current_a(&self, potential_v: f32) -> f32 {
        self.max_power_w / potential_v
    }
}

/// Make sure that all tools have unique `id` fields. Avoids accidental duplicates.
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
        id: JBC_T210,
        name: "JBC T210",
        max_power_w: 60.0,
        heater_resistance_ohm: 2.0,
        detect_ratio: 0.7,
        temperature_calibration: TemperatureCalibration {
            // FIXME: Values are not tested.
            quadratic_c_per_vv: 3.89,
            linear_c_per_v: 150.0,
            constant_c: 47.1,
        },

        p: 0.025,
        i: 0.005,
        d: 0.0,
    },
    {
        id: JBC_T245,
        name: "JBC T245",
        max_power_w: 130.0,
        heater_resistance_ohm: 2.5,
        detect_ratio: 0.5,
        temperature_calibration: TemperatureCalibration {
            quadratic_c_per_vv: -1.4275,
            linear_c_per_v: 171.29,
            constant_c: 30.614,
        },

        p: 0.15,
        i: 2.0,
        d: 0.0,
    },
];
