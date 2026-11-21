//! Line tools: trend line, ray, extended line, horizontal line / ray,
//! vertical line, cross line (and later info line, trend angle, arrow).

use super::Drawing;
use super::hit::extend;
use super::render::styled_line;
use crate::draw::Scene;
use crate::render::Canvas;
use emath::{Pos2, pos2};

pub(super) fn draw(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    use super::DrawingTool::*;
    let pane = s.vp_rect();
    let st = d.props.primary_stroke();
    let (w, col, style) = (st.width as f32, st.color.to_color32(), st.style);
    match d.tool {
        TrendLine if pts.len() >= 2 => styled_line(c, pts[0], pts[1], w, col, style),
        Ray if pts.len() >= 2 => {
            let (a, b) = extend(pts[0], pts[1], pane, false, true);
            styled_line(c, a, b, w, col, style);
        }
        ExtendedLine if pts.len() >= 2 => {
            let (a, b) = extend(pts[0], pts[1], pane, true, true);
            styled_line(c, a, b, w, col, style);
        }
        HorizontalLine => styled_line(
            c,
            pos2(pane.left(), pts[0].y),
            pos2(pane.right(), pts[0].y),
            w,
            col,
            style,
        ),
        HorizontalRay => styled_line(c, pts[0], pos2(pane.right(), pts[0].y), w, col, style),
        VerticalLine | CrossLine => {
            c.set_clip(s.plot);
            styled_line(
                c,
                pos2(pts[0].x, s.plot.top()),
                pos2(pts[0].x, s.plot.bottom()),
                w,
                col,
                style,
            );
            c.set_clip(pane);
            if d.tool == CrossLine {
                styled_line(
                    c,
                    pos2(pane.left(), pts[0].y),
                    pos2(pane.right(), pts[0].y),
                    w,
                    col,
                    style,
                );
            }
        }
        _ => {}
    }
}
