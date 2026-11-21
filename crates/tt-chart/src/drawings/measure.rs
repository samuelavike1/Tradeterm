//! Measurers: price range, date range, date and price range, drawn from
//! `RangeProps`.
//!
//! A box between the two points (extended: the price range left / right,
//! the date range up / down, using `extend.left` / `extend.right` as top /
//! bottom), the edge lines of the measured axis, arrows along the measured
//! axes and a label with the stats. Price-measuring tools switch to the
//! down color when measuring down (TradingView tints them red).

use super::props::{Props, RangeProps};
use super::render::styled_line;
use super::stats::{self, Axes, Lines, StatInputs};
use super::{Drawing, DrawingTool};
use crate::draw::Scene;
use crate::render::Canvas;
use crate::style::Rgba;
use ecolor::Color32;
use emath::{Align2, Pos2, Rect, pos2};
use std::fmt::Write;

pub(super) fn axes(tool: DrawingTool) -> Axes {
    match tool {
        DrawingTool::PriceRange => Axes {
            price: true,
            time: false,
        },
        DrawingTool::DateRange => Axes {
            price: false,
            time: true,
        },
        _ => Axes::BOTH,
    }
}

fn props(d: &Drawing) -> Option<&RangeProps> {
    match &d.props {
        Props::Range(p) => Some(p),
        _ => None,
    }
}

/// The measurer's box: between the two points, extended to the pane's
/// sides (price range) or top / bottom (date range) when set.
pub(super) fn measurer_rect(d: &Drawing, pts: &[Pos2], pane: Rect) -> Rect {
    let r = Rect::from_two_pos(pts[0], pts[1]);
    let Some(rp) = props(d) else {
        return r;
    };
    let (a, b) = (rp.extend.left, rp.extend.right);
    match d.tool {
        DrawingTool::PriceRange => Rect::from_min_max(
            pos2(if a { pane.left() } else { r.left() }, r.top()),
            pos2(if b { pane.right() } else { r.right() }, r.bottom()),
        ),
        DrawingTool::DateRange => Rect::from_min_max(
            pos2(r.left(), if a { pane.top() } else { r.top() }),
            pos2(r.right(), if b { pane.bottom() } else { r.bottom() }),
        ),
        _ => r,
    }
}

/// `c`'s opacity with the down hue when measuring down.
fn tint(c: Rgba, down: Option<Rgba>) -> Color32 {
    match down {
        Some(d) => c.with_rgb_of(d).to_color32(),
        None => c.to_color32(),
    }
}

/// Sum of the volumes of the bars from slot `i0` to `i1` (inclusive,
/// clamped to the series).
pub(super) fn volume_between(volumes: &[f64], i0: f32, i1: f32) -> f64 {
    let (lo, hi) = (i0.min(i1).round(), i0.max(i1).round());
    if volumes.is_empty() || hi < 0.0 {
        return 0.0;
    }
    let last = volumes.len() - 1;
    let lo = (lo.max(0.0) as usize).min(last);
    let hi = (hi as usize).min(last);
    volumes[lo..=hi].iter().filter(|v| v.is_finite()).sum()
}

pub(super) fn draw(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    let Some(rp) = props(d) else {
        return;
    };
    let (a, b) = (d.points[0], d.points[1]);
    let ax = axes(d.tool);
    let up = b.price >= a.price;
    let down = (ax.price && !up).then_some(rp.down_color);
    let line = tint(rp.line.color, down);
    let lw = rp.line.width as f32;
    let pane = s.vp_rect();
    let r = measurer_rect(d, pts, pane);
    if rp.background.on {
        c.fill_rect(r, tint(rp.background.color, down));
    }
    if rp.border_on {
        let bc = tint(rp.border.color, down);
        let bw = rp.border.width as f32;
        let k = [
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
            r.left_top(),
        ];
        for e in k.windows(2) {
            styled_line(c, e[0], e[1], bw, bc, rp.border.style);
        }
    }
    // Edge lines across the measured axis.
    match d.tool {
        DrawingTool::PriceRange => {
            for y in [pts[0].y, pts[1].y] {
                c.line(pos2(r.left(), y), pos2(r.right(), y), lw, line);
            }
        }
        DrawingTool::DateRange => {
            for x in [pts[0].x, pts[1].x] {
                c.line(pos2(x, r.top()), pos2(x, r.bottom()), lw, line);
            }
        }
        _ => {}
    }
    let mid = Rect::from_two_pos(pts[0], pts[1]).center();
    if ax.price {
        arrow(c, pos2(mid.x, pts[0].y), pos2(mid.x, pts[1].y), lw, line);
    }
    if ax.time {
        arrow(c, pos2(pts[0].x, mid.y), pos2(pts[1].x, mid.y), lw, line);
    }

    let volume = (ax.time && rp.stats.volume).then(|| {
        volume_between(
            s.display.volumes(),
            s.vp.index_at(pts[0].x),
            s.vp.index_at(pts[1].x),
        )
    });
    let inp = StatInputs {
        from: a,
        to: b,
        bars: ((pts[1].x - pts[0].x) / s.vp.bar_spacing).round() as i64,
        angle_deg: stats::angle_deg(pts[0], pts[1]),
        precision: s.precision,
        tick: stats::tick_or_precision(s.tick_size, s.precision),
        volume,
    };
    let mut lines = Lines::default();
    let mut st = rp.stats;
    st.angle = false;
    stats::stats_lines(&st, ax, &inp, &mut lines);
    if rp.text.on && !d.text.is_empty() {
        // The first line of the text (the label is one block).
        let first = d.text.lines().next().unwrap_or("");
        if let Some(buf) = lines.push() {
            let mut end = first.len().min(90);
            while !first.is_char_boundary(end) {
                end -= 1;
            }
            let _ = buf.write_str(&first[..end]);
        }
    }
    if lines.is_empty() {
        return;
    }
    // Below the box when measuring down, above when up (TradingView).
    let (at, align) = if up {
        (pos2(mid.x, r.top() - 6.0), Align2::CENTER_BOTTOM)
    } else {
        (pos2(mid.x, r.bottom() + 6.0), Align2::CENTER_TOP)
    };
    let bg = rp
        .label_background
        .on
        .then(|| tint(rp.label_background.color, down));
    stats::draw_box(
        c,
        &lines,
        at,
        align,
        rp.label.size as f32,
        rp.label.bold,
        rp.label.color.to_color32(),
        bg,
    );
}

fn arrow(c: &mut dyn Canvas, from: Pos2, to: Pos2, width: f32, col: Color32) {
    c.line(from, to, width, col);
    let d = to - from;
    if d.length() < 8.0 {
        return;
    }
    c.arrow_head(to, d.normalized(), 6.0 + width, width, col);
}
