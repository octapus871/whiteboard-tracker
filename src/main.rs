mod app;
mod models;
mod rtl_text;
mod storage;
mod theme;

use app::WhiteboardApp;
use eframe::NativeOptions;
use storage::load_app_data;

fn main() -> eframe::Result<()> {
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("السبورة الذكية - Whiteboard Tracker")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    let data = load_app_data();

    eframe::run_native(
        "Whiteboard Tracker",
        native_options,
        Box::new(|cc| Ok(Box::new(WhiteboardApp::new(cc, data)))),
    )
}
