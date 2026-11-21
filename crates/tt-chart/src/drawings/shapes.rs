//! Shapes: rectangle (and later circle, ellipse, polyline).

use super::Drawing;
use super::render::styled_line;
use crate::render::Canvas;
use emath::{Pos2, Rect};
use tt_theme::with_alpha;

/// The fill still follows the border color at 20 % (as before props); the
/// shapes slice switches it to `ShapeProps::background`.
pub(super) fn rectangle(c: &mut dyn Canvas, d: &Drawing, pts: &[Pos2]) {
    let st = d.props.primary_stroke();
    let (w, col) = (st.width as f32, st.color.to_color32());
    let r = Rect::from_two_pos(pts[0], pts[1]);
    c.fill_rect(r, with_alpha(col, 0.2));
    let corners = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    for e in corners.windows(2) {
        styled_line(c, e[0], e[1], w, col, st.style);
    }
}
