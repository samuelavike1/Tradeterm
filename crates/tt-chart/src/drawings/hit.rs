//! Hit testing: which drawing (and which part) is under the pointer.

use super::measure::measurer_rect;
use super::{DrawingPart, DrawingTool, Drawings, HANDLE_RADIUS, HIT_DISTANCE, Placement};
use emath::{Pos2, Rect};

impl Drawings {
    /// The drawing (and part) under `p`, topmost first.
    pub fn hit(&self, p: Pos2) -> Option<(u64, DrawingPart)> {
        if self.hidden || !self.pane.contains(p) {
            return None;
        }
        // Handles of the selected drawings win.
        for &id in &self.selection {
            if self.items.iter().any(|d| d.id == id && has_handles(d.tool))
                && let Some(pts) = self.screen_of(id)
                && let Some(k) = pts
                    .iter()
                    .position(|q| q.distance(p) <= HANDLE_RADIUS + 2.0)
            {
                return Some((id, DrawingPart::Handle(k)));
            }
        }
        for d in self.items.iter().rev() {
            if d.hidden {
                continue;
            }
            let Some(pts) = self.screen_of(d.id) else {
                continue;
            };
            if has_handles(d.tool)
                && let Some(k) = pts
                    .iter()
                    .position(|q| q.distance(p) <= HANDLE_RADIUS + 1.0)
            {
                return Some((d.id, DrawingPart::Handle(k)));
            }
            if touches(d.tool, pts, self.pane, self.plot, p) {
                return Some((d.id, DrawingPart::Body));
            }
        }
        None
    }
}

/// Freehand strokes move as a whole (no per-point handles).
fn has_handles(tool: DrawingTool) -> bool {
    tool.placement() != Placement::Freehand
}

/// Whether `p` touches a drawing whose screen points are `pts`.
fn touches(tool: DrawingTool, pts: &[Pos2], pane: Rect, plot: Rect, p: Pos2) -> bool {
    use DrawingTool::*;
    let near = |a: Pos2, b: Pos2| dist_to_segment(p, a, b) <= HIT_DISTANCE;
    if pts.is_empty() {
        return false;
    }
    match tool {
        HorizontalLine => (p.y - pts[0].y).abs() <= HIT_DISTANCE,
        HorizontalRay => (p.y - pts[0].y).abs() <= HIT_DISTANCE && p.x >= pts[0].x - HIT_DISTANCE,
        VerticalLine => (p.x - pts[0].x).abs() <= HIT_DISTANCE && plot.y_range().contains(p.y),
        CrossLine => {
            (p.y - pts[0].y).abs() <= HIT_DISTANCE || (p.x - pts[0].x).abs() <= HIT_DISTANCE
        }
        TrendLine if pts.len() >= 2 => near(pts[0], pts[1]),
        Ray if pts.len() >= 2 => {
            let (a, b) = extend(pts[0], pts[1], pane, false, true);
            near(a, b)
        }
        ExtendedLine if pts.len() >= 2 => {
            let (a, b) = extend(pts[0], pts[1], pane, true, true);
            near(a, b)
        }
        FibRetracement | Rectangle if pts.len() >= 2 => Rect::from_two_pos(pts[0], pts[1])
            .expand(HIT_DISTANCE)
            .contains(p),
        PriceRange | DateRange | DateAndPriceRange if pts.len() >= 2 => {
            measurer_rect(tool, pts, pane)
                .expand(HIT_DISTANCE)
                .contains(p)
        }
        // Tools without their own hit test yet: the polyline through their
        // points (or the point itself).
        _ => match pts {
            [a] => a.distance(p) <= HIT_DISTANCE,
            _ => pts.windows(2).any(|w| near(w[0], w[1])),
        },
    }
}

pub(super) fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 <= f32::EPSILON {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// The line through `a` and `b`, extended backwards and/or forwards to the
/// edges of `r`.
pub(super) fn extend(a: Pos2, b: Pos2, r: Rect, back: bool, fwd: bool) -> (Pos2, Pos2) {
    let d = b - a;
    if d.length_sq() <= f32::EPSILON {
        return (a, b);
    }
    // Parameter range of the line inside `r` (slab clipping).
    let (mut t0, mut t1) = (f32::NEG_INFINITY, f32::INFINITY);
    for (o, dd, lo, hi) in [
        (a.x, d.x, r.left(), r.right()),
        (a.y, d.y, r.top(), r.bottom()),
    ] {
        if dd.abs() > f32::EPSILON {
            let (u, v) = ((lo - o) / dd, (hi - o) / dd);
            t0 = t0.max(u.min(v));
            t1 = t1.min(u.max(v));
        }
    }
    if !t0.is_finite() || !t1.is_finite() {
        return (a, b);
    }
    let start = if back { a + d * t0.min(0.0) } else { a };
    let end = if fwd { a + d * t1.max(1.0) } else { b };
    (start, end)
}
