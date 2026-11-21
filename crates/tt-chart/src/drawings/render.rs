//! Drawing dispatch: the base layer (drawings), axis tags and the pointer
//! layer (the drawing being placed, handles). Each tool family draws itself
//! in its own file; this only routes.

use super::{Drawing, DrawingTool, HANDLE_RADIUS, Placement, fib, lines, measure, shapes};
use crate::draw::{Scene, Tag, draw_tag, price_axis_tag};
use crate::render::Canvas;
use crate::scale::format_price;
use ecolor::Color32;
use emath::Pos2;

pub(super) use crate::render::styled_line;

/// Draws the drawings into the base layer (clipped to the price pane;
/// vertical lines span all panes).
pub(crate) fn draw_base(c: &mut dyn Canvas, s: &Scene<'_>) {
    let ds = s.drawings;
    if ds.hidden || s.main_collapsed {
        return;
    }
    for d in &ds.items {
        if d.hidden {
            continue;
        }
        if let Some(pts) = ds.screen_of(d.id) {
            draw_one(c, s, d, pts);
        }
    }
    c.set_clip(s.full);
}

/// Axis tags of horizontal lines / rays (base layer, after the price labels).
pub(crate) fn axis_tags(s: &Scene<'_>) -> Vec<Tag> {
    let ds = s.drawings;
    if ds.hidden || s.main_collapsed {
        return Vec::new();
    }
    let pane = s.vp_rect();
    ds.items
        .iter()
        .filter(|d| {
            !d.hidden
                && matches!(
                    d.tool,
                    DrawingTool::HorizontalLine
                        | DrawingTool::HorizontalRay
                        | DrawingTool::CrossLine
                )
        })
        .filter_map(|d| {
            let y = s.vp.y_of(d.points[0].price);
            pane.y_range().contains(y).then(|| {
                price_axis_tag(
                    s,
                    y,
                    vec![format_price(d.points[0].price, s.precision)],
                    d.color(),
                )
            })
        })
        .collect()
}

/// The pointer layer: the drawing being placed and handles of the selected
/// and hovered drawings.
pub(crate) fn draw_overlay(c: &mut dyn Canvas, s: &Scene<'_>) {
    let ds = s.drawings;
    if s.main_collapsed {
        return;
    }
    if let Some(pl) = &ds.placing {
        draw_one(c, s, &pl.drawing, &pl.screen);
        handles(c, s, pl.drawing.tool, &pl.screen, pl.drawing.color());
    }
    if ds.hidden {
        c.set_clip(s.full);
        return;
    }
    c.set_clip(s.vp_rect());
    for id in [ds.selected, ds.hovered.filter(|h| Some(*h) != ds.selected)]
        .into_iter()
        .flatten()
    {
        if let (Some(d), Some(pts)) = (ds.items.iter().find(|d| d.id == id), ds.screen_of(id)) {
            handles(c, s, d.tool, pts, d.color());
        }
    }
    // Selected lines and rectangles also show their prices on the axis.
    c.set_clip(s.full);
    if let Some(d) = ds.selected_drawing() {
        let pts = ds.screen_of(d.id).unwrap_or(&[]);
        if matches!(
            d.tool,
            DrawingTool::TrendLine
                | DrawingTool::Ray
                | DrawingTool::ExtendedLine
                | DrawingTool::Rectangle
        ) {
            for (a, pt) in d.points.iter().zip(pts) {
                if s.vp_rect().y_range().contains(pt.y) {
                    draw_tag(
                        c,
                        s,
                        &price_axis_tag(
                            s,
                            pt.y,
                            vec![format_price(a.price, s.precision)],
                            d.color(),
                        ),
                    );
                }
            }
        }
    }
}

fn handles(c: &mut dyn Canvas, s: &Scene<'_>, tool: DrawingTool, pts: &[Pos2], color: Color32) {
    // Freehand strokes have no per-point handles.
    if tool.placement() == Placement::Freehand {
        return;
    }
    for p in pts {
        c.fill_circle(*p, HANDLE_RADIUS, color);
        c.fill_circle(*p, HANDLE_RADIUS - 1.5, s.theme.bg);
    }
}

fn draw_one(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    use DrawingTool::*;
    c.set_clip(s.vp_rect());
    if pts.is_empty() {
        return;
    }
    match d.tool {
        TrendLine | Ray | ExtendedLine | HorizontalLine | HorizontalRay | VerticalLine
        | CrossLine => lines::draw(c, s, d, pts),
        Rectangle if pts.len() >= 2 => shapes::rectangle(c, d, pts),
        FibRetracement if pts.len() >= 2 => fib::draw(c, s, d, pts),
        PriceRange | DateRange | DateAndPriceRange if pts.len() >= 2 => measure::draw(c, s, d, pts),
        // Tools not implemented yet (and the drawing being placed with
        // them): the main line through their points.
        _ => {
            let st = d.props.primary_stroke();
            if pts.len() == 1 {
                c.fill_circle(pts[0], st.width as f32 + 1.0, st.color.to_color32());
            } else {
                c.stroke_polyline_styled(pts, st.width as f32, st.color.to_color32(), st.style);
            }
        }
    }
}
