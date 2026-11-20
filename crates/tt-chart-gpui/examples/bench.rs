//! Chart benchmark on gpui: 1M synthetic 1m bars + Volume, EMA 21, BB 20,
//! RSI 14, MACD at 1920×1040, driven every frame for `secs` seconds.
//!
//! `cargo run -p tt-chart-gpui --release --example bench -- hover [secs]`
//!   moves the crosshair (only the overlay should repaint);
//! `… -- pan [secs]` pans (both layers repaint), alternating zoomed out over
//!   all bars and 300 bars.
//!
//! Prints, per second: frames, and per layer paints and average CPU ms.

use tt_session::synthetic;

use egui::pos2;
use gpui_kit::*;
use std::time::{Duration, Instant};
use tt_chart::{ChartView, Indicator};
use tt_chart_gpui::{Chart, ChartState, LayerStats};
use tt_core::{Source, Study, StudyKind};
use tt_theme::Theme;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Hover,
    Pan,
    /// The chart, unchanged, redrawn every frame (gpui's per-frame cost).
    Static,
    /// An empty window redrawn every frame (baseline).
    Empty,
}

struct Bench {
    chart: Entity<Chart>,
    mode: Mode,
    secs: u64,
    started: Instant,
    frame: u64,
    phase: Option<u64>,
    window_start: Instant,
    window_frames: u32,
    last: (LayerStats, LayerStats),
    rows: Vec<[f64; 5]>,
}

impl Bench {
    fn report(&mut self, cx: &mut App) {
        let (base, overlay) = {
            let s = self.chart.read(cx).state.read(cx);
            (s.base_stats, s.overlay_stats)
        };
        let el = self.window_start.elapsed().as_secs_f64();
        if el < 1.0 {
            return;
        }
        let per = |now: LayerStats, prev: LayerStats| {
            let n = now.paints - prev.paints;
            let ms = (now.total - prev.total).as_secs_f64() * 1000.0;
            (n, if n > 0 { ms / n as f64 } else { 0.0 })
        };
        let (bn, bms) = per(base, self.last.0);
        let (on, oms) = per(overlay, self.last.1);
        let fps = self.window_frames as f64 / el;
        let label = match (self.mode, self.phase) {
            (Mode::Hover, _) => "hover",
            (Mode::Static, _) => "static chart",
            (Mode::Empty, _) => "empty window",
            (Mode::Pan, Some(0)) => "pan, zoomed out (1M bars)",
            _ => "pan, zoomed in (300 bars)",
        };
        println!(
            "[{label}] fps {fps:5.1} | base {bn:3} paints {bms:5.2} ms ({} prims) | overlay {on:3} paints {oms:5.2} ms ({} prims)",
            base.primitives, overlay.primitives
        );
        self.rows.push([fps, bn as f64, bms, on as f64, oms]);
        self.last = (base, overlay);
        self.window_start = Instant::now();
        self.window_frames = 0;
    }
}

impl Render for Bench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frame += 1;
        self.window_frames += 1;
        let size = window.viewport_size();
        let (w, h) = (f32::from(size.width), f32::from(size.height));
        let frame = self.frame;
        match self.mode {
            Mode::Hover => {
                // Sweep the crosshair across the price pane.
                let t = (frame % 600) as f32 / 600.0;
                let p = pos2(40.0 + t * (w - 160.0), h * 0.35);
                self.chart.update(cx, |c, cx| c.set_pointer(Some(p), cx));
            }
            Mode::Static | Mode::Empty => {}
            Mode::Pan => {
                let phase = (self.started.elapsed().as_secs() / 4) % 2;
                let switch = self.phase != Some(phase);
                self.phase = Some(phase);
                let dx = if (frame / 120).is_multiple_of(2) {
                    6.0
                } else {
                    -6.0
                };
                self.chart.update(cx, |c, cx| {
                    c.update_state(cx, |s| {
                        if switch {
                            if phase == 0 {
                                s.view.fit_all();
                            } else {
                                s.view.zoom_to_bars(300);
                            }
                        }
                        s.view.pan(dx);
                    });
                    c.set_pointer(Some(pos2(w * 0.5, h * 0.35)), cx);
                });
            }
        }
        self.report(cx);
        if self.started.elapsed() >= Duration::from_secs(self.secs) {
            let rows: Vec<_> = self.rows.iter().skip(1).collect();
            let n = rows.len().max(1) as f64;
            let avg = |k: usize| rows.iter().map(|r| r[k]).sum::<f64>() / n;
            println!(
                "SUMMARY fps {:.1} | base {:.1} paints/s {:.2} ms | overlay {:.1} paints/s {:.2} ms",
                avg(0),
                avg(1),
                avg(2),
                avg(3),
                avg(4)
            );
            cx.quit();
        }
        window.request_animation_frame();
        if self.mode == Mode::Empty {
            return div().size_full();
        }
        div().size_full().child(self.chart.clone())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = match args.get(1).map(String::as_str) {
        Some("pan") => Mode::Pan,
        Some("static") => Mode::Static,
        Some("empty") => Mode::Empty,
        _ => Mode::Hover,
    };
    let secs = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(16);
    let no_indicators = args.iter().any(|a| a == "noind");
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            tt_chart_gpui::init(cx);
            let series = synthetic::generate_minutes(synthetic::DEFAULT_BARS, 7);
            let opts = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1920.), px(1040.)),
                    cx,
                ))),
                ..Default::default()
            };
            cx.open_window(opts, move |_, cx| {
                let mut view = ChartView::default();
                if no_indicators {
                    // keep the default (none)
                } else {
                    view.indicators = vec![
                        Indicator::new(StudyKind::Volume.default_study()),
                        Indicator::new(Study::Ema {
                            length: 21,
                            source: Source::Close,
                        }),
                        Indicator::new(StudyKind::BollingerBands.default_study()),
                        Indicator::new(StudyKind::Rsi.default_study()),
                        Indicator::new(StudyKind::Macd.default_study()),
                    ];
                }
                if mode != Mode::Pan {
                    view.zoom_to_bars(300);
                }
                let mut state = ChartState::new(view, series, Theme::dark());
                state.title = "SYNTH · 1m · 1,000,000 bars".into();
                state.tick_size = Some(0.01);
                state.bar_interval_ms = Some(60_000);
                let chart = cx.new(|cx| Chart::new(state, cx));
                cx.new(|_| Bench {
                    chart,
                    mode,
                    secs,
                    started: Instant::now(),
                    frame: 0,
                    phase: None,
                    window_start: Instant::now(),
                    window_frames: 0,
                    last: (LayerStats::default(), LayerStats::default()),
                    rows: Vec::new(),
                })
            })
            .expect("window");
        });
}
