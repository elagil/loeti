//! Perform measurements on the tool (using ADC).

use super::{Error, ToolProperties};
use defmt::trace;
use embassy_stm32::Peri;
use embassy_stm32::{adc, peripherals};
use uom::ConstZero;
use uom::si::electric_potential;
use uom::si::electric_potential::volt;
use uom::si::electrical_resistance::ohm;
use uom::si::f32::ElectricCurrent;
use uom::si::f32::ElectricPotential;
use uom::si::f32::ElectricalResistance;
use uom::si::f32::Power;
use uom::si::f32::Ratio;
use uom::si::f32::ThermodynamicTemperature;
use uom::si::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;

/// ADC max. value (16 bit).
pub const ADC_MAX: f32 = 65535.0;
/// ADC sample time for temperature in cycles.
pub const ADC_SAMPLE_TIME_TEMP: adc::SampleTime = adc::SampleTime::CYCLES92_5;
/// ADC sample time for voltage/current in cycles.
pub const ADC_SAMPLE_TIME_CURRENT: adc::SampleTime = adc::SampleTime::CYCLES640_5;

/// The ADC reference voltage.
pub const VREFBUF_V: f32 = 2.9;
/// The analog supply voltage.
pub const ANALOG_SUPPLY_V: f32 = 3.3;
/// The value at which an ADC voltage is considered to be at the upper limit.
pub const MAX_ADC_V: f32 = VREFBUF_V - 0.1;
/// The ratio between the defined maximum ADC voltage and analog supply voltage.
pub const MAX_ADC_RATIO: f32 = MAX_ADC_V / ANALOG_SUPPLY_V;

/// Convert an ADC value to measured voltage.
fn adc_value_to_potential(value: u16) -> ElectricPotential {
    ElectricPotential::new::<volt>(VREFBUF_V * (value as f32) / ADC_MAX)
}

/// A tool's raw measurements.
#[derive(Clone, Copy)]
pub(super) struct RawToolMeasurement {
    /// The result of measuring the detection circuit.
    ///
    /// The detection ratio is used for assigning a certain tool from the library of supported tools.
    pub(super) detect_ratio: Ratio,
    /// The raw thermocouple voltage.
    pub(super) temperature_potential: ElectricPotential,
}

impl RawToolMeasurement {
    /// Derive a tool's temperature, given its unique properties.
    ///
    /// The temperature is invalid if the ADC voltage is zero or below. The hardware cannot measure negative
    /// thermocouple voltages, thus reports invalid temperature measurements in such cases.
    pub(super) fn temperature(
        &self,
        tool_properties: &ToolProperties,
    ) -> Option<ThermodynamicTemperature> {
        if self.temperature_potential <= ElectricPotential::ZERO {
            return None;
        }

        Some(ThermodynamicTemperature::new::<degree_celsius>(
            tool_properties
                .temperature_calibration
                .calc_temperature_c(self.temperature_potential.get::<volt>()),
        ))
    }
}

/// A tool power measurement.
pub(super) struct PowerMeasurement {
    /// The electric current through the tool.
    pub(super) current: ElectricCurrent,
    /// The supply voltage.
    ///
    /// FIXME: Use for checking drop from negotiated voltage?
    _potential: ElectricPotential,
}

impl PowerMeasurement {
    /// Calculate the tool's power dissipation.
    pub(super) fn _power(&self) -> Power {
        self._potential * self.current
    }

    /// Compensate current with an idle power measurement.
    pub(super) fn compensate(&mut self, idle: &Self) {
        self.current = (self.current - idle.current).max(ElectricCurrent::ZERO);
    }
}

/// Resources for the ADC.
pub struct AdcResources {
    /// The ADC.
    pub adc: adc::Adc<'static, peripherals::ADC1>,
    /// The ADC temperature input pin.
    pub pin_temperature: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The ADC detection input pin.
    pub pin_detect: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The ADC input for voltage on the bus.
    pub pin_voltage: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The ADC input for heater current.
    pub pin_current: adc::AnyAdcChannel<peripherals::ADC1>,
    /// The DMA for the ADC.
    pub adc_dma: Peri<'static, peripherals::DMA1_CH6>,
}

impl AdcResources {
    /// Take raw measurements of a tool.
    ///
    /// When the tool properties are known, they can be translated to useful values (e.g. temperature).
    pub(super) async fn measure_tool(&mut self) -> Result<RawToolMeasurement, Error> {
        let mut adc_buffer = [0u16; 2];

        self.adc
            .read(
                self.adc_dma.reborrow(),
                [
                    (&mut self.pin_detect, ADC_SAMPLE_TIME_TEMP),
                    (&mut self.pin_temperature, ADC_SAMPLE_TIME_TEMP),
                ]
                .into_iter(),
                &mut adc_buffer,
            )
            .await;

        trace!("Measured tool, ADC values: {}", adc_buffer);

        let detect_ratio = adc_value_to_potential(adc_buffer[0])
            / ElectricPotential::new::<electric_potential::volt>(ANALOG_SUPPLY_V);
        let temperature_potential = adc_value_to_potential(adc_buffer[1]);

        if detect_ratio > Ratio::new::<ratio::ratio>(MAX_ADC_RATIO) {
            Err(Error::NoTool)
        } else {
            Ok(RawToolMeasurement {
                detect_ratio,
                temperature_potential,
            })
        }
    }

    /// Measure the tool's power (voltage and current).
    pub(super) async fn measure_tool_power(&mut self) -> PowerMeasurement {
        let mut adc_buffer = [0u16; 2];

        self.adc
            .read(
                self.adc_dma.reborrow(),
                [
                    (&mut self.pin_current, ADC_SAMPLE_TIME_CURRENT),
                    (&mut self.pin_voltage, ADC_SAMPLE_TIME_CURRENT),
                ]
                .into_iter(),
                &mut adc_buffer,
            )
            .await;

        let current_sense_resistance = ElectricalResistance::new::<ohm>(0.2);
        let current = adc_value_to_potential(adc_buffer[0]) / current_sense_resistance;

        const VOLTAGE_DIVIDER_RATIO: f32 = 7.667;
        let potential = VOLTAGE_DIVIDER_RATIO * adc_value_to_potential(adc_buffer[1]);

        PowerMeasurement {
            current,
            _potential: potential,
        }
    }
}
