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
        #[cfg(feature = "board_v6")]
        const GAIN: f32 = 221.0;
        #[cfg(feature = "board_v7")]
        const GAIN: f32 = 230.0;

        // Convert measured voltage to actual thermocouple voltage.
        //
        // `GAIN` is the thermocouple amplifier gain.
        let tc_potential_v = tc_potential_v / GAIN;

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
    /// Maximum allowed tool power in Watt.
    pub max_power_w: f32,
    /// Approximate heater resistance in Ohm.
    ///
    /// Used for initial setup of the current control loop.
    pub heater_resistance_ohm: f32,
    /// The detection ratio for distinguishing between tools.
    ///
    /// This is the voltage divider ratio of the tool identification resistor to ground,
    /// and the station's built-in 10k pull-up.
    pub detect_ratio: f32,
    /// Temperature calibration settings.
    pub temperature_calibration: TemperatureCalibration,

    /// Temperature control P-value in units of A / K.
    pub p: f32,
    /// Temperature control I-value in units of A / (K * ms).
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
        detect_ratio: 0.0, //0.31973, // 4.7k
        temperature_calibration: TemperatureCalibration {
            quadratic_c_per_vv: -7423.7,
            linear_c_per_v: 90912.0,
            constant_c: 51.865,
        },

        p: 0.02,
        i: 0.05,
        d: 0.0,
    },
    {
        id: JBC_T245,
        name: "JBC T245",
        max_power_w: 130.0,
        heater_resistance_ohm: 2.5,
        detect_ratio: 0.5, // 10k
        temperature_calibration: TemperatureCalibration {
            quadratic_c_per_vv: -69720.5,
            linear_c_per_v: 37855.0,
            constant_c: 30.614,
        },

        p: 0.1,
        i: 0.25,
        d: 0.0,
    },
];
