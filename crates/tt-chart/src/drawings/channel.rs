//! Parallel channel: points 0 and 1 are line 0; point 2 sits on line 1,
//! which is line 0 moved vertically on screen (constant in scale space, so
//! the lines stay parallel on linear and log charts). Levels are at
//! `line 0 + offset × coeff` (0 = line 0, 1 = line 1, 0.5 = the middle).

use super::hit::{dist_to_segment, extend};
use super::lines::text_along;
use super::props::{ChannelProps, Props, VAlign};
use super::render::styled_line;
use super::{Anchor, Drawing, HIT_DISTANCE, Mapper};
use crate::draw::Scene;
use crate::render::Canvas;
use emath::{Pos2, Rect, Vec2, vec2};

/// Handles: line 0's ends, line 1's ends, line 0's middle, line 1's middle.
pub(super) const HANDLES: usize = 6;

/// Screen geometry of a channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Geo {
    pub a: Pos2,
    pub b: Pos2,
    /// Line 1 = line 0 + `(0, offset)`.
    pub offset: f32,
}

impl Geo {
    pub fn of(pts: &[Pos2]) -> Option<Geo> {
        let (a, b, p) = (*pts.first()?, *pts.get(1)?, *pts.get(2)?);
        Some(Geo {
            a,
            b,
            offset: p.y - y_on(a, b, p.x),
        })
    }

    /// Line at level `coeff`.
    pub fn level(&self, coeff: f32) -> (Pos2, Pos2) {
        let d = vec2(0.0, self.offset * coeff);
        (self.a + d, self.b + d)
    }

    pub fn handles(&self) -> [Pos2; HANDLES] {
        let (c, d) = self.level(1.0);
        [
            self.a,
            self.b,
            c,
            d,
            self.a + (self.b - self.a) * 0.5,
            c + (d - c) * 0.5,
        ]
    }
}

/// Y of the line `a → b` at `x` (a vertical line: `a.y`).
pub(super) fn y_on(a: Pos2, b: Pos2, x: f32) -> f32 {
    let dx = b.x - a.x;
    if dx.abs() <= f32::EPSILON {
        a.y
    } else {
        a.y + (b.y - a.y) * (x - a.x) / dx
    }
}

fn props(d: &Drawing) -> Option<&ChannelProps> {
    match &d.props {
        Props::Channel(p) => Some(p),
        _ => None,
    }
}

pub(super) fn draw(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    let (Some(cp), Some(g)) = (props(d), Geo::of(pts)) else {
        return;
    };
    let pane = s.vp_rect();
    let (l, r) = (cp.extend.left, cp.extend.right);
    if cp.background.on && g.offset.abs() > 0.5 {
        // Extended in x only; the clip cuts the rest.
        let tall = Rect::from_x_y_ranges(pane.x_range(), -1e5..=1e5);
        let (a, b) = if l || r {
            extend(g.a, g.b, tall, l, r)
        } else {
            (g.a, g.b)
        };
        let dv = Vec2::new(0.0, g.offset);
        c.fill_polygon(&[a, b, b + dv, a + dv], cp.background.color.to_color32());
    }
    for lv in cp.levels.iter().filter(|lv| lv.on) {
        let st = lv.stroke.unwrap_or(cp.line);
        let (a, b) = g.level(lv.coeff as f32);
        let (a, b) = if l || r {
            extend(a, b, pane, l, r)
        } else {
            (a, b)
        };
        styled_line(c, a, b, st.width as f32, st.color.to_color32(), st.style);
    }
    if cp.text.on && !d.text.is_empty() {
        // TradingView: along the upper line (Top), the middle (Inside) or
        // the lower line (Bottom).
        let upper = if g.offset < 0.0 { 1.0 } else { 0.0 };
        let coeff = match cp.text.v {
            VAlign::Top => upper,
            VAlign::Middle => 0.5,
            VAlign::Bottom => 1.0 - upper,
        };
        let (a, b) = g.level(coeff);
        text_along(c, a, b, &d.text, &cp.text_style, &cp.text);
    }
}

/// Body hit: any shown level line, or inside the band with a background.
pub(super) fn touches(d: &Drawing, pts: &[Pos2], pane: Rect, p: Pos2) -> bool {
    let (Some(cp), Some(g)) = (props(d), Geo::of(pts)) else {
        return false;
    };
    let (l, r) = (cp.extend.left, cp.extend.right);
    let near_level = |coeff: f32| {
        let (a, b) = g.level(coeff);
        let (a, b) = if l || r {
            extend(a, b, pane, l, r)
        } else {
            (a, b)
        };
        dist_to_segment(p, a, b) <= HIT_DISTANCE
    };
    if near_level(0.0)
        || near_level(1.0)
        || cp
            .levels
            .iter()
            .any(|lv| lv.on && near_level(lv.coeff as f32))
    {
        return true;
    }
    if !cp.background.on {
        return false;
    }
    let (x0, x1) = (g.a.x.min(g.b.x), g.a.x.max(g.b.x));
    let in_x = (l || p.x >= x0) && (r || p.x <= x1);
    let y0 = y_on(g.a, g.b, p.x);
    let (lo, hi) = (y0.min(y0 + g.offset), y0.max(y0 + g.offset));
    in_x && p.y >= lo && p.y <= hi
}

/// Drags handle `k` (see [`HANDLES`]) to the anchor `a`.
pub(super) fn drag_handle(d: &mut Drawing, k: usize, a: Anchor, m: &Mapper<'_>) {
    if d.points.len() < 3 {
        return;
    }
    let mut pts = [Pos2::ZERO; 3];
    for (q, p) in pts.iter_mut().zip(&d.points) {
        *q = m.screen(*p);
    }
    let Some(g) = Geo::of(&pts) else {
        return;
    };
    let q = m.screen(a);
    let offset = match k {
        0 | 1 => {
            d.points[k] = a;
            g.offset
        }
        // Line 1's end: the matching end of line 0 follows in time, the
        // offset follows the pointer.
        2 | 3 => {
            let j = k - 2;
            d.points[j].time_ms = a.time_ms;
            q.y - m.screen(d.points[j]).y
        }
        // Line 0's middle: line 0 moves parallel, line 1 stays.
        4 => {
            let dy = q.y - (g.a.y + g.b.y) / 2.0;
            for p in &mut d.points[..2] {
                p.price = m.vp.price_at(m.screen(*p).y + dy);
            }
            g.offset - dy
        }
        // Line 1's middle: only the offset changes.
        5 => q.y - (g.a.y + g.b.y) / 2.0,
        _ => return,
    };
    set_offset(d, offset, m);
}

/// Puts point 2 on line 1 at line 0's start (keeps the offset exact).
fn set_offset(d: &mut Drawing, offset: f32, m: &Mapper<'_>) {
    let start = d.points[0];
    d.points[2] = Anchor {
        time_ms: start.time_ms,
        price: m.vp.price_at(m.screen(start).y + offset),
    };
}
