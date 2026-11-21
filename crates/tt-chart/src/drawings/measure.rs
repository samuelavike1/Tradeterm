//! Measurers: price range, date range, date and price range.
//!
//! Reads only the main line color from `RangeProps`; the stats checklist,
//! border, label and extend settings come with slice G.

use super::model::RED;
use super::stats::format_span;
use super::{Drawing, DrawingTool};
use crate::draw::Scene;
use crate::render::Canvas;
use crate::scale::{format_percent, format_signed_price};
use ecolor::Color32;
use emath::{Align2, Pos2, Rect, pos2, vec2};
use tt_theme::with_alpha;

/// The shaded area of a measurer: like TradingView, the box between the two
/// points for all three (price / date range only label one axis).
pub(super) fn measurer_rect(_tool: DrawingTool, pts: &[Pos2], _pane: Rect) -> Rect {
    Rect::from_two_pos(pts[0], pts[1])
}

/// A shaded box between the two points, arrows along the measured axes and
/// a label with the numbers.
pub(super) fn draw(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    let (a, b) = (d.points[0], d.points[1]);
    let up = b.price >= a.price;
    let col = if d.tool == DrawingTool::DateRange || up {
        d.color()
    } else {
        RED
    };
    let r = Rect::from_two_pos(pts[0], pts[1]);
    c.fill_rect(r, with_alpha(col, 0.2));
    let mid = r.center();
    let (price, date) = match d.tool {
        DrawingTool::PriceRange => (true, false),
        DrawingTool::DateRange => (false, true),
        _ => (true, true),
    };
    if price {
        arrow(c, pos2(mid.x, pts[0].y), pos2(mid.x, pts[1].y), col);
    }
    if date {
        arrow(c, pos2(pts[0].x, mid.y), pos2(pts[1].x, mid.y), col);
    }
    let mut lines = Vec::new();
    if price {
        let diff = b.price - a.price;
        let pct = if a.price != 0.0 {
            diff / a.price * 100.0
        } else {
            0.0
        };
        lines.push(format!(
            "{} ({})",
            format_signed_price(diff, s.precision),
            format_percent(pct)
        ));
    }
    if date {
        let bars = ((pts[1].x - pts[0].x) / s.vp.bar_spacing).round() as i64;
        lines.push(format!(
            "{bars} bars, {}",
            format_span(b.time_ms - a.time_ms)
        ));
    }
    let size = s.style.scales.text_size;
    let line_h = size + 4.0;
    let width = lines
        .iter()
        .map(|l| c.text_size(l, size).x)
        .fold(0.0, f32::max)
        + 16.0;
    let h = line_h * lines.len() as f32 + 8.0;
    // Below the box when measuring down, above when up (TradingView).
    let top = if up {
        r.top() - h - 6.0
    } else {
        r.bottom() + 6.0
    };
    let label = Rect::from_min_size(pos2(mid.x - width / 2.0, top), vec2(width, h));
    c.fill_rounded_rect(label, 4.0, col);
    for (i, l) in lines.iter().enumerate() {
        c.text(
            pos2(
                label.center().x,
                label.top() + 4.0 + line_h * (i as f32 + 0.5),
            ),
            Align2::CENTER_CENTER,
            l,
            size,
            s.theme.text_on_accent,
        );
    }
}

fn arrow(c: &mut dyn Canvas, from: Pos2, to: Pos2, col: Color32) {
    c.line(from, to, 1.0, col);
    let d = to - from;
    if d.length() < 8.0 {
        return;
    }
    c.arrow_head(to, d.normalized(), 7.0, 1.0, col);
}
