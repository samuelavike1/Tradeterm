//! Shapes: rectangle (and later circle, ellipse, polyline).

use super::Drawing;
use super::props::{Props, ShapeProps};
use super::render::styled_line;
use crate::render::Canvas;
use emath::{Pos2, Rect, pos2};

/// The rectangle: background (`ShapeProps::background`), border, optional
/// middle line, extended left / right to the pane's edges.
pub(super) fn rectangle(c: &mut dyn Canvas, d: &Drawing, pts: &[Pos2], pane: Rect) {
    let fallback;
    let sp = match &d.props {
        Props::Shape(p) => p,
        _ => {
            fallback = ShapeProps::default();
            &fallback
        }
    };
    let r = Rect::from_two_pos(pts[0], pts[1]);
    let (left, right) = (
        if sp.extend.left {
            pane.left()
        } else {
            r.left()
        },
        if sp.extend.right {
            pane.right()
        } else {
            r.right()
        },
    );
    let wide = Rect::from_min_max(pos2(left, r.top()), pos2(right, r.bottom()));
    if sp.background.on {
        c.fill_rect(wide, sp.background.color.to_color32());
    }
    let (w, col, style) = (
        sp.border.width as f32,
        sp.border.color.to_color32(),
        sp.border.style,
    );
    // Top and bottom run the extended width; the sides stay at the points.
    styled_line(c, wide.left_top(), wide.right_top(), w, col, style);
    styled_line(c, wide.left_bottom(), wide.right_bottom(), w, col, style);
    if !sp.extend.left {
        styled_line(c, r.left_top(), r.left_bottom(), w, col, style);
    }
    if !sp.extend.right {
        styled_line(c, r.right_top(), r.right_bottom(), w, col, style);
    }
    if sp.middle_line_on {
        let m = &sp.middle_line;
        let y = r.center().y;
        styled_line(
            c,
            pos2(left, y),
            pos2(right, y),
            m.width as f32,
            m.color.to_color32(),
            m.style,
        );
    }
}
