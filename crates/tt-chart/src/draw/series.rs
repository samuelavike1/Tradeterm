//! Main series drawing: one function per chart type, plus the pixel-aligned
//! bar geometry they share.

use super::{Scene, crisp};
use crate::bars::{DrawBar, max_volume};
use crate::render::Canvas;
use crate::{ChartStyle, ChartType};
use ecolor::Color32;
use emath::{Pos2, Rect, pos2};
use tt_theme::with_alpha;

/// Pixel-aligned bar geometry shared by all bar-like chart types.
pub(super) struct BarGeom {
    pub(super) ppp: f32,
    /// Body width in physical pixels (odd, ≥ 1).
    pub(super) body_px: f32,
    /// Wick/line width in physical pixels.
    pub(super) wick_px: f32,
}

impl BarGeom {
    pub(super) fn new(spacing: f32, ppp: f32) -> Self {
        let mut body_px = (spacing * ppp * 0.72).floor().max(1.0);
        if body_px as i32 % 2 == 0 {
            body_px -= 1.0;
        }
        let body_px = body_px.max(1.0);
        Self {
            ppp,
            body_px,
            wick_px: ppp.round().max(1.0).min(body_px),
        }
    }

    /// Physical-pixel center column for screen x.
    pub(super) fn center_px(&self, x: f32) -> f32 {
        (x * self.ppp).round()
    }

    /// Rect `width_px` physical pixels wide, centered on `cx_px`, between y0 and y1.
    pub(super) fn column(&self, cx_px: f32, width_px: f32, y0: f32, y1: f32) -> Rect {
        let x0 = (cx_px - (width_px / 2.0).floor()) / self.ppp;
        Rect::from_min_max(
            pos2(x0, y0.min(y1)),
            pos2(x0 + width_px / self.ppp, y0.max(y1)),
        )
    }

    /// Snaps y0..y1 to pixels, at least one pixel tall.
    pub(super) fn span(&self, a: f32, b: f32) -> (f32, f32) {
        let y0 = (a.min(b) * self.ppp).round() / self.ppp;
        let y1 = ((a.max(b) * self.ppp).round() / self.ppp).max(y0 + 1.0 / self.ppp);
        (y0, y1)
    }
}

pub(super) fn series(c: &mut dyn Canvas, s: &Scene<'_>) {
    let g = BarGeom::new(s.vp.bar_spacing, c.pixels_per_point());
    if s.decimated {
        decimated(c, s, &g);
        return;
    }
    match s.chart_type {
        ChartType::Candles | ChartType::HeikinAshi => candles(c, s, &g, false),
        ChartType::HollowCandles => candles(c, s, &g, true),
        ChartType::VolumeCandles => volume_candles(c, s, &g),
        ChartType::Bars => ohlc_bars(c, s, &g),
        ChartType::Line => line(c, s, false),
        ChartType::LineWithMarkers => {
            line(c, s, false);
            markers(c, s);
        }
        ChartType::StepLine => step_line(c, s),
        ChartType::Area => line(c, s, true),
        ChartType::HlcArea => hlc_area(c, s),
        ChartType::Baseline => baseline(c, s),
        ChartType::Columns => columns(c, s, &g),
        ChartType::HighLow => high_low(c, s, &g),
        ChartType::Renko | ChartType::LineBreak => bricks(c, s),
    }
}

/// Candles whose body width is proportional to volume (vs. the visible max).
fn volume_candles(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom) {
    let vp = &s.vp;
    let max = max_volume(s.bars).max(f64::MIN_POSITIVE);
    // Allow wider bodies than regular candles: the busiest bar fills its slot.
    let full_px = (s.vp.bar_spacing * g.ppp * 0.92).floor().max(1.0);
    for b in s.bars {
        let (body, wick) = s.colors(b.close >= b.open);
        let cx = g.center_px(b.x);
        let mut w = (full_px * (b.volume / max) as f32).round().max(1.0);
        if w as i32 % 2 == 0 {
            w -= 1.0;
        }
        let w = w.max(1.0);
        c.fill_rect(
            g.column(cx, g.wick_px.min(w), vp.y_of(b.high), vp.y_of(b.low)),
            wick,
        );
        let (y0, y1) = g.span(vp.y_of(b.open), vp.y_of(b.close));
        c.fill_rect(g.column(cx, w, y0, y1), body);
    }
}

/// A dot on each close (when bars are far enough apart to see them).
fn markers(c: &mut dyn Canvas, s: &Scene<'_>) {
    if s.vp.bar_spacing < 6.0 {
        return;
    }
    let color = line_color(s);
    let r = (s.vp.bar_spacing * 0.18).clamp(2.0, 3.5);
    for b in s.bars {
        c.fill_circle(pos2(b.x, s.vp.y_of(b.close)), r, color);
    }
}

fn step_line(c: &mut dyn Canvas, s: &Scene<'_>) {
    let half = s.vp.bar_spacing / 2.0;
    let mut pts = Vec::with_capacity(s.bars.len() * 2);
    for b in s.bars {
        let y = s.vp.y_of(b.close);
        pts.push(pos2(b.x - half, y));
        pts.push(pos2(b.x + half, y));
    }
    c.polyline(&pts, 1.5, line_color(s));
}

/// High (up color), low (down color) and close lines with bands between.
fn hlc_area(c: &mut dyn Canvas, s: &Scene<'_>) {
    let vp = &s.vp;
    let pts = |f: fn(&DrawBar) -> f64| -> Vec<Pos2> {
        s.bars.iter().map(|b| pos2(b.x, vp.y_of(f(b)))).collect()
    };
    let (high, low, close) = (pts(|b| b.high), pts(|b| b.low), pts(|b| b.close));
    let alpha = if s.theme.dark { 0.22 } else { 0.18 };
    c.fill_between(&high, &close, with_alpha(s.theme.up, alpha));
    c.fill_between(&close, &low, with_alpha(s.theme.down, alpha));
    c.polyline(&high, 1.0, s.theme.up);
    c.polyline(&low, 1.0, s.theme.down);
    c.polyline(&close, 1.5, line_color(s));
}

/// Closes against a baseline at the middle of the visible price range
/// (TradingView's default 50%): green above, red below.
fn baseline(c: &mut dyn Canvas, s: &Scene<'_>) {
    let vp = &s.vp;
    let base = vp.price_min + (vp.price_max - vp.price_min) * 0.5;
    let yb = vp.y_of(base);
    // Close points, with exact crossing points inserted so fills split cleanly.
    let mut pts: Vec<Pos2> = Vec::with_capacity(s.bars.len() + 8);
    for b in s.bars {
        let p = pos2(b.x, vp.y_of(b.close));
        if let Some(prev) = pts.last().copied()
            && (prev.y - yb) * (p.y - yb) < 0.0
        {
            let t = (yb - prev.y) / (p.y - prev.y);
            pts.push(pos2(prev.x + (p.x - prev.x) * t, yb));
        }
        pts.push(p);
    }
    let alpha = if s.theme.dark { 0.25 } else { 0.2 };
    let above: Vec<Pos2> = pts.iter().map(|p| pos2(p.x, p.y.min(yb))).collect();
    let below: Vec<Pos2> = pts.iter().map(|p| pos2(p.x, p.y.max(yb))).collect();
    let flat: Vec<Pos2> = pts.iter().map(|p| pos2(p.x, yb)).collect();
    c.fill_between(&above, &flat, with_alpha(s.theme.up, alpha));
    c.fill_between(&flat, &below, with_alpha(s.theme.down, alpha));
    let rect = s.vp_rect();
    c.dashed_line(
        pos2(rect.left(), yb),
        pos2(rect.right(), yb),
        1.0,
        s.theme.text_muted,
        3.0,
        3.0,
    );
    // The line itself: up color above the baseline, down color below.
    c.set_clip(Rect::from_min_max(rect.min, pos2(rect.right(), yb)));
    c.polyline(&pts, 1.5, s.theme.up);
    c.set_clip(Rect::from_min_max(pos2(rect.left(), yb), rect.max));
    c.polyline(&pts, 1.5, s.theme.down);
    c.set_clip(rect);
}

/// Columns from the bottom of the price area up to each close.
fn columns(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom) {
    let bottom = s.vp.plot.bottom;
    for b in s.bars {
        let (color, _) = s.colors(b.close >= b.prev_close);
        let col = g.column(g.center_px(b.x), g.body_px, s.vp.y_of(b.close), bottom);
        c.fill_rect(col, with_alpha(color, 0.85));
    }
}

/// Bars spanning each high-low range.
fn high_low(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom) {
    let color = line_color(s);
    for b in s.bars {
        let (y0, y1) = g.span(s.vp.y_of(b.high), s.vp.y_of(b.low));
        c.fill_rect(g.column(g.center_px(b.x), g.body_px, y0, y1), color);
    }
}

/// Renko / Line break bricks: adjacent bodies without wicks.
fn bricks(c: &mut dyn Canvas, s: &Scene<'_>) {
    let ppp = c.pixels_per_point();
    let w = (s.vp.bar_spacing * ppp * 0.9).floor().max(1.0) / ppp;
    for b in s.bars {
        let (color, _) = s.colors(b.close >= b.open);
        let (y0, y1) = (s.vp.y_of(b.open), s.vp.y_of(b.close));
        let rect = Rect::from_min_max(
            pos2(b.x - w / 2.0, y0.min(y1)),
            pos2(b.x + w / 2.0, y0.max(y1).max(y0.min(y1) + 1.0 / ppp)),
        );
        c.fill_rect(rect, color);
    }
}

fn candles(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom, hollow: bool) {
    let vp = &s.vp;
    let st = s.style.symbol.candle(s.chart_type);
    let by_prev_close = st.color_by_prev_close;
    let borders = [
        ChartStyle::border_color(st, s.theme, false),
        ChartStyle::border_color(st, s.theme, true),
    ];
    for b in s.bars {
        let rising = b.close >= b.open;
        // Hollow candles: color by close vs previous close, hollow when rising.
        let up = if hollow {
            b.close >= b.prev_close
        } else {
            Scene::rising(by_prev_close, b)
        };
        let (body, wick) = s.colors(up);
        let cx = g.center_px(b.x);
        let (yh, yl) = (vp.y_of(b.high), vp.y_of(b.low));
        let (y0, y1) = g.span(vp.y_of(b.open), vp.y_of(b.close));

        if g.body_px <= g.wick_px {
            c.fill_rect(g.column(cx, g.wick_px, yh, yl), wick);
            continue;
        }
        if hollow && rising {
            // Wick above and below the body only, then an outlined body.
            if st.wick {
                c.fill_rect(g.column(cx, g.wick_px, yh, y0), wick);
                c.fill_rect(g.column(cx, g.wick_px, y1, yl), wick);
            }
            c.stroke_rect(g.column(cx, g.body_px, y0, y1), g.wick_px / g.ppp, body);
        } else {
            if st.wick {
                c.fill_rect(g.column(cx, g.wick_px, yh, yl), wick);
            }
            let rect = g.column(cx, g.body_px, y0, y1);
            if st.body {
                c.fill_rect(rect, body);
            }
            // Borders only cost a primitive when they differ from the body.
            let border = borders[up as usize];
            if st.borders && (!st.body || border != body) {
                c.stroke_rect(rect, 1.0 / g.ppp, border);
            }
        }
    }
}

fn ohlc_bars(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom) {
    let vp = &s.vp;
    let tick_px = ((g.body_px - g.wick_px) / 2.0).floor().max(1.0);
    let w = g.wick_px / g.ppp;
    let by_prev_close = s.style.symbol.bars.color_by_prev_close;
    for b in s.bars {
        let (color, _) = s.colors(Scene::rising(by_prev_close, b));
        let cx = g.center_px(b.x);
        c.fill_rect(
            g.column(cx, g.wick_px, vp.y_of(b.high), vp.y_of(b.low)),
            color,
        );
        let half_wick = (g.wick_px / 2.0).floor();
        let yo = crisp(vp.y_of(b.open), g.ppp);
        let yc = crisp(vp.y_of(b.close), g.ppp);
        // Open tick to the left, close tick to the right.
        let left = (cx - half_wick - tick_px) / g.ppp;
        let right = (cx - half_wick + g.wick_px) / g.ppp;
        let tick = |x0: f32, y: f32| {
            Rect::from_min_max(
                pos2(x0, y - w / 2.0),
                pos2(x0 + tick_px / g.ppp, y + w / 2.0),
            )
        };
        c.fill_rect(tick(left, yo), color);
        c.fill_rect(tick(right, yc), color);
    }
}

fn line_color(s: &Scene<'_>) -> Color32 {
    s.theme.series_palette[0]
}

fn area_fill(c: &mut dyn Canvas, s: &Scene<'_>, pts: &[Pos2]) {
    let color = line_color(s);
    let alpha = if s.theme.dark { 0.28 } else { 0.22 };
    c.fill_area(
        pts,
        s.vp.plot.bottom,
        s.vp.plot.top,
        with_alpha(color, alpha),
        with_alpha(color, 0.0),
    );
}

fn line(c: &mut dyn Canvas, s: &Scene<'_>, area: bool) {
    let pts: Vec<Pos2> = s
        .bars
        .iter()
        .map(|b| pos2(b.x, s.vp.y_of(b.close)))
        .collect();
    if area {
        area_fill(c, s, &pts);
    }
    c.polyline(&pts, 1.5, line_color(s));
}

/// Zoomed far out: one aggregated column per pixel.
fn decimated(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom) {
    let vp = &s.vp;
    match s.chart_type {
        t if t.is_line_like() => {
            let color = line_color(s);
            let pts: Vec<Pos2> = s.bars.iter().map(|b| pos2(b.x, vp.y_of(b.close))).collect();
            if matches!(s.chart_type, ChartType::Area | ChartType::Baseline) {
                area_fill(c, s, &pts);
            }
            // Min/max envelope per column keeps spikes visible; then the close path.
            for b in s.bars {
                let col = g.column(
                    g.center_px(b.x),
                    1.0,
                    vp.y_of(b.close_max),
                    vp.y_of(b.close_min),
                );
                c.fill_rect(col, color);
            }
            c.polyline(&pts, 1.0, color);
        }
        _ => {
            for b in s.bars {
                let (color, _) = s.colors(b.close >= b.open);
                let col = g.column(g.center_px(b.x), 1.0, vp.y_of(b.high), vp.y_of(b.low));
                c.fill_rect(col, color);
            }
        }
    }
}
