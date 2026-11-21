//! Drawing dispatch: the base layer (drawings), axis tags and the pointer
//! layer (the drawing being placed, handles). Each tool family draws itself
//! in its own file; this only routes.

use super::props::Props;
use super::stats::{Buf, write_full_time};
use super::{Drawing, DrawingTool, HANDLE_RADIUS, channel, fib, hit, lines, measure, shapes};
use crate::draw::{Scene, Tag, draw_tag, price_axis_tag};
use crate::render::Canvas;
use crate::scale::format_price;
use ecolor::Color32;
use emath::{Align2, Pos2, pos2, vec2};

pub(super) use crate::render::styled_line;

/// Draws the drawings into the base layer (clipped to the price pane;
/// vertical lines span all panes).
pub(crate) fn draw_base(c: &mut dyn Canvas, s: &Scene<'_>) {
    let ds = s.drawings;
    if ds.hidden || s.main_collapsed {
        return;
    }
    for d in &ds.items {
        if !ds.is_shown(d) {
            continue;
        }
        if let Some(pts) = ds.screen_of(d.id) {
            draw_one(c, s, d, pts);
        }
    }
    c.set_clip(s.full);
}

/// Whether `d` tags its points' prices on the price axis all the time.
fn tags_prices(d: &Drawing) -> bool {
    match &d.props {
        Props::HLine(p) => p.price_label,
        Props::Cross(p) => p.price_label,
        Props::Line(p) => p.price_labels,
        _ => false,
    }
}

/// Axis tags of horizontal lines / rays, cross lines and lines with "price
/// labels" on (base layer, after the price labels).
pub(crate) fn axis_tags(s: &Scene<'_>) -> Vec<Tag> {
    let ds = s.drawings;
    if ds.hidden || s.main_collapsed {
        return Vec::new();
    }
    let pane = s.vp_rect();
    let mut tags = Vec::new();
    for d in ds.items.iter().filter(|d| ds.is_shown(d) && tags_prices(d)) {
        let n = if matches!(d.props, Props::Line(_)) {
            2
        } else {
            1
        };
        for a in d.points.iter().take(n) {
            let y = s.vp.y_of(a.price);
            if pane.y_range().contains(y) {
                tags.push(price_axis_tag(
                    s,
                    y,
                    vec![format_price(a.price, s.precision)],
                    d.color(),
                ));
            }
        }
    }
    tags
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
        lines::overlay_stats(c, s, &pl.drawing, &pl.screen);
        handles(c, s, pl.drawing.tool, &pl.screen, pl.drawing.color());
    }
    if ds.hidden {
        c.set_clip(s.full);
        return;
    }
    c.set_clip(s.vp_rect());
    let hovered = ds.hovered.filter(|h| !ds.is_selected(*h));
    for &id in ds.selection().iter().chain(hovered.as_ref()) {
        if let (Some(d), Some(pts)) = (ds.items.iter().find(|d| d.id == id), ds.screen_of(id))
            && ds.is_shown(d)
        {
            lines::overlay_stats(c, s, d, pts);
            handles(c, s, d.tool, pts, d.color());
        }
    }
    // The Ctrl + drag rubber band.
    if let Some((a, b)) = ds.band {
        let r = emath::Rect::from_two_pos(a, b);
        c.fill_rect(r, tt_theme::with_alpha(s.theme.accent, 0.12));
        c.stroke_rect(r, 1.0, tt_theme::with_alpha(s.theme.accent, 0.8));
    }
    c.set_clip(s.full);
    // Time-axis tags of vertical and cross lines.
    for d in ds.items.iter().filter(|d| ds.is_shown(d)) {
        let on = match &d.props {
            Props::VLine(p) => p.time_label,
            Props::Cross(p) => p.time_label,
            _ => false,
        };
        if on && let Some(pts) = ds.screen_of(d.id) {
            time_tag(c, s, pts[0].x, d.points[0].time_ms, d.color());
        }
    }
    // A selected line or rectangle (alone) also shows its prices on the axis.
    if let [_] = ds.selection()
        && let Some(d) = ds.selected_drawing()
    {
        let pts = ds.screen_of(d.id).unwrap_or(&[]);
        if lines::is_line_family(d.tool) || d.tool == DrawingTool::Rectangle {
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

/// A tag on the time axis at `x` with the bar's date and time.
fn time_tag(c: &mut dyn Canvas, s: &Scene<'_>, x: f32, time_ms: i64, bg: Color32) {
    if !s.plot.x_range().contains(x) {
        return;
    }
    let mut b = Buf::new();
    let _ = write_full_time(&mut b, time_ms, s.bar_interval_ms);
    let size = s.style.scales.text_size;
    let sz = c.text_size(b.as_str(), size) + vec2(16.0, 8.0);
    let mut r = Align2::CENTER_CENTER.anchor_size(pos2(x, s.time_axis.center().y), sz);
    let dx = (s.plot.left() - r.left()).max(0.0) + (s.plot.right() - r.right()).min(0.0);
    r = r.translate(vec2(dx, 0.0));
    c.fill_rounded_rect(r, tt_theme::metrics::R_XS, bg);
    c.text(
        r.center(),
        Align2::CENTER_CENTER,
        b.as_str(),
        size,
        s.theme.text_on_accent,
    );
}

fn handles(c: &mut dyn Canvas, s: &Scene<'_>, tool: DrawingTool, pts: &[Pos2], color: Color32) {
    hit::for_each_handle(tool, pts, |p| {
        c.fill_circle(p, HANDLE_RADIUS, color);
        c.fill_circle(p, HANDLE_RADIUS - 1.5, s.theme.bg);
    });
}

fn draw_one(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    use DrawingTool::*;
    c.set_clip(s.vp_rect());
    if pts.is_empty() {
        return;
    }
    match d.tool {
        TrendLine | Ray | ExtendedLine | InfoLine | TrendAngle | Arrow | HorizontalLine
        | HorizontalRay | VerticalLine | CrossLine => lines::draw(c, s, d, pts),
        ParallelChannel if pts.len() >= 3 => channel::draw(c, s, d, pts),
        Rectangle if pts.len() >= 2 => shapes::rectangle(c, d, pts, s.vp_rect()),
        FibRetracement if pts.len() >= 2 => fib::draw(c, s, d, pts),
        FibExtension if pts.len() >= 3 => fib::draw(c, s, d, pts),
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
