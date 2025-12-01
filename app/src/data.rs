//! Management of data points.

use egui_plot::PlotPoint;
use loeti_protocol::{Measurement, Status};

/// Slices of data to plot.
pub struct DataSlices<'d> {
    pub outputs: &'d [PlotPoint],
    pub ps: &'d [PlotPoint],
    pub is: &'d [PlotPoint],
    pub ds: &'d [PlotPoint],
    pub temperatures_deg_c: &'d [PlotPoint],
    pub set_temperatures_deg_c: &'d [PlotPoint],
}

/// Manages data received from the device.
pub struct DataManager {
    pub status: Status,
    pub plot_duration_s: f64,
    outputs: Vec<PlotPoint>,
    ps: Vec<PlotPoint>,
    is: Vec<PlotPoint>,
    ds: Vec<PlotPoint>,
    temperatures_deg_c: Vec<PlotPoint>,
    set_temperatures_deg_c: Vec<PlotPoint>,
}

impl Default for DataManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DataManager {
    /// Create an empty instance.
    pub fn new() -> Self {
        Self {
            status: Default::default(),
            plot_duration_s: 60.0,
            outputs: Vec::new(),
            ps: Vec::new(),
            is: Vec::new(),
            ds: Vec::new(),
            temperatures_deg_c: Vec::new(),
            set_temperatures_deg_c: Vec::new(),
        }
    }

    /// Get the latest measurement timestamp.
    pub fn last_timestamp_s(&self) -> Option<f64> {
        self.temperatures_deg_c.last().map(|v| v.x)
    }

    /// Get the latest measured temperature in °C.
    pub fn temperature_deg_c(&self) -> Option<f64> {
        self.temperatures_deg_c.last().map(|v| v.y)
    }

    /// Update the status from the reported structure.
    pub fn update_status(&mut self, status: Status) {
        self.status = status
    }

    /// Push a new measurement.
    pub fn push(&mut self, measurement: &Measurement) {
        let x = measurement.time_ms as f64 / 1000.0;

        if let Some((pid, output)) = measurement.pid_state.as_ref() {
            self.outputs.push(PlotPoint {
                x,
                y: *output as f64,
            });
            self.ps.push(PlotPoint { x, y: pid.p as f64 });
            self.is.push(PlotPoint { x, y: pid.i as f64 });
            self.ds.push(PlotPoint { x, y: pid.d as f64 });
        } else {
            self.outputs.push(PlotPoint { x, y: f64::NAN });
            self.ps.push(PlotPoint { x, y: f64::NAN });
            self.is.push(PlotPoint { x, y: f64::NAN });
            self.ds.push(PlotPoint { x, y: f64::NAN });
        }

        self.temperatures_deg_c.push(PlotPoint {
            x,
            y: measurement.temperature_deg_c.unwrap_or(f32::NAN) as f64,
        });
        self.set_temperatures_deg_c.push(PlotPoint {
            x,
            y: measurement.set_temperature_deg_c.unwrap_or(f32::NAN) as f64,
        });
    }

    /// Get the current data slices.
    pub fn get(&mut self) -> Option<DataSlices<'_>> {
        let first_timestamp_s = self.last_timestamp_s()? - self.plot_duration_s;

        let mut start = None;
        for (rev_index, point) in self.temperatures_deg_c.iter().rev().enumerate() {
            let t = point.x;

            if t <= first_timestamp_s {
                start = Some(self.temperatures_deg_c.len() - rev_index - 1);
                break;
            }
        }

        let start = start.unwrap_or_default();

        Some(DataSlices {
            outputs: &self.outputs[start..],
            ps: &self.ps[start..],
            is: &self.is[start..],
            ds: &self.ds[start..],
            temperatures_deg_c: &self.temperatures_deg_c[start..],
            set_temperatures_deg_c: &self.set_temperatures_deg_c[start..],
        })
    }
}
