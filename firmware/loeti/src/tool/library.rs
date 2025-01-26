#[derive(Clone, Copy, PartialEq)]
pub enum ToolType {
    JBCC210,
    JBCC245,
}

#[derive(Clone, Copy)]
pub struct TemperatureCalibration {
    slope_k_per_v: f32,
    offset_c: f32,
}

impl TemperatureCalibration {
    pub fn calc_temperature_c(&self, tc_potential_v: f32) -> f32 {
        self.slope_k_per_v * tc_potential_v + self.offset_c
    }
}

#[derive(Clone, Copy)]
pub struct ToolProperties {
    pub name: &'static str,
    pub tool_type: ToolType,
    pub max_current_a: f32,
    pub heater_resistance_ohm: f32,
    pub detect_ratio: f32,
    pub temperature_calibration: TemperatureCalibration,

    pub p: f32,
    pub i: f32, // In units of 1/(°C * ms)
    pub d: f32,
}

impl ToolProperties {
    pub const fn all() -> &'static [Self] {
        &[
            Self {
                name: "JBC C210",
                tool_type: ToolType::JBCC210,
                max_current_a: 1.0,
                heater_resistance_ohm: 2.0,
                detect_ratio: 0.7,
                temperature_calibration: TemperatureCalibration {
                    slope_k_per_v: 165.41,
                    offset_c: 4.4,
                },

                p: 0.025,
                i: 0.005,
                d: 0.0,
            },
            Self {
                name: "JBC C245",
                tool_type: ToolType::JBCC245,
                max_current_a: 6.0,
                heater_resistance_ohm: 2.5,
                detect_ratio: 0.5,
                temperature_calibration: TemperatureCalibration {
                    slope_k_per_v: 165.41,
                    offset_c: 4.4, // Compensates for heat up of the handle and cold junction itself - around 20 °C
                },

                p: 0.2,
                i: 0.25,
                d: 0.0,
            },
        ]
    }
}
