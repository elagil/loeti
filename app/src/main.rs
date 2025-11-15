//! Application for plotting live PID outputs and temperatures.
#![warn(missing_docs)]

use eframe::egui::{self, Color32, FontId, RichText, Vec2b};
use egui_plot::{Legend, Line, LineStyle, Plot, PlotPoint, PlotPoints};
use ergot::{
    Address,
    socket::topic::std_bounded::BoxedReceiverHandle,
    toolkits::nusb_v0_1::{RouterStack, find_new_devices, register_router_interface},
    well_known::ErgotPingEndpoint,
};
use loeti_protocol::{ControlState, Measurement, MeasurementTopic, Status, StatusTopic, ToolState};
use log::{info, warn};
use tokio::time::{interval, sleep, timeout};

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

const MTU: u16 = 128;
const OUT_BUFFER_SIZE: usize = 1024;

/// Slices of data to plot.
struct DataSlices<'d> {
    outputs: &'d [PlotPoint],
    ps: &'d [PlotPoint],
    is: &'d [PlotPoint],
    ds: &'d [PlotPoint],
    temperatures_deg_c: &'d [PlotPoint],
    set_temperatures_deg_c: &'d [PlotPoint],
}

/// Manages data received from the device.
struct DataManager {
    plot_duration_s: f64,
    outputs: Vec<PlotPoint>,
    ps: Vec<PlotPoint>,
    is: Vec<PlotPoint>,
    ds: Vec<PlotPoint>,
    temperatures_deg_c: Vec<PlotPoint>,
    set_temperatures_deg_c: Vec<PlotPoint>,
    status: Status,
}

impl DataManager {
    fn new() -> Self {
        Self {
            plot_duration_s: 60.0,
            outputs: Vec::new(),
            ps: Vec::new(),
            is: Vec::new(),
            ds: Vec::new(),
            temperatures_deg_c: Vec::new(),
            set_temperatures_deg_c: Vec::new(),
            status: Default::default(),
        }
    }

    fn last_timestamp_s(&self) -> Option<f64> {
        self.temperatures_deg_c.last().map(|v| v.x)
    }

    fn temperature_deg_c(&self) -> Option<f64> {
        self.temperatures_deg_c.last().map(|v| v.y)
    }

    fn update_status(&mut self, status: Status) {
        self.status = status
    }

    fn push(&mut self, measurement: &Measurement) {
        let x = measurement.time_ms as f64 / 1000.0;

        if let Some(pid) = measurement.pid.as_ref() {
            self.outputs.push(PlotPoint {
                x,
                y: pid.output as f64,
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

    fn get(&mut self) -> Option<DataSlices<'_>> {
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

/// The application that plots PID and temperature data.
struct PlotApp {
    data: DataManager,
    measurement_receiver: BoxedReceiverHandle<MeasurementTopic, crate::RouterStack>,
    status_receiver: BoxedReceiverHandle<StatusTopic, crate::RouterStack>,
}

impl PlotApp {
    /// Create a new plot application.
    fn new(_cc: &eframe::CreationContext<'_>, stack: crate::RouterStack) -> Self {
        let measurement_receiver = Box::pin(
            stack
                .topics()
                .heap_bounded_receiver::<MeasurementTopic>(128, None),
        );
        let measurement_receiver = measurement_receiver.subscribe_boxed();

        let status_receiver = Box::pin(
            stack
                .topics()
                .heap_bounded_receiver::<StatusTopic>(128, None),
        );
        let status_receiver = status_receiver.subscribe_boxed();

        Self {
            data: DataManager::new(),
            measurement_receiver,
            status_receiver,
        }
    }
}

impl PlotApp {
    /// Plot PID outputs and temperatures.
    fn plot(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                let status_text = match &self.data.status.control_state {
                    ControlState::NoTool => "No tool".to_string(),
                    ControlState::NoTip => "No tip".to_string(),
                    ControlState::UnknownTool => "Unknown tool".to_string(),
                    ControlState::Tool(state) => match state {
                        ToolState::Active => "Tool detected".to_string(),
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

impl eframe::App for PlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(msg) = self.measurement_receiver.try_recv() {
            self.data.push(&msg.t);
        }

        if let Some(msg) = self.status_receiver.try_recv() {
            self.data.update_status(msg.t);
        }

        self.plot(ctx);

        ctx.request_repaint();
    }
}

/// The main plot application entry point.
#[tokio::main]
async fn main() {
    env_logger::init();
    info!("Hi");

    let stack: RouterStack = RouterStack::new();

    tokio::task::spawn(ping_all(stack.clone()));
    tokio::task::spawn(manage_connections(stack.clone()));

    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport.min_inner_size = Some(eframe::egui::Vec2 { x: 900.0, y: 600.0 });
    eframe::run_native(
        "Löti",
        native_options,
        Box::new(|cc| Ok(Box::new(PlotApp::new(cc, stack.clone())))),
    )
    .unwrap();
}

/// Check for new devices and register, if possible.
async fn manage_connections(stack: RouterStack) {
    let mut seen = HashSet::new();

    loop {
        let devices = find_new_devices(&HashSet::new()).await;

        for dev in devices {
            let info = dev.info.clone();
            info!("Found {:?}, registering", info);
            let _hdl = register_router_interface(&stack, dev, MTU, OUT_BUFFER_SIZE)
                .await
                .unwrap();
            seen.insert(info);
        }

        sleep(Duration::from_secs(3)).await;
    }
}

/// Ping all known devices.
async fn ping_all(stack: RouterStack) {
    let mut ival = interval(Duration::from_secs(1));
    let mut ctr = 0u32;
    // Attempt to remember the ping port
    let mut portmap: HashMap<u16, u8> = HashMap::new();

    loop {
        ival.tick().await;
        let nets = stack.manage_profile(|im| im.get_nets());
        info!("Nets to ping: {:?}", nets);
        for net in nets {
            let pg = ctr;
            ctr = ctr.wrapping_add(1);

            let addr = if let Some(port) = portmap.get(&net) {
                Address {
                    network_id: net,
                    node_id: 2,
                    port_id: *port,
                }
            } else {
                Address {
                    network_id: net,
                    node_id: 2,
                    port_id: 0,
                }
            };

            let start = Instant::now();
            let rr = stack
                .endpoints()
                .request_full::<ErgotPingEndpoint>(addr, &pg, None);
            let fut = timeout(Duration::from_millis(100), rr);
            let res = fut.await;
            let elapsed = start.elapsed();
            warn!("ping {}.2 w/ {}: {:?}, took: {:?}", net, pg, res, elapsed);
            if let Ok(Ok(msg)) = res {
                assert_eq!(msg.t, pg);
                portmap.insert(net, msg.hdr.src.port_id);
            } else {
                portmap.remove(&net);
            }
        }
    }
}
