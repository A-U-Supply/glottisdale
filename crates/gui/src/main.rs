//! Glottisdale GUI — egui-based interface for syllable-level audio processing.

mod app;
mod editor;

use std::sync::Arc;

use eframe::egui;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    // Load and decode the app icon
    let icon_bytes = include_bytes!("../assets/icon.jpg");
    let icon_image = image::load_from_memory(icon_bytes)
        .expect("Failed to decode icon image")
        .to_rgba8();
    let (w, h) = icon_image.dimensions();
    let icon_data = egui::IconData {
        rgba: icon_image.into_raw(),
        width: w,
        height: h,
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([800.0, 500.0])
            .with_icon(Arc::new(icon_data)),
        ..Default::default()
    };

    eframe::run_native(
        "Glottisdale",
        options,
        Box::new(|cc| Ok(Box::new(app::GlottisdaleApp::new(cc)))),
    )
}
