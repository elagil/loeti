//! A library of supported tools (soldering irons).
use loeti_protocol::PidParameters;

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

        // Enable for calibrating/fitting manually.
        // defmt::info!("TC potential: {} V", tc_potential_v);

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
    /// PID parameters.
    pub pid_parameters: PidParameters,
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
        detect_ratio: 0.0, // 0 Ohm
        temperature_calibration: TemperatureCalibration {
            quadratic_c_per_vv: -7.9586e6,
            linear_c_per_v: 1.2239e5 ,
            constant_c: 26.932
        },

        pid_parameters: PidParameters {
            p: 0.04,
            i: 0.5,
            d: 0.0
        },
    },
    {
        id: JBC_T245,
        name: "JBC T245",
        max_power_w: 130.0,
        heater_resistance_ohm: 2.8,
        detect_ratio: 0.5, // 10 kOhm
        temperature_calibration: TemperatureCalibration {
            quadratic_c_per_vv: -6.972e4,
            linear_c_per_v: 3.7855e4,
            constant_c: 30.614,
        },

        pid_parameters: PidParameters {
            p: 0.2,
            i: 0.5,
            d: 0.0,
        },
    },
];
