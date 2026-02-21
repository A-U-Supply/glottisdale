//! Glottisdale GUI — egui-based interface for syllable-level audio processing.

mod app;
mod editor;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Glottisdale",
        options,
        Box::new(|cc| Ok(Box::new(app::GlottisdaleApp::new(cc)))),
    )
}
