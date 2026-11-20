//! tradeterm — native desktop charting.

mod app;
mod runtime;
mod theme;

use app::TradeTermApp;
use runtime::BackgroundRuntime;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();

    let runtime = BackgroundRuntime::start().expect("failed to start tokio runtime");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("tradeterm")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "tradeterm",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(TradeTermApp::new(cc.egui_ctx.clone(), runtime)?))
        }),
    )
}
