//! All drawing for the chart. This is the only module that uses `egui::Painter`,
//! so it is the swap point for a future custom wgpu renderer.

use crate::ChartStyle;
use crate::scale::{
    self, format_axis_time, format_full_time, format_price, format_volume, nice_step,
    time_label_step,
};
use crate::viewport::Viewport;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};
use tt_core::CandleSeries;

/// Everything the painter needs for one frame.
pub(crate) struct FrameData<'a> {
    pub vp: Viewport,
    pub series: &'a CandleSeries,
    pub style: &'a ChartStyle,
    /// Whole widget area (plot + axes).
    pub full: Rect,
    pub plot: Rect,
    pub crosshair: Option<Pos2>,
    pub decimals: usize,
    pub bar_interval_ms: i64,
    pub title: &'a str,
}

pub(crate) fn paint(painter: &Painter, f: &FrameData<'_>) {
    let s = f.style;
    painter.rect_filled(f.full, 0.0, s.background);

    if f.series.is_empty() {
        painter.text(
            f.plot.center(),
            Align2::CENTER_CENTER,
            "Waiting for data…",
            FontId::proportional(s.font_size + 2.0),
            s.axis_text,
        );
        paint_axis_lines(painter, f);
        paint_title(painter, f, None);
        return;
    }

    let visible = f.vp.visible_range();
    let ppp = painter.pixels_per_point();
    let plot_painter = painter.with_clip_rect(f.plot);

    let price_ticks = price_grid(painter, &plot_painter, f);
    let time_labels = time_grid(&plot_painter, f);
    paint_candles(&plot_painter, f, visible.clone(), ppp);
    paint_axis_lines(painter, f);
    paint_price_labels(painter, f, &price_ticks);
    paint_time_labels(painter, f, &time_labels);
    paint_last_price(painter, &plot_painter, f);

    let hovered_bar = f.crosshair.and_then(|p| f.vp.bar_at(p.x));
    if let Some(p) = f.crosshair {
        paint_crosshair(painter, &plot_painter, f, p, hovered_bar);
    }
    paint_title(painter, f, hovered_bar.or(f.series.len().checked_sub(1)));
}

fn font(s: &ChartStyle) -> FontId {
    FontId::monospace(s.font_size)
}

/// Snaps a coordinate to the center of a physical pixel for crisp 1px lines.
fn crisp(v: f32, ppp: f32) -> f32 {
    ((v * ppp).floor() + 0.5) / ppp
}

/// Horizontal grid lines; returns the tick prices for labelling.
fn price_grid(painter: &Painter, plot_painter: &Painter, f: &FrameData<'_>) -> Vec<f64> {
    let ppp = painter.pixels_per_point();
    let target = (f.plot.height() / 56.0).max(2.0) as f64;
    let step = nice_step(f.vp.price_hi - f.vp.price_lo, target);
    let ticks: Vec<f64> = scale::ticks(f.vp.price_lo, f.vp.price_hi, step).collect();
    for &p in &ticks {
        let y = crisp(f.vp.y_of(p), ppp);
        plot_painter.hline(f.plot.x_range(), y, Stroke::new(1.0, f.style.grid));
    }
    ticks
}

/// Vertical grid lines at "round" times; returns (x, open time, label step).
fn time_grid(plot_painter: &Painter, f: &FrameData<'_>) -> Vec<(f32, i64, i64)> {
    let ppp = plot_painter.pixels_per_point();
    let step = time_label_step(f.bar_interval_ms, f.vp.bar_spacing, 90.0);
    let times = f.series.open_times();
    let mut out = Vec::new();
    let mut last_x = f32::NEG_INFINITY;
    for i in f.vp.visible_range() {
        let t = times[i];
        if t.rem_euclid(step) != 0 {
            continue;
        }
        let x = crisp(f.vp.x_of(i as f32), ppp);
        // Guard against gaps in the data packing labels too tightly.
        if x - last_x < 60.0 {
            continue;
        }
        last_x = x;
        plot_painter.vline(x, f.plot.y_range(), Stroke::new(1.0, f.style.grid));
        out.push((x, t, step));
    }
    out
}

fn paint_candles(painter: &Painter, f: &FrameData<'_>, range: std::ops::Range<usize>, ppp: f32) {
    let (o, h, l, c) = (
        f.series.opens(),
        f.series.highs(),
        f.series.lows(),
        f.series.closes(),
    );
    // Work in physical pixels so bodies are an odd width and wicks sit exactly centered.
    let spacing_px = f.vp.bar_spacing * ppp;
    let mut body_px = (spacing_px * 0.72).floor().max(1.0);
    if body_px as i32 % 2 == 0 {
        body_px -= 1.0;
    }
    let body_px = body_px.max(1.0);
    let wick_px = ppp.round().max(1.0).min(body_px);

    let mut shapes = Vec::with_capacity(range.len() * 2);
    for i in range {
        let color = if c[i] >= o[i] {
            f.style.up
        } else {
            f.style.down
        };
        let cx_px = (f.vp.x_of(i as f32) * ppp).round();

        let y_hi = f.vp.y_of(h[i]);
        let y_lo = f.vp.y_of(l[i]);
        let wick_x0 = (cx_px - (wick_px / 2.0).floor()) / ppp;
        shapes.push(Shape::rect_filled(
            Rect::from_min_max(pos2(wick_x0, y_hi), pos2(wick_x0 + wick_px / ppp, y_lo)),
            0.0,
            color,
        ));

        if body_px > wick_px {
            let (y0, y1) = {
                let a = f.vp.y_of(o[i]);
                let b = f.vp.y_of(c[i]);
                (a.min(b), a.max(b))
            };
            // At least one physical pixel tall so dojis stay visible.
            let y0 = (y0 * ppp).round() / ppp;
            let y1 = ((y1 * ppp).round() / ppp).max(y0 + 1.0 / ppp);
            let body_x0 = (cx_px - (body_px / 2.0).floor()) / ppp;
            shapes.push(Shape::rect_filled(
                Rect::from_min_max(pos2(body_x0, y0), pos2(body_x0 + body_px / ppp, y1)),
                0.0,
                color,
            ));
        }
    }
    painter.extend(shapes);
}

fn paint_axis_lines(painter: &Painter, f: &FrameData<'_>) {
    let ppp = painter.pixels_per_point();
    let stroke = Stroke::new(1.0, f.style.axis_line);
    let x = crisp(f.plot.right(), ppp);
    painter.vline(x, f.full.y_range(), stroke);
    let y = crisp(f.plot.bottom(), ppp);
    painter.hline(f.full.x_range(), y, stroke);
}

fn paint_price_labels(painter: &Painter, f: &FrameData<'_>, ticks: &[f64]) {
    let x = f.plot.right() + 8.0;
    for &p in ticks {
        let y = f.vp.y_of(p);
        if y < f.plot.top() + 6.0 || y > f.plot.bottom() - 6.0 {
            continue;
        }
        painter.text(
            pos2(x, y),
            Align2::LEFT_CENTER,
            format_price(p, f.decimals),
            font(f.style),
            f.style.axis_text,
        );
    }
}

fn paint_time_labels(painter: &Painter, f: &FrameData<'_>, labels: &[(f32, i64, i64)]) {
    let y = f.plot.bottom() + f.style.time_axis_height / 2.0;
    for &(x, t, step) in labels {
        painter.text(
            pos2(x, y),
            Align2::CENTER_CENTER,
            format_axis_time(t, step),
            font(f.style),
            f.style.axis_text,
        );
    }
}

/// A filled tag on an axis with centered text.
fn axis_tag(
    painter: &Painter,
    f: &FrameData<'_>,
    anchor: Pos2,
    align: Align2,
    text: String,
    bg: Color32,
) {
    let galley = painter.layout_no_wrap(text, font(f.style), f.style.label_text);
    let size = galley.size() + vec2(10.0, 6.0);
    let rect = align.anchor_size(anchor, size);
    painter.rect_filled(rect, 2.0, bg);
    painter.galley(
        rect.center() - galley.size() / 2.0,
        galley,
        f.style.label_text,
    );
}

fn paint_last_price(painter: &Painter, plot_painter: &Painter, f: &FrameData<'_>) {
    let Some(last) = f.series.last() else { return };
    let color = if last.is_bullish() {
        f.style.up
    } else {
        f.style.down
    };
    let ppp = painter.pixels_per_point();
    let y = crisp(f.vp.y_of(last.close), ppp);
    if y < f.plot.top() || y > f.plot.bottom() {
        return;
    }
    plot_painter.extend(Shape::dashed_line(
        &[pos2(f.plot.left(), y), pos2(f.plot.right(), y)],
        Stroke::new(1.0, color),
        2.0,
        2.0,
    ));
    axis_tag(
        painter,
        f,
        pos2(f.plot.right() + 1.0, y),
        Align2::LEFT_CENTER,
        format_price(last.close, f.decimals),
        color,
    );
}

fn paint_crosshair(
    painter: &Painter,
    plot_painter: &Painter,
    f: &FrameData<'_>,
    pointer: Pos2,
    bar: Option<usize>,
) {
    let ppp = painter.pixels_per_point();
    let stroke = Stroke::new(1.0, f.style.crosshair);
    // Snap the vertical line to the hovered bar like TradingView does.
    let x = crisp(bar.map_or(pointer.x, |i| f.vp.x_of(i as f32)), ppp);
    let y = crisp(pointer.y, ppp);
    plot_painter.extend(Shape::dashed_line(
        &[pos2(x, f.plot.top()), pos2(x, f.plot.bottom())],
        stroke,
        4.0,
        4.0,
    ));
    plot_painter.extend(Shape::dashed_line(
        &[pos2(f.plot.left(), y), pos2(f.plot.right(), y)],
        stroke,
        4.0,
        4.0,
    ));

    axis_tag(
        painter,
        f,
        pos2(f.plot.right() + 1.0, y),
        Align2::LEFT_CENTER,
        format_price(f.vp.price_at(pointer.y), f.decimals),
        f.style.label_bg,
    );
    if let Some(i) = bar {
        let t = f.series.open_times()[i];
        axis_tag(
            painter,
            f,
            pos2(x, f.plot.bottom() + 2.0),
            Align2::CENTER_TOP,
            format_full_time(t, f.bar_interval_ms),
            f.style.label_bg,
        );
    }
}

/// Top-left readout: title and OHLCV of the hovered (or last) bar.
fn paint_title(painter: &Painter, f: &FrameData<'_>, bar: Option<usize>) {
    let s = f.style;
    let font = font(s);
    let mut pos = f.plot.left_top() + vec2(10.0, 8.0);
    let mut put = |text: String, color: Color32, gap: f32| {
        let r = painter.text(pos, Align2::LEFT_TOP, text, font.clone(), color);
        pos.x = r.right() + gap;
    };

    if !f.title.is_empty() {
        put(f.title.to_owned(), s.text, 14.0);
    }
    let Some(c) = bar.and_then(|i| f.series.get(i)) else {
        return;
    };
    let color = if c.is_bullish() { s.up } else { s.down };
    let d = f.decimals;
    for (k, v) in [("O", c.open), ("H", c.high), ("L", c.low), ("C", c.close)] {
        put(k.to_owned(), s.axis_text, 3.0);
        put(format_price(v, d), color, 10.0);
    }
    let change = c.close - c.open;
    let pct = if c.open != 0.0 {
        change / c.open * 100.0
    } else {
        0.0
    };
    put(format!("{change:+.d$} ({pct:+.2}%)"), color, 14.0);
    put("Vol".to_owned(), s.axis_text, 3.0);
    put(format_volume(c.volume), s.text, 0.0);
}
