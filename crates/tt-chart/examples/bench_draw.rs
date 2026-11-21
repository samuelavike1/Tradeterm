//! Headless chart benchmark: 1,000,000 bars through the real `ChartView`,
//! without any UI toolkit.
//!
//! `cargo run --release -p tt-chart --example bench_draw [-- BARS]`
//!
//! Each frame runs `ChartView::render_base` and `render_overlay` (layout,
//! caches, decimation, every draw call) into a canvas that only counts
//! primitives, so the numbers are the chart's own CPU cost: what any backend
//! pays before it hands primitives to the GPU. The last bar is updated every
//! frame, as a live feed would, so incremental cache updates are measured
//! too. The pointer sits over the plot so the crosshair is drawn.

use ecolor::Color32;
use emath::{Align2, Pos2, Rect, Vec2, pos2, vec2};
use std::time::{Duration, Instant};
use tt_chart::render::Canvas;
use tt_chart::{ChartData, ChartType, ChartView, Indicator, RenderInput};
use tt_core::{Candle, CandleSeries, StudyKind};
use tt_theme::Theme;

const FRAMES: usize = 60;

/// Counts primitives; text is measured with a fixed-width estimate.
#[derive(Default)]
struct CountingCanvas {
    prims: usize,
}

impl Canvas for CountingCanvas {
    fn pixels_per_point(&self) -> f32 {
        1.0
    }
    fn set_clip(&mut self, _: Rect) {}
    fn fill_rect(&mut self, _: Rect, _: Color32) {
        self.prims += 1;
    }
    fn fill_rounded_rect(&mut self, _: Rect, _: f32, _: Color32) {
        self.prims += 1;
    }
    fn stroke_rect(&mut self, _: Rect, _: f32, _: Color32) {
        self.prims += 1;
    }
    fn line(&mut self, _: Pos2, _: Pos2, _: f32, _: Color32) {
        self.prims += 1;
    }
    fn dashed_line(&mut self, _: Pos2, _: Pos2, _: f32, _: Color32, _: f32, _: f32) {
        self.prims += 1;
    }
    fn polyline(&mut self, points: &[Pos2], _: f32, _: Color32) {
        self.prims += points.len().saturating_sub(1);
    }
    fn fill_area(&mut self, points: &[Pos2], _: f32, _: f32, _: Color32, _: Color32) {
        self.prims += points.len();
    }
    fn fill_between(&mut self, top: &[Pos2], _: &[Pos2], _: Color32) {
        self.prims += top.len();
    }
    fn fill_circle(&mut self, _: Pos2, _: f32, _: Color32) {
        self.prims += 1;
    }
    fn text_styled(
        &mut self,
        pos: Pos2,
        anchor: Align2,
        text: &str,
        size: f32,
        _: Color32,
        bold: bool,
    ) -> Rect {
        self.prims += 1;
        anchor.anchor_size(pos, self.text_size_styled(text, size, bold))
    }
    fn text_size_styled(&mut self, text: &str, size: f32, _: bool) -> Vec2 {
        vec2(text.chars().count() as f32 * size * 0.6, size * 1.25)
    }
}

fn synthetic(n: usize) -> CandleSeries {
    let mut s = CandleSeries::new();
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut rnd = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut price = 100.0;
    for i in 0..n {
        let open: f64 = price;
        let close = open * (1.0 + (rnd() - 0.5) * 0.002);
        s.upsert(Candle {
            open_time_ms: 1_704_067_200_000 + i as i64 * 60_000,
            open,
            high: open.max(close) * (1.0 + rnd() * 0.0005),
            low: open.min(close) * (1.0 - rnd() * 0.0005),
            close,
            volume: 1.0 + rnd() * 50.0,
        });
        price = close;
    }
    s
}

fn ms(v: &[Duration]) -> (f64, f64) {
    let avg = v.iter().map(Duration::as_secs_f64).sum::<f64>() / v.len() as f64 * 1e3;
    let max = v.iter().map(Duration::as_secs_f64).fold(0.0, f64::max) * 1e3;
    (avg, max)
}

/// (base times, overlay times, primitives per frame)
#[allow(clippy::field_reassign_with_default)] // ChartView has private fields
fn run(
    series: &mut CandleSeries,
    chart_type: ChartType,
    visible: usize,
    indicators: bool,
) -> (Vec<Duration>, Vec<Duration>, usize) {
    let theme = Theme::dark();
    let mut view = ChartView::default();
    view.chart_type = chart_type;
    if indicators {
        // A typical layout: Volume, an MA and bands on price, two oscillator panes.
        view.indicators = [
            StudyKind::Volume,
            StudyKind::Ema,
            StudyKind::BollingerBands,
            StudyKind::Rsi,
            StudyKind::Macd,
        ]
        .into_iter()
        .map(|k| Indicator::new(k.default_study()))
        .collect();
    }
    view.zoom_to_bars(visible);
    let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0));
    let input = RenderInput {
        crosshair: Some(full.center()),
        legend_hover: None,
        ..Default::default()
    };
    let (mut base, mut overlay, mut prims) = (Vec::new(), Vec::new(), 0);
    for frame in 0..(FRAMES + 5) {
        // Live forming-bar update; every indicator resumes from its
        // checkpoint, so this measures the tick's real cost.
        let mut last = series.last().expect("non-empty");
        last.close *= 1.0 + if frame % 2 == 0 { 1e-4 } else { -1e-4 };
        last.high = last.high.max(last.close);
        last.low = last.low.min(last.close);
        series.upsert(last);
        let data = ChartData {
            series,
            title: "BENCH · 1m",
            tick_size: Some(0.01),
            now_ms: None,
            bar_interval_ms: Some(60_000),
            bar_close_ms: None,
        };
        let mut canvas = CountingCanvas::default();
        let t0 = Instant::now();
        view.render_base(&mut canvas, full, 1.0, data, &theme);
        let t1 = Instant::now();
        view.render_overlay(&mut canvas, data, &theme, input);
        let t2 = Instant::now();
        // Frames 0-4: zoom applies, caches build (one-off costs).
        if frame >= 5 {
            base.push(t1 - t0);
            overlay.push(t2 - t1);
            prims = canvas.prims;
        }
    }
    (base, overlay, prims)
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.replace('_', "").parse().ok())
        .unwrap_or(1_000_000);
    let t = Instant::now();
    let mut series = synthetic(n);
    println!(
        "generated {n} bars in {:.0} ms",
        t.elapsed().as_secs_f64() * 1e3
    );
    println!(
        "{:<15} {:>10}  {:>17}  {:>19}  {:>8}",
        "type", "visible", "base avg/max ms", "overlay avg/max ms", "prims"
    );
    for (ty, visible, ind) in [
        (ChartType::Candles, n, false),
        (ChartType::Candles, 200_000, false),
        (ChartType::Candles, 20_000, false),
        (ChartType::Candles, 1_000, false),
        (ChartType::Candles, 150, false),
        (ChartType::HeikinAshi, n, false),
        (ChartType::Area, n, false),
        (ChartType::Area, 1_000, false),
        // `+ind`: Volume, EMA, Bollinger Bands, RSI and MACD.
        (ChartType::Candles, n, true),
        (ChartType::Candles, 20_000, true),
        (ChartType::Candles, 1_000, true),
        (ChartType::Candles, 150, true),
    ] {
        let (base, overlay, prims) = run(&mut series, ty, visible, ind);
        let ((ba, bm), (oa, om)) = (ms(&base), ms(&overlay));
        println!(
            "{:<15} {:>10}  {:>8.2} / {:<6.2}  {:>9.2} / {:<6.2}  {:>8}",
            format!("{ty:?}{}", if ind { "+ind" } else { "" }),
            visible.min(n),
            ba,
            bm,
            oa,
            om,
            prims
        );
    }
}
