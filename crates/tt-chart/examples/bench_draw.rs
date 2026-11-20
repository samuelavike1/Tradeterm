//! Headless chart benchmark: 1,000,000 bars through the real `ChartView`.
//!
//! `cargo run --release -p tt-chart --example bench_draw [-- BARS]`
//!
//! Each frame runs a full egui pass (layout, input, chart scene building) via
//! `Context::run_ui`, then tessellates the output like eframe does before
//! uploading to the GPU. GPU time is not included. The last bar is updated
//! every frame, as a live feed would, so incremental cache updates are
//! measured too. The pointer sits over the plot so the crosshair is drawn.

use egui::{Event, RawInput, Rect, pos2, vec2};
use std::time::{Duration, Instant};
use tt_chart::{ChartData, ChartType, ChartView};
use tt_core::{Candle, CandleSeries};
use tt_theme::Theme;

const FRAMES: usize = 60;

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

struct Stats {
    chart: Vec<Duration>,
    pass: Vec<Duration>,
    tess: Vec<Duration>,
}

fn ms(v: &[Duration]) -> (f64, f64) {
    let avg = v.iter().map(Duration::as_secs_f64).sum::<f64>() / v.len() as f64 * 1e3;
    let max = v.iter().map(Duration::as_secs_f64).fold(0.0, f64::max) * 1e3;
    (avg, max)
}

fn run(series: &mut CandleSeries, chart_type: ChartType, visible: usize, ppp: f32) -> Stats {
    let ctx = egui::Context::default();
    let theme = Theme::dark();
    tt_theme::install(&ctx, tt_theme::ThemeMode::Dark, Default::default());
    let mut view = ChartView::default();
    view.chart_type = chart_type;
    view.zoom_to_bars(visible);
    let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0) / ppp);

    let mut stats = Stats {
        chart: Vec::new(),
        pass: Vec::new(),
        tess: Vec::new(),
    };
    for frame in 0..(FRAMES + 5) {
        // Live forming-bar update.
        let mut last = series.last().expect("non-empty");
        last.close *= 1.0 + if frame % 2 == 0 { 1e-4 } else { -1e-4 };
        last.high = last.high.max(last.close);
        last.low = last.low.min(last.close);
        series.upsert(last);

        let input = RawInput {
            screen_rect: Some(screen),
            events: vec![Event::PointerMoved(screen.center())],
            ..Default::default()
        };
        ctx.set_pixels_per_point(ppp);
        let t0 = Instant::now();
        let out = ctx.run_ui(input, |ui| {
            let data = ChartData {
                series,
                title: "BENCH · 1m",
                tick_size: Some(0.01),
                now_ms: None,
                bar_interval_ms: Some(60_000),
            };
            view.show(ui, data, &theme);
        });
        let pass = t0.elapsed();
        let t1 = Instant::now();
        let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
        let tess = t1.elapsed();
        std::hint::black_box(prims);

        // Frames 0-4: zoom applies, caches build (one-off costs).
        if frame >= 5 {
            stats.chart.push(view.stats().draw_time);
            stats.pass.push(pass);
            stats.tess.push(tess);
        }
    }
    stats
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
        "{:<15} {:>10} {:>4}  {:>17}  {:>17}  {:>17}",
        "type", "visible", "ppp", "chart avg/max ms", "egui pass avg/max", "tessellate avg/max"
    );
    for ppp in [1.0, 1.5] {
        for (ty, visible) in [
            (ChartType::Candles, n),
            (ChartType::Candles, 200_000),
            (ChartType::Candles, 20_000),
            (ChartType::Candles, 1_000),
            (ChartType::Candles, 150),
            (ChartType::HeikinAshi, n),
            (ChartType::Area, n),
            (ChartType::Area, 1_000),
        ] {
            let s = run(&mut series, ty, visible, ppp);
            let (ca, cm) = ms(&s.chart);
            let (pa, pm) = ms(&s.pass);
            let (ta, tm) = ms(&s.tess);
            println!(
                "{:<15} {:>10} {:>4}  {:>7.3} / {:>7.3}  {:>7.3} / {:>7.3}  {:>7.3} / {:>7.3}",
                format!("{ty:?}"),
                visible.min(n),
                ppp,
                ca,
                cm,
                pa,
                pm,
                ta,
                tm
            );
        }
    }
}
