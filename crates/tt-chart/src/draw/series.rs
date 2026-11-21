//! Main series drawing: one function per chart type, plus the pixel-aligned
//! bar geometry they share.
//!
//! Every type draws from its own style struct in `style.symbol` (see
//! [`crate::style::SymbolStyle`]). Color slots are resolved against the
//! theme once per frame, at the top of each function, never per bar.

use super::{Scene, crisp};
use crate::ChartType;
use crate::bars::{DrawBar, max_volume};
use crate::render::Canvas;
use crate::scale::format_price;
use crate::style::{BrickColors, CandleStyle, LineKind, resolve};
use ecolor::Color32;
use emath::{Align2, Pos2, Rect, pos2};
use tt_theme::{Theme, mix, with_alpha};

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
        let body_px = odd((spacing * ppp * 0.72).floor());
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

/// The largest odd pixel count ≤ `px` (at least 1).
fn odd(px: f32) -> f32 {
    let px = px.max(1.0);
    if px as i32 % 2 == 0 { px - 1.0 } else { px }
}

pub(super) fn series(c: &mut dyn Canvas, s: &Scene<'_>) {
    let g = BarGeom::new(s.vp.bar_spacing, c.pixels_per_point());
    if s.decimated {
        decimated(c, s, &g);
        return;
    }
    match s.chart_type {
        ChartType::Candles | ChartType::HeikinAshi => candles(c, s, &g, CandleMode::Plain),
        ChartType::HollowCandles => candles(c, s, &g, CandleMode::Hollow),
        ChartType::VolumeCandles => candles(c, s, &g, CandleMode::Volume),
        ChartType::Bars => ohlc_bars(c, s, &g),
        ChartType::Line => line(c, s),
        ChartType::LineWithMarkers => {
            line(c, s);
            markers(c, s);
        }
        ChartType::StepLine => step_line(c, s),
        ChartType::Area => area(c, s),
        ChartType::HlcArea => hlc_area(c, s),
        ChartType::Baseline => baseline(c, s),
        ChartType::Columns => columns(c, s, &g),
        ChartType::HighLow => high_low(c, s, &g),
        ChartType::Renko | ChartType::LineBreak => bricks(c, s),
    }
}

// ---- resolved styles (once per frame) ------------------------------------

/// Resolved candle colors, indexed by `up as usize`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CandleColors {
    pub body: [Color32; 2],
    /// `None` slots follow the body color.
    pub border: [Color32; 2],
    pub wick: [Color32; 2],
}

pub(crate) fn candle_colors(st: &CandleStyle, t: &Theme) -> CandleColors {
    let body = [resolve(st.body_down, t.down), resolve(st.body_up, t.up)];
    CandleColors {
        body,
        border: [
            resolve(st.border_down, body[0]),
            resolve(st.border_up, body[1]),
        ],
        wick: [
            resolve(st.wick_down, t.down_wick),
            resolve(st.wick_up, t.up_wick),
        ],
    }
}

/// Resolved brick colors, indexed `[projection as usize][up as usize]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BrickPalette {
    pub fill: [[Color32; 2]; 2],
    pub border: [[Color32; 2]; 2],
}

pub(crate) fn brick_palette(bc: &BrickColors, t: &Theme) -> BrickPalette {
    let (up, down) = (resolve(bc.up, t.up), resolve(bc.down, t.down));
    let (pu, pd) = (
        resolve(bc.proj_up, mix(up, t.bg, 0.6)),
        resolve(bc.proj_down, mix(down, t.bg, 0.6)),
    );
    BrickPalette {
        fill: [[down, up], [pd, pu]],
        border: [
            [resolve(bc.border_down, down), resolve(bc.border_up, up)],
            [
                resolve(bc.proj_border_down, pd),
                resolve(bc.proj_border_up, pu),
            ],
        ],
    }
}

/// A resolved line: color, width and line style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LineLook {
    pub color: Color32,
    pub width: f32,
    pub kind: LineKind,
}

/// The line of the line family and Area (`series_palette[0]` by default).
pub(crate) fn line_look(s: &Scene<'_>) -> LineLook {
    let p0 = s.theme.series_palette[0];
    let y = &s.style.symbol;
    let (color, width, kind) = match s.chart_type {
        ChartType::Area => (y.area.line, y.area.width, y.area.kind),
        t => {
            let l = y.line(t);
            (l.color, l.width, l.kind)
        }
    };
    LineLook {
        color: resolve(color, p0),
        width: line_width(width),
        kind,
    }
}

fn line_width(w: f32) -> f32 {
    if w.is_finite() {
        w.clamp(0.5, 10.0)
    } else {
        2.0
    }
}

/// A polyline in a [`LineKind`] (dash lengths scale with the width).
fn stroke(c: &mut dyn Canvas, pts: &[Pos2], width: f32, color: Color32, kind: LineKind) {
    let w = width.max(1.0);
    match kind {
        LineKind::Solid => c.polyline(pts, width, color),
        LineKind::Dashed => c.dashed_polyline(pts, width, color, 6.0 * w, 4.0 * w),
        LineKind::Dotted => c.dashed_polyline(pts, width, color, w, 2.0 * w),
    }
}

/// Up / down colors of a bar-like type, for decimated columns.
fn up_down(s: &Scene<'_>) -> [Color32; 2] {
    let (y, t) = (&s.style.symbol, s.theme);
    match s.chart_type {
        ChartType::Bars => [resolve(y.bars.down, t.down), resolve(y.bars.up, t.up)],
        ChartType::HighLow => [resolve(y.high_low.body, t.series_palette[0]); 2],
        ChartType::Renko => brick_palette(&y.renko.colors, t).fill[0],
        ChartType::LineBreak => brick_palette(&y.line_break.colors, t).fill[0],
        ChartType::HlcArea => [t.down, t.up],
        ty => candle_colors(y.candle(ty), t).body,
    }
}

fn source_points(s: &Scene<'_>) -> Vec<Pos2> {
    s.bars
        .iter()
        .map(|b| pos2(b.x, s.vp.y_of(b.value)))
        .collect()
}

// ---- candles and bars ----------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandleMode {
    /// Candles, Heikin Ashi.
    Plain,
    /// Hollow when rising; colored by close vs the previous close.
    Hollow,
    /// Body width proportional to volume (vs. the visible max).
    Volume,
}

fn candles(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom, mode: CandleMode) {
    let vp = &s.vp;
    let st = s.style.symbol.candle(s.chart_type);
    let col = candle_colors(st, s.theme);
    let by_prev_close = st.color_by_prev_close;
    let max_vol = if mode == CandleMode::Volume {
        max_volume(s.bars).max(f64::MIN_POSITIVE)
    } else {
        1.0
    };
    // Volume candles may be wider: the busiest bar fills its slot.
    let full_px = (vp.bar_spacing * g.ppp * 0.92).floor().max(1.0);
    let border_w = 1.0 / g.ppp;
    let hollow_w = g.wick_px / g.ppp;
    for b in s.bars {
        let up = match mode {
            CandleMode::Hollow => b.close >= b.prev_close,
            _ => Scene::rising(by_prev_close, b),
        } as usize;
        let hollow = mode == CandleMode::Hollow && b.close >= b.open;
        let body_px = match mode {
            CandleMode::Volume => odd((full_px * (b.volume / max_vol) as f32).round()),
            _ => g.body_px,
        };
        let wick_px = g.wick_px.min(body_px);
        let cx = g.center_px(b.x);
        let (yh, yl) = (vp.y_of(b.high), vp.y_of(b.low));
        if body_px <= g.wick_px && mode != CandleMode::Volume {
            // Too narrow for a body: one line.
            let color = if st.wick {
                col.wick[up]
            } else if st.body {
                col.body[up]
            } else {
                col.border[up]
            };
            c.fill_rect(g.column(cx, wick_px, yh, yl), color);
            continue;
        }
        let (y0, y1) = g.span(vp.y_of(b.open), vp.y_of(b.close));
        let filled = st.body && !hollow;
        if st.wick {
            if filled {
                // One rect; the body covers its middle.
                c.fill_rect(g.column(cx, wick_px, yh, yl), col.wick[up]);
            } else {
                c.fill_rect(g.column(cx, wick_px, yh, y0), col.wick[up]);
                c.fill_rect(g.column(cx, wick_px, y1, yl), col.wick[up]);
            }
        }
        let rect = g.column(cx, body_px, y0, y1);
        if filled {
            c.fill_rect(rect, col.body[up]);
        }
        // Borders only cost a primitive when they differ from the body.
        if st.borders && (!filled || col.border[up] != col.body[up]) {
            let w = if hollow { hollow_w } else { border_w };
            c.stroke_rect(rect, w, col.border[up]);
        }
    }
}

fn ohlc_bars(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom) {
    let vp = &s.vp;
    let st = &s.style.symbol.bars;
    let colors = [resolve(st.down, s.theme.down), resolve(st.up, s.theme.up)];
    // Thin bars: 1 pt regardless of zoom; otherwise the line grows with it.
    let line_px = if st.thin {
        g.wick_px
    } else {
        odd((g.body_px / 4.0).floor()).max(g.wick_px).min(g.body_px)
    };
    let tick_px = ((g.body_px - line_px) / 2.0).floor().max(1.0);
    let w = line_px / g.ppp;
    let half_line = (line_px / 2.0).floor();
    for b in s.bars {
        let color = colors[Scene::rising(st.color_by_prev_close, b) as usize];
        let cx = g.center_px(b.x);
        c.fill_rect(
            g.column(cx, line_px, vp.y_of(b.high), vp.y_of(b.low)),
            color,
        );
        let tick = |x0: f32, y: f32| {
            Rect::from_min_max(
                pos2(x0, y - w / 2.0),
                pos2(x0 + tick_px / g.ppp, y + w / 2.0),
            )
        };
        // Open tick to the left (not for HLC bars), close tick to the right.
        if !st.hlc {
            let left = (cx - half_line - tick_px) / g.ppp;
            c.fill_rect(tick(left, crisp(vp.y_of(b.open), g.ppp)), color);
        }
        let right = (cx - half_line + line_px) / g.ppp;
        c.fill_rect(tick(right, crisp(vp.y_of(b.close), g.ppp)), color);
    }
}

// ---- line family ---------------------------------------------------------

fn line(c: &mut dyn Canvas, s: &Scene<'_>) {
    let l = line_look(s);
    stroke(c, &source_points(s), l.width, l.color, l.kind);
}

/// A dot on each value (when bars are far enough apart to see them).
fn markers(c: &mut dyn Canvas, s: &Scene<'_>) {
    if s.vp.bar_spacing < 6.0 {
        return;
    }
    let l = line_look(s);
    let r = (s.vp.bar_spacing * 0.18).clamp(2.0, 3.5).max(l.width);
    for b in s.bars {
        c.fill_circle(pos2(b.x, s.vp.y_of(b.value)), r, l.color);
    }
}

fn step_line(c: &mut dyn Canvas, s: &Scene<'_>) {
    let l = line_look(s);
    let half = s.vp.bar_spacing / 2.0;
    let mut pts = Vec::with_capacity(s.bars.len() * 2);
    for b in s.bars {
        let y = s.vp.y_of(b.value);
        pts.push(pos2(b.x - half, y));
        pts.push(pos2(b.x + half, y));
    }
    stroke(c, &pts, l.width, l.color, l.kind);
}

fn area_fill(c: &mut dyn Canvas, s: &Scene<'_>, pts: &[Pos2], line: Color32) {
    let st = &s.style.symbol.area;
    c.fill_area(
        pts,
        s.vp.plot.bottom,
        s.vp.plot.top,
        resolve(st.fill_top, with_alpha(line, 0.28)),
        resolve(st.fill_bottom, with_alpha(line, 0.0)),
    );
}

fn area(c: &mut dyn Canvas, s: &Scene<'_>) {
    let l = line_look(s);
    let pts = source_points(s);
    area_fill(c, s, &pts, l.color);
    stroke(c, &pts, l.width, l.color, l.kind);
}

/// High, low and close lines with bands between.
fn hlc_area(c: &mut dyn Canvas, s: &Scene<'_>) {
    let (vp, t) = (&s.vp, s.theme);
    let st = &s.style.symbol.hlc_area;
    let pts = |f: fn(&DrawBar) -> f64| -> Vec<Pos2> {
        s.bars.iter().map(|b| pos2(b.x, vp.y_of(f(b)))).collect()
    };
    let (high, low, close) = (pts(|b| b.high), pts(|b| b.low), pts(|b| b.close));
    c.fill_between(
        &high,
        &close,
        resolve(st.fill_high_close, with_alpha(t.up, 0.2)),
    );
    c.fill_between(
        &close,
        &low,
        resolve(st.fill_close_low, with_alpha(t.down, 0.2)),
    );
    for (p, line, token) in [
        (&high, &st.high, t.up),
        (&low, &st.low, t.down),
        (&close, &st.close, t.text_muted),
    ] {
        stroke(
            c,
            p,
            line_width(line.width),
            resolve(line.color, token),
            line.kind,
        );
    }
}

/// Values against a base level at `level_pct` of the visible price range
/// (from the bottom): up colors above, down colors below.
fn baseline(c: &mut dyn Canvas, s: &Scene<'_>) {
    let (vp, t) = (&s.vp, s.theme);
    let st = &s.style.symbol.baseline;
    let pct = if st.level_pct.is_finite() {
        (st.level_pct as f64 / 100.0).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let base = vp.price_min + (vp.price_max - vp.price_min) * pct;
    let yb = vp.y_of(base);
    // Points, with exact crossing points inserted so fills split cleanly.
    let mut pts: Vec<Pos2> = Vec::with_capacity(s.bars.len() + 8);
    for b in s.bars {
        let p = pos2(b.x, vp.y_of(b.value));
        if let Some(prev) = pts.last().copied()
            && (prev.y - yb) * (p.y - yb) < 0.0
        {
            let k = (yb - prev.y) / (p.y - prev.y);
            pts.push(pos2(prev.x + (p.x - prev.x) * k, yb));
        }
        pts.push(p);
    }
    let rect = s.vp_rect();
    let above: Vec<Pos2> = pts.iter().map(|p| pos2(p.x, p.y.min(yb))).collect();
    let below: Vec<Pos2> = pts.iter().map(|p| pos2(p.x, p.y.max(yb))).collect();
    // Gradients: fill 1 near the line (top) / near the base (bottom).
    c.fill_area(
        &above,
        yb,
        rect.top(),
        resolve(st.top_fill_1, with_alpha(t.up, 0.28)),
        resolve(st.top_fill_2, with_alpha(t.up, 0.05)),
    );
    c.fill_area(
        &below,
        yb,
        rect.bottom(),
        resolve(st.bottom_fill_2, with_alpha(t.down, 0.28)),
        resolve(st.bottom_fill_1, with_alpha(t.down, 0.05)),
    );
    c.dashed_line(
        pos2(rect.left(), yb),
        pos2(rect.right(), yb),
        1.0,
        resolve(st.base_color, t.text_muted),
        3.0,
        3.0,
    );
    // The line itself: top color above the base, bottom color below.
    c.set_clip(Rect::from_min_max(rect.min, pos2(rect.right(), yb)));
    c.polyline(&pts, line_width(st.top_width), resolve(st.top_line, t.up));
    c.set_clip(Rect::from_min_max(pos2(rect.left(), yb), rect.max));
    c.polyline(
        &pts,
        line_width(st.bottom_width),
        resolve(st.bottom_line, t.down),
    );
    c.set_clip(rect);
}

// ---- columns, high-low, bricks -------------------------------------------

/// Columns from the bottom of the price area up to each value.
fn columns(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom) {
    let (st, t) = (&s.style.symbol.columns, s.theme);
    let colors = [
        resolve(st.down, with_alpha(t.down, 0.5)),
        resolve(st.up, with_alpha(t.up, 0.5)),
    ];
    let bottom = s.vp.plot.bottom;
    let width = if s.decimated { 1.0 } else { g.body_px };
    for b in s.bars {
        let up = if st.color_by_prev_close {
            b.value >= b.prev_value
        } else {
            b.close >= b.open
        };
        // Decimated: up to the column's highest value (keeps spikes).
        let top = if s.decimated { b.value_max } else { b.value };
        let col = g.column(g.center_px(b.x), width, s.vp.y_of(top), bottom);
        c.fill_rect(col, colors[up as usize]);
    }
}

/// Bars spanning each high-low range, with optional borders and labels.
fn high_low(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom) {
    let st = &s.style.symbol.high_low;
    let p0 = s.theme.series_palette[0];
    let (body, border, label) = (
        resolve(st.body, p0),
        resolve(st.border, p0),
        resolve(st.label, p0),
    );
    let stroke_border = st.borders && border != body && g.body_px >= 3.0;
    for b in s.bars {
        let (y0, y1) = g.span(s.vp.y_of(b.high), s.vp.y_of(b.low));
        let rect = g.column(g.center_px(b.x), g.body_px, y0, y1);
        c.fill_rect(rect, body);
        if stroke_border {
            c.stroke_rect(rect, 1.0 / g.ppp, border);
        }
    }
    // Labels only when a typical one fits in a bar slot.
    let Some(last) = s.bars.last().filter(|_| st.labels) else {
        return;
    };
    let size = (s.style.scales.text_size - 1.0).max(7.0);
    let typical = c.text_size(&format_price(last.high, s.precision), size).x;
    if typical + 4.0 > s.vp.bar_spacing {
        return;
    }
    for b in s.bars {
        let (yh, yl) = (s.vp.y_of(b.high), s.vp.y_of(b.low));
        let (hi, lo) = (
            format_price(b.high, s.precision),
            format_price(b.low, s.precision),
        );
        c.text(pos2(b.x, yh - 2.0), Align2::CENTER_BOTTOM, &hi, size, label);
        c.text(pos2(b.x, yl + 2.0), Align2::CENTER_TOP, &lo, size, label);
    }
}

/// Renko / Line break bricks: adjacent bodies, projection (forming) bricks
/// in their own colors, Renko wicks when enabled.
fn bricks(c: &mut dyn Canvas, s: &Scene<'_>) {
    let (y, t) = (&s.style.symbol, s.theme);
    let renko = s.chart_type == ChartType::Renko;
    let pal = brick_palette(
        if renko {
            &y.renko.colors
        } else {
            &y.line_break.colors
        },
        t,
    );
    let wicks = renko && y.renko.inputs.wicks;
    let wick = [
        resolve(y.renko.wick_down, t.down_wick),
        resolve(y.renko.wick_up, t.up_wick),
    ];
    let proj_from = s.projection_from.unwrap_or(usize::MAX);
    let ppp = c.pixels_per_point();
    let w = (s.vp.bar_spacing * ppp * 0.9).floor().max(1.0) / ppp;
    let wick_w = ppp.round().max(1.0) / ppp;
    for (k, b) in s.bars.iter().enumerate() {
        let up = (b.close >= b.open) as usize;
        let proj = (s.range.start + k >= proj_from) as usize;
        let (y0, y1) = (s.vp.y_of(b.open), s.vp.y_of(b.close));
        let (top, bottom) = (y0.min(y1), y0.max(y1).max(y0.min(y1) + 1.0 / ppp));
        if wicks {
            let (yh, yl) = (s.vp.y_of(b.high), s.vp.y_of(b.low));
            let x0 = ((b.x - wick_w / 2.0) * ppp).round() / ppp;
            if yh.min(yl) < top {
                let r = Rect::from_min_max(pos2(x0, yh.min(yl)), pos2(x0 + wick_w, top));
                c.fill_rect(r, wick[up]);
            }
            if yh.max(yl) > bottom {
                let r = Rect::from_min_max(pos2(x0, bottom), pos2(x0 + wick_w, yh.max(yl)));
                c.fill_rect(r, wick[up]);
            }
        }
        let rect = Rect::from_min_max(pos2(b.x - w / 2.0, top), pos2(b.x + w / 2.0, bottom));
        let fill = pal.fill[proj][up];
        c.fill_rect(rect, fill);
        let border = pal.border[proj][up];
        if border != fill && w * ppp >= 3.0 {
            c.stroke_rect(rect, 1.0 / ppp, border);
        }
    }
}

// ---- decimated -----------------------------------------------------------

/// Zoomed far out: one aggregated column per pixel.
fn decimated(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom) {
    let vp = &s.vp;
    match s.chart_type {
        ChartType::Line | ChartType::LineWithMarkers | ChartType::StepLine | ChartType::Area => {
            let l = line_look(s);
            let pts = source_points(s);
            if s.chart_type == ChartType::Area {
                area_fill(c, s, &pts, l.color);
            }
            // Min/max envelope of the source per column keeps spikes
            // visible; then the path through the last value of each.
            for b in s.bars {
                let col = g.column(
                    g.center_px(b.x),
                    1.0,
                    vp.y_of(b.value_max),
                    vp.y_of(b.value_min),
                );
                c.fill_rect(col, l.color);
            }
            stroke(c, &pts, l.width, l.color, l.kind);
        }
        ChartType::Baseline => baseline(c, s),
        ChartType::HlcArea => hlc_area(c, s),
        ChartType::Columns => columns(c, s, g),
        _ => {
            let colors = up_down(s);
            for b in s.bars {
                let col = g.column(g.center_px(b.x), 1.0, vp.y_of(b.high), vp.y_of(b.low));
                c.fill_rect(col, colors[(b.close >= b.open) as usize]);
            }
        }
    }
}

#[cfg(test)]
mod tests;
