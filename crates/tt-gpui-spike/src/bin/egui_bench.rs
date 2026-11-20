//! The same benchmark as `tt-gpui-spike --bench` on the egui/eframe stack
//! (the current app's renderer), for an apples-to-apples comparison.

#[allow(dead_code)]
#[path = "../../../tradeterm/src/fonts.rs"]
mod fonts;
#[allow(dead_code)]
#[path = "../../../tradeterm/src/icons.rs"]
mod icons;
#[allow(dead_code)]
#[path = "../../../tradeterm/src/synthetic.rs"]
mod synthetic;

use egui::pos2;
use std::time::Instant;
use tt_chart::{ChartData, ChartView, Indicator};
use tt_core::{CandleSeries, Source, Study, StudyKind};
use tt_theme::Theme;

struct App {
    chart: ChartView,
    series: CandleSeries,
    started: Instant,
    secs: u64,
    frame: u64,
    phase: Option<u64>,
    // per-second accumulators
    win_start: Instant,
    frames: u32,
    show_ms: f64,
    cpu_ms: f64,
    worst_ms: f64,
    last_frame: Option<Instant>,
    history: Vec<(f64, f64, f64, f64)>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // cpu_usage: the previous frame's total CPU (UI + tessellation).
        if let Some(cpu) = frame.info().cpu_usage {
            self.cpu_ms += cpu as f64 * 1000.0;
        }
        let now = Instant::now();
        if let Some(l) = self.last_frame {
            self.worst_ms = self.worst_ms.max((now - l).as_secs_f64() * 1000.0);
        }
        self.last_frame = Some(now);
        self.frame += 1;
        let phase = (self.started.elapsed().as_secs() / 4) % 2;
        if self.phase != Some(phase) {
            self.phase = Some(phase);
            if phase == 0 {
                self.chart.fit_all();
            } else {
                self.chart.zoom_to_bars(300);
            }
        }
        let dx = if (self.frame / 120).is_multiple_of(2) {
            6.0
        } else {
            -6.0
        };
        self.chart.pan(dx);
        // Fake the crosshair by moving the pointer is not possible here; the
        // crosshair is a few primitives either way.
        let theme = Theme::dark();
        let t0 = Instant::now();
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let data = ChartData {
                series: &self.series,
                title: "SYNTH · 1m · 1,000,000 bars",
                tick_size: Some(0.01),
                now_ms: None,
                bar_interval_ms: Some(60_000),
                bar_close_ms: None,
            };
            self.chart.show(ui, data, &theme);
        });
        self.show_ms += t0.elapsed().as_secs_f64() * 1000.0;
        self.frames += 1;
        let el = self.win_start.elapsed().as_secs_f64();
        if el >= 1.0 {
            let f = self.frames as f64;
            let row = (f / el, self.show_ms / f, self.cpu_ms / f, self.worst_ms);
            let label = if phase == 0 {
                "zoomed out: 1M bars decimated"
            } else {
                "zoomed in: 300 bars"
            };
            println!(
                "[{label}] fps {:6.1}  show {:5.2} ms  frame cpu {:5.2} ms  worst frame {:6.1} ms",
                row.0, row.1, row.2, row.3
            );
            self.history.push(row);
            self.win_start = Instant::now();
            self.frames = 0;
            self.show_ms = 0.0;
            self.cpu_ms = 0.0;
            self.worst_ms = 0.0;
        }
        if self.started.elapsed().as_secs() >= self.secs {
            let h: Vec<_> = self.history.iter().skip(1).collect();
            let n = h.len().max(1) as f64;
            println!(
                "SUMMARY fps {:.1}  show {:.2} ms  frame cpu {:.2} ms  worst frame {:.1} ms",
                h.iter().map(|r| r.0).sum::<f64>() / n,
                h.iter().map(|r| r.1).sum::<f64>() / n,
                h.iter().map(|r| r.2).sum::<f64>() / n,
                h.iter().map(|r| r.3).fold(0.0, f64::max)
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
        let _ = pos2(0.0, 0.0);
    }
}

fn main() -> eframe::Result {
    let secs = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);
    let series = synthetic::generate_minutes(synthetic::DEFAULT_BARS, 7);
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1920.0, 1040.0]),
        ..Default::default()
    };
    eframe::run_native(
        "egui bench",
        opts,
        Box::new(move |cc| {
            let bold = fonts::install(&cc.egui_ctx);
            let mut chart = ChartView::default();
            chart.style.bold_family = bold;
            chart.style.icon_family = Some(icons::family());
            chart.indicators = vec![
                Indicator::new(StudyKind::Volume.default_study()),
                Indicator::new(Study::Ema {
                    length: 21,
                    source: Source::Close,
                }),
                Indicator::new(StudyKind::BollingerBands.default_study()),
                Indicator::new(StudyKind::Rsi.default_study()),
                Indicator::new(StudyKind::Macd.default_study()),
            ];
            Ok(Box::new(App {
                chart,
                series,
                started: Instant::now(),
                secs,
                frame: 0,
                phase: None,
                win_start: Instant::now(),
                frames: 0,
                show_ms: 0.0,
                cpu_ms: 0.0,
                worst_ms: 0.0,
                last_frame: None,
                history: Vec::new(),
            }))
        }),
    )
}
