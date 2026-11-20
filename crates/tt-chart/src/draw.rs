//! Chart scene drawing. Uses only the [`Canvas`] trait, never egui directly.

use crate::ChartStyle;
use crate::ScaleMode;
use crate::render::Canvas;
use crate::scale::{self, format_full_time, format_percent, format_price, format_volume};
use crate::time_axis;
use crate::viewport::Viewport;
use egui::{Align2, Color32, Pos2, Rect, pos2, vec2};
use tt_core::CandleSeries;
use tt_theme::Theme;

/// Everything needed to draw one frame.
pub(crate) struct Scene<'a> {
    pub vp: Viewport,
    pub theme: &'a Theme,
    pub style: &'a ChartStyle,
    /// Whole widget.
    pub full: Rect,
    pub plot: Rect,
    pub price_axis: Rect,
    pub time_axis: Rect,
    pub series: &'a CandleSeries,
    pub crosshair: Option<Pos2>,
    pub precision: usize,
    pub tick_size: Option<f64>,
    pub bar_interval_ms: i64,
    pub title: &'a str,
}

pub(crate) fn draw(c: &mut dyn Canvas, s: &Scene<'_>) {
    let t = s.theme;
    c.set_clip(s.full);
    c.fill_rect(s.full, t.bg);

    if s.series.is_empty() {
        c.text(
            s.plot.center(),
            Align2::CENTER_CENTER,
            "Waiting for data…",
            s.style.font_size + 1.0,
            t.text_muted,
        );
        axis_lines(c, s);
        readout(c, s, None);
        return;
    }

    let price_ticks = price_ticks(s);
    c.set_clip(s.plot);
    for (y, _) in &price_ticks {
        c.line(
            pos2(s.plot.left(), *y),
            pos2(s.plot.right(), *y),
            1.0,
            t.grid,
        );
    }
    let time_ticks = time_ticks(c, s);
    for (x, _) in &time_ticks {
        c.line(
            pos2(*x, s.plot.top()),
            pos2(*x, s.plot.bottom()),
            1.0,
            t.grid,
        );
    }
    candles(c, s);
    let last_y = last_price_line(c, s);

    c.set_clip(s.full);
    axis_lines(c, s);
    price_labels(c, s, &price_ticks);
    time_labels(c, s, &time_ticks);
    if let Some((y, label, color)) = last_y {
        axis_tag(
            c,
            s,
            pos2(s.price_axis.left() + 1.0, y),
            Align2::LEFT_CENTER,
            &label,
            color,
        );
    }

    let hovered = s.crosshair.and_then(|p| s.vp.bar_at(p.x));
    if let Some(p) = s.crosshair {
        crosshair(c, s, p, hovered);
    }
    readout(c, s, hovered.or(s.series.len().checked_sub(1)));
}

/// Snaps a coordinate to the center of a physical pixel for crisp 1px lines.
fn crisp(v: f32, ppp: f32) -> f32 {
    ((v * ppp).floor() + 0.5) / ppp
}

/// Grid/label positions on the price axis: (y, label).
fn price_ticks(s: &Scene<'_>) -> Vec<(f32, String)> {
    let vp = &s.vp;
    scale::price_ticks(vp, 56.0, s.tick_size)
        .into_iter()
        .map(|p| (vp.y_of(p), axis_value_label(s, p)))
        .collect()
}

/// Time-axis tick positions: (x, label).
fn time_ticks(c: &mut dyn Canvas, s: &Scene<'_>) -> Vec<(f32, String)> {
    let ppp = c.pixels_per_point();
    let vp = s.vp;
    // Examine at most one candidate bar per ~3 points.
    let stride = (3.0 / vp.bar_spacing).ceil().max(1.0) as usize;
    let size = s.style.font_size;
    time_axis::time_ticks(
        s.series.open_times(),
        vp.visible_range(),
        (s.plot.left(), s.plot.right()),
        stride,
        18.0,
        |i| crisp(vp.x_of(i as f32), ppp),
        |text| c.text_size(text, size).x,
    )
    .into_iter()
    .map(|t| (t.x, t.label))
    .collect()
}

fn candles(c: &mut dyn Canvas, s: &Scene<'_>) {
    let ppp = c.pixels_per_point();
    let ser = s.series;
    let (o, h, l, cl) = (ser.opens(), ser.highs(), ser.lows(), ser.closes());
    let t = s.theme;

    // Work in physical pixels so bodies are an odd width and wicks sit centered.
    let spacing_px = s.vp.bar_spacing * ppp;
    let mut body_px = (spacing_px * 0.72).floor().max(1.0);
    if body_px as i32 % 2 == 0 {
        body_px -= 1.0;
    }
    let body_px = body_px.max(1.0);
    let wick_px = ppp.round().max(1.0).min(body_px);

    for i in s.vp.visible_range() {
        let up = cl[i] >= o[i];
        let (body, wick) = if up {
            (t.up, t.up_wick)
        } else {
            (t.down, t.down_wick)
        };
        let cx_px = (s.vp.x_of(i as f32) * ppp).round();
        let wx0 = (cx_px - (wick_px / 2.0).floor()) / ppp;
        c.fill_rect(
            Rect::from_min_max(
                pos2(wx0, s.vp.y_of(h[i])),
                pos2(wx0 + wick_px / ppp, s.vp.y_of(l[i])),
            ),
            wick,
        );
        if body_px > wick_px {
            let a = s.vp.y_of(o[i]);
            let b = s.vp.y_of(cl[i]);
            let y0 = (a.min(b) * ppp).round() / ppp;
            let y1 = ((a.max(b) * ppp).round() / ppp).max(y0 + 1.0 / ppp);
            let bx0 = (cx_px - (body_px / 2.0).floor()) / ppp;
            c.fill_rect(
                Rect::from_min_max(pos2(bx0, y0), pos2(bx0 + body_px / ppp, y1)),
                body,
            );
        }
    }
}

fn axis_lines(c: &mut dyn Canvas, s: &Scene<'_>) {
    let ppp = c.pixels_per_point();
    let col = s.theme.border;
    let x = crisp(s.plot.right(), ppp);
    c.line(pos2(x, s.full.top()), pos2(x, s.full.bottom()), 1.0, col);
    let y = crisp(s.plot.bottom(), ppp);
    c.line(pos2(s.full.left(), y), pos2(s.full.right(), y), 1.0, col);
}

fn price_labels(c: &mut dyn Canvas, s: &Scene<'_>, ticks: &[(f32, String)]) {
    let x = s.price_axis.left() + 8.0;
    for (y, label) in ticks {
        if *y < s.plot.top() + 6.0 || *y > s.plot.bottom() - 6.0 {
            continue;
        }
        c.text(
            pos2(x, *y),
            Align2::LEFT_CENTER,
            label,
            s.style.font_size,
            s.theme.text_muted,
        );
    }
}

fn time_labels(c: &mut dyn Canvas, s: &Scene<'_>, ticks: &[(f32, String)]) {
    let y = s.time_axis.center().y;
    for (x, label) in ticks {
        c.text(
            pos2(*x, y),
            Align2::CENTER_CENTER,
            label,
            s.style.font_size,
            s.theme.text_muted,
        );
    }
}

/// Label as shown on the price axis for a price (percent in percent mode).
fn axis_value_label(s: &Scene<'_>, price: f64) -> String {
    match s.vp.scale.mode {
        ScaleMode::Percent => format_percent(s.vp.scale.to_scaled(price)),
        _ => format_price(price, s.precision),
    }
}

/// A filled tag on an axis with centered text.
fn axis_tag(
    c: &mut dyn Canvas,
    s: &Scene<'_>,
    anchor: Pos2,
    align: Align2,
    text: &str,
    bg: Color32,
) {
    let size = c.text_size(text, s.style.font_size) + vec2(10.0, 6.0);
    let rect = align.anchor_size(anchor, size);
    c.fill_rounded_rect(rect, 2.0, bg);
    c.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        s.style.font_size,
        s.theme.text_on_accent,
    );
}

/// Draws the dashed last-price line; returns where to put its axis tag.
fn last_price_line(c: &mut dyn Canvas, s: &Scene<'_>) -> Option<(f32, String, Color32)> {
    let last = s.series.last()?;
    let color = if last.is_bullish() {
        s.theme.up
    } else {
        s.theme.down
    };
    let y = crisp(s.vp.y_of(last.close), c.pixels_per_point());
    if y < s.plot.top() || y > s.plot.bottom() {
        return None;
    }
    c.dashed_line(
        pos2(s.plot.left(), y),
        pos2(s.plot.right(), y),
        1.0,
        color,
        2.0,
        2.0,
    );
    Some((y, axis_value_label(s, last.close), color))
}

fn crosshair(c: &mut dyn Canvas, s: &Scene<'_>, p: Pos2, bar: Option<usize>) {
    let ppp = c.pixels_per_point();
    let col = s.theme.crosshair;
    // Snap the vertical line to the hovered bar's center.
    let x = crisp(bar.map_or(p.x, |i| s.vp.x_of(i as f32)), ppp);
    let y = crisp(p.y, ppp);
    c.set_clip(s.plot);
    c.dashed_line(
        pos2(x, s.plot.top()),
        pos2(x, s.plot.bottom()),
        1.0,
        col,
        4.0,
        4.0,
    );
    c.dashed_line(
        pos2(s.plot.left(), y),
        pos2(s.plot.right(), y),
        1.0,
        col,
        4.0,
        4.0,
    );
    c.set_clip(s.full);

    let price = s.vp.price_at(p.y);
    axis_tag(
        c,
        s,
        pos2(s.price_axis.left() + 1.0, y),
        Align2::LEFT_CENTER,
        &axis_value_label(s, price),
        s.theme.crosshair_label_bg,
    );
    if let Some(i) = bar {
        let t = s.series.open_times()[i];
        axis_tag(
            c,
            s,
            pos2(x, s.time_axis.top() + 2.0),
            Align2::CENTER_TOP,
            &format_full_time(t, s.bar_interval_ms),
            s.theme.crosshair_label_bg,
        );
    }
}

/// Top-left readout: title and OHLCV of the hovered (or last) bar.
fn readout(c: &mut dyn Canvas, s: &Scene<'_>, bar: Option<usize>) {
    let t = s.theme;
    let size = s.style.font_size;
    let mut pos = s.plot.left_top() + vec2(10.0, 8.0);
    let mut put = |c: &mut dyn Canvas, text: &str, color: Color32, gap: f32| {
        let r = c.text(pos, Align2::LEFT_TOP, text, size, color);
        pos.x = r.right() + gap;
    };

    if !s.title.is_empty() {
        put(c, s.title, t.text, 14.0);
    }
    let Some(k) = bar.and_then(|i| s.series.get(i)) else {
        return;
    };
    let color = if k.is_bullish() { t.up } else { t.down };
    let d = s.precision;
    for (name, v) in [("O", k.open), ("H", k.high), ("L", k.low), ("C", k.close)] {
        put(c, name, t.text_muted, 3.0);
        put(c, &format_price(v, d), color, 10.0);
    }
    let change = k.close - k.open;
    let pct = if k.open != 0.0 {
        change / k.open * 100.0
    } else {
        0.0
    };
    put(c, &format!("{change:+.d$} ({pct:+.2}%)"), color, 14.0);
    put(c, "Vol", t.text_muted, 3.0);
    put(c, &format_volume(k.volume), t.text, 0.0);
}
