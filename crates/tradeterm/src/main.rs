//! tradeterm — native desktop charting on gpui.

mod app;
mod chart_settings;
mod custom_interval;
mod dialogs;
mod drawing_settings;
mod drawing_toolbar;
mod indicator_settings;
mod indicators;
mod menus;
mod search;
mod source;
mod status_bar;
mod title_bar;
mod top_toolbar;
mod ui;

use app::TradeTerm;
use gpui_kit::*;
use tt_session::runtime::BackgroundRuntime;
use tt_session::settings::SettingsStore;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let runtime = BackgroundRuntime::start().expect("failed to start tokio runtime");
    let (settings, store) = SettingsStore::load(SettingsStore::default_path());
    let synthetic = synthetic_arg(std::env::args().skip(1));

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            // Registers the Lucide icon font the chart canvas uses too
            // (`tt_chart_gpui::ICON_FONT` is the same family).
            tt_ui::init(cx);
            app::bind_keys(cx);
            let opts = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1440.), px(900.)),
                    cx,
                ))),
                window_min_size: Some(size(px(640.), px(400.))),
                ..gpui_kit::component::TitleBar::window_options()
            };
            let mut runtime = Some(runtime);
            let mut settings = Some(settings);
            let mut store = Some(store);
            let window = gpui_kit::open_window(opts, cx, move |window, cx| {
                cx.new(|cx| {
                    TradeTerm::new(
                        settings.take().expect("once"),
                        store.take().expect("once"),
                        runtime.take().expect("once"),
                        synthetic,
                        window,
                        cx,
                    )
                })
            })
            .expect("open window");
            let _ = window;
        });
}

/// `--synthetic [N]` starts with N synthetic bars (default 1,000,000).
fn synthetic_arg(mut args: impl Iterator<Item = String>) -> Option<usize> {
    while let Some(a) = args.next() {
        if a == "--synthetic" {
            let n = args.next().and_then(|n| n.replace('_', "").parse().ok());
            return Some(n.unwrap_or(tt_session::synthetic::DEFAULT_BARS));
        }
    }
    None
}
