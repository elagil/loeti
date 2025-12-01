use eframe::egui::{self, Color32, FontId, RichText, Vec2b};
use egui_plot::{Legend, Line, LineStyle, Plot, PlotPoints};
use loeti_protocol::{ControlState, ToolState};

use crate::{app::PlotApp, data::DataManager};

impl PlotApp {
    /// Plot PID outputs and temperatures.
    pub fn plot(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                let status_text = match &self.data.status.control_state {
                    ControlState::NoTool => "No tool".to_string(),
                    ControlState::NoTip => "No tip".to_string(),
                    ControlState::UnknownTool => "Unknown tool".to_string(),
                    ControlState::Tool(state) => match state {
                        ToolState::Active(pid) => format!("Tool detected ({:?})", pid).to_string(),
                        ToolState::Sleeping => "Sleep".to_string(),
                        ToolState::InStand(since) => format!(
                            "In stand for {:.0} s",
                            (self.data.status.time_ms - since) as f64 / 1000.0
                        ),
                    },
                    ControlState::ToolMismatch => "Tool mismatch".to_string(),
                };

                ui.label(RichText::new(status_text).color(Color32::LIGHT_GREEN));

                ui.separator();

                ui.add(
                    egui::Slider::new(&mut self.data.plot_duration_s, 10.0..=3600.0)
                        .logarithmic(true)
                        .text("Duration to plot")
                        .suffix(" s"),
                );

                ui.separator();

                if ui.button("Clear").clicked() {
                    self.data = DataManager::new();
                }
            });

            ui.add_space(5.0);
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(5.0);

            ui.label(
                RichText::new(format!(
                    "{:.0} °C",
                    self.data.temperature_deg_c().unwrap_or_default()
                ))
                .font(FontId::proportional(40.0))
                .color(Color32::LIGHT_GREEN),
            );

            ui.add_space(5.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(slices) = self.data.get() {
                let temperature: Line<'_> =
                    Line::new("Current", PlotPoints::from(slices.temperatures_deg_c))
                        .width(3.0)
                        .color(Color32::LIGHT_GREEN);
                let set_temperature =
                    Line::new("Setpoint", PlotPoints::from(slices.set_temperatures_deg_c))
                        .style(LineStyle::Dashed { length: 10.0 })
                        .color(Color32::GRAY);

                let control_output = Line::new("Output", PlotPoints::from(slices.outputs))
                    .width(3.0)
                    .color(Color32::LIGHT_GRAY);
                let control_p = Line::new("P", PlotPoints::from(slices.ps));
                let control_i = Line::new("I", PlotPoints::from(slices.is));
                let control_d = Line::new("D", PlotPoints::from(slices.ds));

                let plt_height = ui.available_height() / 2.0;

                let link_group_id = ui.id().with("linked_plots");
                let link_axis = Vec2b::new(true, false);
                let link_cursor = Vec2b::new(true, false);

                Plot::new("pid_plot")
                    .legend(Legend::default())
                    .y_axis_label("PID control")
                    .height(plt_height)
                    .width(ui.available_width())
                    .link_axis(link_group_id, link_axis)
                    .link_cursor(link_group_id, link_cursor)
                    .set_margin_fraction([0.0, 0.2].into())
                    .show(ui, |plot_ui| {
                        plot_ui.line(control_output);
                        plot_ui.line(control_p);
                        plot_ui.line(control_i);
                        plot_ui.line(control_d);
                    });

                Plot::new("temperature_plot")
                    .legend(Legend::default())
                    .x_axis_label("Time / s")
                    .y_axis_label("Temperature / °C")
                    .height(plt_height)
                    .width(ui.available_width())
                    .link_axis(link_group_id, link_axis)
                    .link_cursor(link_group_id, link_cursor)
                    .set_margin_fraction([0.0, 0.2].into())
                    .show(ui, |plot_ui| {
                        plot_ui.line(temperature);
                        plot_ui.line(set_temperature);
                    });
            }
        });
    }
}
