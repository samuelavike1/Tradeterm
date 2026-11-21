//! Fib retracement and trend-based Fib extension, drawn from `FibProps`.
//!
//! Level prices are `base + span × coeff`:
//! - retracement: base = point 2 (level 0), span = point 1 − point 2, so
//!   level 1 is at point 1 (TradingView's convention); Reverse swaps them;
//! - extension: base = point 3, span = point 2 − point 1; Reverse measures
//!   back from `point 3 + span`.
//!
//! The interpolation is linear in price, or in log space when "levels based
//! on log scale" is on and the chart is in log scale.

use super::props::{FibProps, HAlign, Props, VAlign};
use super::render::styled_line;
use super::stats::{Buf, write_price};
use super::{Drawing, DrawingTool};
use crate::ScaleMode;
use crate::draw::Scene;
use crate::render::{Canvas, TextOpts};
use crate::viewport::PriceScale;
use emath::{Align, Align2, Pos2, Rect, pos2};
use std::fmt::Write;

/// Levels drawn at most (TradingView has 24 slots).
const MAX_LEVELS: usize = 32;

/// Where the levels come from, in the space they are interpolated in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Levels {
    base: f64,
    span: f64,
    log: bool,
}

impl Levels {
    /// `prices`: the drawing's anchor prices (2 for the retracement, 3 for
    /// the extension).
    pub fn new(tool: DrawingTool, fp: &FibProps, prices: &[f64], scale: PriceScale) -> Levels {
        let log = fp.log_scale && scale.mode == ScaleMode::Logarithmic;
        let t = |p: f64| {
            if log { scale.to_scaled(p) } else { p }
        };
        let (base, span) = match (tool, prices) {
            (DrawingTool::FibExtension, [p1, p2, p3, ..]) => (t(*p3), t(*p2) - t(*p1)),
            (_, [p1, p2, ..]) => (t(*p2), t(*p1) - t(*p2)),
            _ => (0.0, 0.0),
        };
        let (base, span) = if fp.reverse {
            (base + span, -span)
        } else {
            (base, span)
        };
        Levels { base, span, log }
    }

    /// Price of level `coeff`.
    pub fn price(&self, coeff: f64, scale: PriceScale) -> f64 {
        let v = self.base + self.span * coeff;
        if self.log { scale.from_scaled(v) } else { v }
    }
}

fn props(d: &Drawing) -> Option<&FibProps> {
    match &d.props {
        Props::Fib(p) => Some(p),
        _ => None,
    }
}

/// The x range the levels span (before extending): the two points of the
/// retracement; for the extension, from the end of the move / correction
/// to the rightmost point.
pub(super) fn x_range(tool: DrawingTool, pts: &[Pos2]) -> (f32, f32) {
    if tool == DrawingTool::FibExtension && pts.len() >= 3 {
        let x0 = pts[1].x.min(pts[2].x);
        let x1 = pts[0].x.max(pts[1].x).max(pts[2].x);
        (x0, x1)
    } else {
        (pts[0].x.min(pts[1].x), pts[0].x.max(pts[1].x))
    }
}

/// The level label: `0.618 (1,234.50)`, `61.8%`, or the price alone.
pub(super) fn write_label(b: &mut Buf, fp: &FibProps, coeff: f64, price: f64, precision: usize) {
    if fp.show_levels {
        let _ = if fp.percents {
            write_trimmed(b, coeff * 100.0, 1).and_then(|_| b.write_char('%'))
        } else {
            write_trimmed(b, coeff, 3)
        };
    }
    if fp.show_prices {
        if fp.show_levels {
            let _ = b.write_str(" (");
            let _ = write_price(b, price, precision);
            let _ = b.write_char(')');
        } else {
            let _ = write_price(b, price, precision);
        }
    }
}

/// `v` with up to `decimals` decimals, trailing zeros dropped.
fn write_trimmed(b: &mut Buf, v: f64, decimals: usize) -> std::fmt::Result {
    let mut t = Buf::new();
    write!(t, "{v:.decimals$}")?;
    let s = t.as_str();
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s
    };
    b.write_str(if s.is_empty() || s == "-0" { "0" } else { s })
}

/// The shown levels as `(coeff, index)`, sorted by coefficient, into `out`.
fn shown_levels(fp: &FibProps, out: &mut [(f64, usize); MAX_LEVELS]) -> usize {
    let mut n = 0;
    for (i, l) in fp.levels.iter().enumerate() {
        if l.on && l.coeff.is_finite() && n < MAX_LEVELS {
            out[n] = (l.coeff, i);
            n += 1;
        }
    }
    out[..n].sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    n
}

pub(super) fn draw(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    let Some(fp) = props(d) else {
        return;
    };
    let need = if d.tool == DrawingTool::FibExtension {
        3
    } else {
        2
    };
    if pts.len() < need || d.points.len() < need {
        return;
    }
    let sc = s.vp.scale;
    let mut prices = [0.0; 3];
    for (p, a) in prices.iter_mut().zip(&d.points) {
        *p = a.price;
    }
    let lv = Levels::new(d.tool, fp, &prices[..need], sc);
    let pane = s.vp_rect();
    let (x0, x1) = x_range(d.tool, pts);
    let left = if fp.extend.left { pane.left() } else { x0 };
    let right = if fp.extend.right { pane.right() } else { x1 };
    let mut order = [(0.0, 0usize); MAX_LEVELS];
    let n = shown_levels(fp, &mut order);
    let y_of = |coeff: f64| s.vp.y_of(lv.price(coeff, sc));

    // Bands between consecutive levels, in the upper level's color.
    if fp.background_on && fp.background_opacity > 0.0 {
        for w in order[..n].windows(2) {
            let (ya, yb) = (y_of(w[0].0), y_of(w[1].0));
            let r = Rect::from_x_y_ranges(left..=right, ya.min(yb)..=ya.max(yb));
            let col = fp.levels[w[1].1].color;
            let a = col.opacity() * fp.background_opacity.clamp(0.0, 1.0);
            c.fill_rect(r, col.with_opacity(a).to_color32());
        }
    }

    let (w, style) = (fp.levels_line.width as f32, fp.levels_line.style);
    let size = fp.font_size as f32;
    for &(coeff, i) in &order[..n] {
        let price = lv.price(coeff, sc);
        let y = s.vp.y_of(price);
        let col = fp.levels[i].color.to_color32();
        styled_line(c, pos2(left, y), pos2(right, y), w, col, style);
        if !(fp.show_levels || fp.show_prices) {
            continue;
        }
        let mut b = Buf::new();
        write_label(&mut b, fp, coeff, price, s.precision);
        let (x, hx) = match fp.label_h {
            HAlign::Left if fp.extend.left => (pane.left() + 4.0, Align::Min),
            HAlign::Left => (left - 4.0, Align::Max),
            HAlign::Center => ((left + right) / 2.0, Align::Center),
            HAlign::Right if fp.extend.right => (pane.right() - 4.0, Align::Max),
            HAlign::Right => (right + 4.0, Align::Min),
        };
        // Top: above the line, Bottom: below it.
        let (y, vy) = match fp.label_v {
            VAlign::Top => (y - 1.0, Align::Max),
            VAlign::Middle => (y, Align::Center),
            VAlign::Bottom => (y + 1.0, Align::Min),
        };
        c.text_rich(
            pos2(x, y),
            Align2([hx, vy]),
            b.as_str(),
            size,
            col,
            TextOpts::default(),
        );
    }

    if fp.trend_line_on {
        let t = &fp.trend_line;
        let (tw, tc) = (t.width as f32, t.color.to_color32());
        for seg in pts[..need].windows(2) {
            styled_line(c, seg[0], seg[1], tw, tc, t.style);
        }
    }
}

/// Body hit: the levels' box (interpolated on screen, exact unless the
/// levels are linear on a log chart) and the trend line.
pub(super) fn touches(d: &Drawing, pts: &[Pos2], pane: Rect, p: Pos2, hit: f32) -> bool {
    let Some(fp) = props(d) else {
        return false;
    };
    let need = if d.tool == DrawingTool::FibExtension {
        3
    } else {
        2
    };
    if pts.len() < need {
        return false;
    }
    let near_trend = pts[..need]
        .windows(2)
        .any(|w| super::hit::dist_to_segment(p, w[0], w[1]) <= hit);
    if fp.trend_line_on && near_trend {
        return true;
    }
    // Screen-space levels: y(coeff) = y_base + (y_span) × coeff.
    let ys: [f32; 3] = [pts[0].y, pts[1].y, pts.get(2).map_or(0.0, |q| q.y)];
    let (mut base, mut span) = if d.tool == DrawingTool::FibExtension {
        (ys[2], ys[1] - ys[0])
    } else {
        (ys[1], ys[0] - ys[1])
    };
    if fp.reverse {
        base += span;
        span = -span;
    }
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for l in fp.levels.iter().filter(|l| l.on) {
        let y = base + span * l.coeff as f32;
        lo = lo.min(y);
        hi = hi.max(y);
    }
    if !lo.is_finite() {
        return near_trend;
    }
    let (x0, x1) = x_range(d.tool, pts);
    let left = if fp.extend.left { pane.left() } else { x0 };
    let right = if fp.extend.right { pane.right() } else { x1 };
    Rect::from_x_y_ranges(left..=right, lo..=hi)
        .expand(hit)
        .contains(p)
}
