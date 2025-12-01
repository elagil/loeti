//! Application for plotting live PID outputs and temperatures.
#![warn(missing_docs)]

use loeti_app::{
    app::PlotApp,
    comm::{manage_connections, ping_all},
    kit,
};
use log::{info, warn};

/// The main plot application entry point.
#[tokio::main]
async fn main() {
    env_logger::init();
    info!("Hi");

    let stack: kit::RouterStack = kit::RouterStack::new();

    tokio::task::spawn(ping_all(stack.clone()));
    tokio::task::spawn(manage_connections(stack.clone()));

    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport.min_inner_size = Some(eframe::egui::Vec2 { x: 900.0, y: 600.0 });
    eframe::run_native(
        "Löti control",
        native_options,
        Box::new(|cc| Ok(Box::new(PlotApp::new(cc, stack.clone())))),
    )
    .unwrap();
}
