#![windows_subsystem = "windows"]

mod app;
mod battery;
mod power;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("BattGuard")
            .with_inner_size([540.0, 680.0])
            .with_min_inner_size([440.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        "battguard",
        options,
        Box::new(|cc| Ok(Box::new(app::BattApp::new(cc)))),
    )
}
