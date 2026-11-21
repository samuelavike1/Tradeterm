//! Fib retracement (and later the trend-based Fib extension).
//!
//! Reads the visible levels and the levels line from `FibProps`; the band
//! opacity (0.12), labels ("level (price)" on the left), log-space levels
//! and the trend line look are still fixed as before props (slice C).

use super::Drawing;
use super::props::Props;
use super::render::styled_line;
use crate::draw::Scene;
use crate::render::Canvas;
use crate::scale::format_price;
use emath::{Align2, Pos2, Rect, pos2};
use tt_theme::with_alpha;

/// Level `l` sits at `end + (start - end) * l`, so 0 is at the second point
/// and 1 at the first (TradingView's convention).
pub(super) fn draw(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    let Props::Fib(fp) = &d.props else {
        return;
    };
    let (p0, p1) = (d.points[0].price, d.points[1].price);
    let (x0, x1) = (pts[0].x.min(pts[1].x), pts[0].x.max(pts[1].x));
    let sc = s.vp.scale;
    let level_price =
        |l: f64| sc.from_scaled(sc.to_scaled(p1) + (sc.to_scaled(p0) - sc.to_scaled(p1)) * l);
    let (w, style) = (fp.levels_line.width as f32, fp.levels_line.style);
    let levels = fp.levels.iter().filter(|l| l.on);
    // Bands between consecutive levels.
    let mut prev: Option<f32> = None;
    for l in levels.clone() {
        let y = s.vp.y_of(level_price(l.coeff));
        if let Some(py) = prev {
            let r = Rect::from_x_y_ranges(x0..=x1, py.min(y)..=py.max(y));
            c.fill_rect(r, with_alpha(l.color.to_color32(), 0.12));
        }
        prev = Some(y);
    }
    for l in levels {
        let price = level_price(l.coeff);
        let y = s.vp.y_of(price);
        let col = l.color.to_color32();
        styled_line(c, pos2(x0, y), pos2(x1, y), w, col, style);
        let label = format!(
            "{} ({})",
            trim_level(l.coeff),
            format_price(price, s.precision)
        );
        c.text(
            pos2(x0 - 4.0, y),
            Align2::RIGHT_CENTER,
            &label,
            s.style.font_size - 1.0,
            col,
        );
    }
    // The trend line between the two points.
    c.dashed_line(
        pts[0],
        pts[1],
        1.0,
        with_alpha(fp.trend_line.color.to_color32(), 0.8),
        4.0,
        4.0,
    );
}

fn trim_level(l: f64) -> String {
    let s = format!("{l:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}
