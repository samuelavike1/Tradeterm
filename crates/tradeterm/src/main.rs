//! tradeterm — native desktop charting.

mod app;
mod chrome;
mod fonts;
mod icons;
mod perf;
mod runtime;
mod search;
mod settings;
mod synthetic;
mod tab;
mod tfmenu;
mod toolbar;

use app::TradeTermApp;
use runtime::BackgroundRuntime;
use settings::SettingsStore;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();

    let runtime = BackgroundRuntime::start().expect("failed to start tokio runtime");
    let (settings, store) = SettingsStore::load(SettingsStore::default_path());
    let synthetic = synthetic_arg(std::env::args().skip(1));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("tradeterm")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0])
            // We draw our own title bar with tabs and window buttons.
            .with_decorations(false),
        ..Default::default()
    };

    eframe::run_native(
        "tradeterm",
        options,
        Box::new(move |cc| {
            let bold_family = fonts::install(&cc.egui_ctx);
            Ok(Box::new(TradeTermApp::new(
                cc.egui_ctx.clone(),
                runtime,
                settings,
                store,
                synthetic,
                bold_family,
            )?))
        }),
    )
}

/// `--synthetic [N]` starts with N synthetic bars (default 1,000,000).
fn synthetic_arg(mut args: impl Iterator<Item = String>) -> Option<usize> {
    while let Some(a) = args.next() {
        if a == "--synthetic" {
            let n = args.next().and_then(|n| n.replace('_', "").parse().ok());
            return Some(n.unwrap_or(synthetic::DEFAULT_BARS));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::synthetic_arg;

    fn args(v: &[&str]) -> impl Iterator<Item = String> {
        v.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn parses_synthetic_flag() {
        assert_eq!(synthetic_arg(args(&[])), None);
        assert_eq!(synthetic_arg(args(&["--synthetic"])), Some(1_000_000));
        assert_eq!(
            synthetic_arg(args(&["--synthetic", "250_000"])),
            Some(250_000)
        );
    }
}
