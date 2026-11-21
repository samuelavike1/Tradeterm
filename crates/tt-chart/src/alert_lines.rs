//! Alert levels on the price pane, like TradingView: a dashed line across
//! the plot in the alert color (Settings › Alerts › Alert lines), a solid
//! price tag on the price axis and a small alarm-clock tag on the plot side
//! of the axis (click: edit, drag: move the level). Also the price scale's
//! "+" button, which follows the crosshair (click: add an alert there).
//!
//! The app sets [`crate::ChartView::alert_lines`]; the view draws them in the
//! base layer and hit-tests them on the last frame's layout.

use crate::draw::{Scene, contrast_text, crisp, draw_price_tag};
use crate::render::Canvas;
use crate::scale::format_price;
use crate::style::resolve;
use ecolor::Color32;
use emath::{Pos2, Rect, pos2, vec2};

/// One alert level on the chart.
#[derive(Debug, Clone, PartialEq)]
pub struct AlertLine {
    /// The alert's id (events carry it back).
    pub id: u64,
    pub price: f64,
    /// Stopped / triggered alerts are drawn faded (and hidden with
    /// "Only active alerts").
    pub active: bool,
}

/// Side of the alarm-clock tag and the "+" button.
pub const TAG: f32 = 18.0;
/// Vertical grab distance of a line.
const GRAB: f32 = 4.0;

/// The alarm-clock tag of a line at `y`: on the plot, against the axis.
pub(crate) fn tag_rect(plot_right: f32, y: f32) -> Rect {
    Rect::from_min_size(pos2(plot_right - TAG - 2.0, y - TAG / 2.0), vec2(TAG, TAG))
}

/// The "+" button at the crosshair's `y` (same place as the tag).
pub(crate) fn plus_rect(plot_right: f32, y: f32) -> Rect {
    tag_rect(plot_right, y)
}

/// Settings › Alerts › Alert lines color, else TradingView's blue.
fn line_color(s: &Scene<'_>) -> Color32 {
    resolve(s.style.alerts.line_color, s.theme.active)
}

/// Lines shown with the current settings.
fn shown<'a>(s: &Scene<'_>, lines: &'a [AlertLine]) -> impl Iterator<Item = &'a AlertLine> {
    let (on, only_active) = (s.style.alerts.lines, s.style.alerts.only_active);
    lines
        .iter()
        .filter(move |l| on && (l.active || !only_active))
}

/// Draws the lines (base layer): line in the plot, tag on the axis, clock.
pub(crate) fn draw(c: &mut dyn Canvas, s: &Scene<'_>, lines: &[AlertLine]) {
    if s.main_collapsed || lines.is_empty() {
        return;
    }
    let ppp = c.pixels_per_point();
    let color = line_color(s);
    let (top, bottom) = (s.plot.top(), s.vp.plot.bottom);
    for l in shown(s, lines) {
        let y = crisp(s.vp.y_of(l.price), ppp);
        if !(top..=bottom).contains(&y) {
            continue;
        }
        let col = if l.active {
            color
        } else {
            color.gamma_multiply(0.45)
        };
        c.set_clip(s.vp_rect());
        let w = 1.0;
        c.dashed_line(
            pos2(s.plot.left(), y),
            pos2(s.vp.plot.right, y),
            w,
            col,
            4.0,
            4.0,
        );
        // Alarm clock on the plot side of the axis.
        let r = tag_rect(s.vp.plot.right, y);
        c.fill_rounded_rect(r, tt_theme::metrics::R_XS, s.theme.bg);
        c.stroke_rounded_rect(r, tt_theme::metrics::R_XS, 1.0, col);
        c.icon(
            r.center(),
            char::from(lucide_icons::Icon::AlarmClock),
            12.0,
            col,
        );
        c.set_clip(s.full);
        let text = format_price(l.price, s.precision);
        draw_price_tag(c, s, y, &text, col, contrast_text(col));
    }
}

/// The "+" at the crosshair (overlay layer), when Settings › Scales › Plus
/// button is on, except over an alert's tag (a click there edits it).
pub(crate) fn draw_plus(c: &mut dyn Canvas, s: &Scene<'_>, lines: &[AlertLine]) {
    let Some(p) = s.crosshair else { return };
    if !s.style.scales.plus_button || s.main_collapsed || p.y > s.vp.plot.bottom {
        return;
    }
    if shown(s, lines).any(|l| (s.vp.y_of(l.price) - p.y).abs() <= GRAB) {
        return;
    }
    let ppp = c.pixels_per_point();
    let r = plus_rect(s.vp.plot.right, crisp(p.y, ppp));
    c.set_clip(s.full);
    c.fill_rounded_rect(r, tt_theme::metrics::R_XS, s.theme.crosshair_label_bg);
    let fg = contrast_text(s.theme.crosshair_label_bg);
    let (cx, cy, h) = (r.center().x, r.center().y, 4.5);
    c.line(pos2(cx - h, cy), pos2(cx + h, cy), 1.0, fg);
    c.line(pos2(cx, cy - h), pos2(cx, cy + h), 1.0, fg);
}

/// The line under `p`: its clock tag, its axis tag or the line itself.
pub(crate) fn hit(
    lines: &[AlertLine],
    p: Pos2,
    plot: Rect,
    price_axis: Rect,
    y_of: impl Fn(f64) -> f32,
) -> Option<(u64, f64)> {
    lines.iter().rev().find_map(|l| {
        let y = y_of(l.price);
        if !(plot.top()..=plot.bottom()).contains(&y) {
            return None;
        }
        let on_line = (p.y - y).abs() <= GRAB
            && (plot.left()..=price_axis.right().max(plot.right())).contains(&p.x);
        (on_line || tag_rect(plot.right(), y).expand(1.0).contains(p)).then_some((l.id, l.price))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_tests_lines_tags_and_misses() {
        let plot = Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 500.0));
        let axis = Rect::from_min_max(pos2(800.0, 0.0), pos2(880.0, 500.0));
        let lines = [
            AlertLine {
                id: 1,
                price: 100.0,
                active: true,
            },
            AlertLine {
                id: 2,
                price: 50.0,
                active: false,
            },
        ];
        // y = 500 - price * 4: 100 → 100, 50 → 300.
        let y_of = |p: f64| 500.0 - p as f32 * 4.0;
        assert_eq!(
            hit(&lines, pos2(10.0, 102.0), plot, axis, y_of),
            Some((1, 100.0))
        );
        assert_eq!(
            hit(&lines, pos2(850.0, 299.0), plot, axis, y_of),
            Some((2, 50.0))
        );
        assert_eq!(
            hit(&lines, pos2(790.0, 108.0), plot, axis, y_of),
            Some((1, 100.0))
        );
        assert_eq!(hit(&lines, pos2(10.0, 200.0), plot, axis, y_of), None);
        // Off-screen levels are not hit.
        let far = [AlertLine {
            id: 3,
            price: 1000.0,
            active: true,
        }];
        assert_eq!(hit(&far, pos2(10.0, -3500.0), plot, axis, y_of), None);
    }
}
