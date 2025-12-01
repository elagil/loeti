use crate::{data::DataManager, kit};
use eframe::egui;
use ergot::socket::topic::std_bounded::BoxedReceiverHandle;
use loeti_protocol::{MeasurementTopic, StatusTopic};

/// The application that plots PID and temperature data.
pub struct PlotApp {
    pub data: crate::data::DataManager,
    pub measurement_receiver: BoxedReceiverHandle<MeasurementTopic, kit::RouterStack>,
    pub status_receiver: BoxedReceiverHandle<StatusTopic, kit::RouterStack>,
}

impl PlotApp {
    /// Create a new plot application.
    pub fn new(_cc: &eframe::CreationContext<'_>, stack: kit::RouterStack) -> Self {
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
