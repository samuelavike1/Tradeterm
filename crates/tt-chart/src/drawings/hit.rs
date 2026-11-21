//! Hit testing: which drawing (and which part) is under the pointer.

use super::measure::measurer_rect;
use super::props::Props;
use super::{
    Anchor, Drawing, DrawingPart, DrawingTool, Drawings, HANDLE_RADIUS, HIT_DISTANCE, Mapper,
    Placement, channel, fib, lines,
};
use emath::{Pos2, Rect};

impl Drawings {
    /// The drawing (and part) under `p`, topmost first.
    pub fn hit(&self, p: Pos2) -> Option<(u64, DrawingPart)> {
        if self.hidden || !self.pane.contains(p) {
            return None;
        }
        // Handles of the selected drawing win.
        if let Some(id) = self.selected
            && let Some(d) = self.items.iter().find(|d| d.id == id)
            && let Some(pts) = self.screen_of(id)
            && let Some(k) = handle_at(d.tool, pts, p, HANDLE_RADIUS + 2.0)
        {
            return Some((id, DrawingPart::Handle(k)));
        }
        for d in self.items.iter().rev() {
            if d.hidden {
                continue;
            }
            let Some(pts) = self.screen_of(d.id) else {
                continue;
            };
            if let Some(k) = handle_at(d.tool, pts, p, HANDLE_RADIUS + 1.0) {
                return Some((d.id, DrawingPart::Handle(k)));
            }
            if touches(d, pts, self.pane, self.plot, p) {
                return Some((d.id, DrawingPart::Body));
            }
        }
        None
    }
}

/// Calls `f` with each handle of a drawing whose screen points are `pts`
/// (handle `k` is `DrawingPart::Handle(k)`). Most tools have one handle per
/// point; the parallel channel has six; freehand strokes have none.
pub(super) fn for_each_handle(tool: DrawingTool, pts: &[Pos2], mut f: impl FnMut(Pos2)) {
    if tool.placement() == Placement::Freehand {
        return;
    }
    if tool == DrawingTool::ParallelChannel
        && let Some(g) = channel::Geo::of(pts)
    {
        for h in g.handles() {
            f(h);
        }
        return;
    }
    for q in pts {
        f(*q);
    }
}

/// The handle within `r` of `p`, if any.
fn handle_at(tool: DrawingTool, pts: &[Pos2], p: Pos2, r: f32) -> Option<usize> {
    let (mut k, mut found) = (0, None);
    for_each_handle(tool, pts, |q| {
        if found.is_none() && q.distance(p) <= r {
            found = Some(k);
        }
        k += 1;
    });
    found
}

/// Moves handle `k` of `d` to the (snapped) anchor `a`.
pub(super) fn drag_handle(d: &mut Drawing, k: usize, a: Anchor, m: &Mapper<'_>) {
    if d.tool == DrawingTool::ParallelChannel {
        channel::drag_handle(d, k, a, m);
    } else if let Some(pt) = d.points.get_mut(k) {
        *pt = a;
    }
}

/// Whether `p` touches the drawing `d` whose screen points are `pts`.
fn touches(d: &Drawing, pts: &[Pos2], pane: Rect, plot: Rect, p: Pos2) -> bool {
    use DrawingTool::*;
    let near = |a: Pos2, b: Pos2| dist_to_segment(p, a, b) <= HIT_DISTANCE;
    if pts.is_empty() {
        return false;
    }
    match (d.tool, &d.props) {
        (HorizontalLine, _) => (p.y - pts[0].y).abs() <= HIT_DISTANCE,
        (HorizontalRay, _) => {
            (p.y - pts[0].y).abs() <= HIT_DISTANCE && p.x >= pts[0].x - HIT_DISTANCE
        }
        (VerticalLine, props) => {
            let span = match props {
                Props::VLine(v) if !v.extend => pane,
                _ => plot,
            };
            (p.x - pts[0].x).abs() <= HIT_DISTANCE && span.y_range().contains(p.y)
        }
        (CrossLine, _) => {
            (p.y - pts[0].y).abs() <= HIT_DISTANCE || (p.x - pts[0].x).abs() <= HIT_DISTANCE
        }
        (_, Props::Line(lp)) if pts.len() >= 2 => {
            let (a, b) = lines::segment(lp, pts[0], pts[1], pane);
            near(a, b)
        }
        (ParallelChannel, _) if pts.len() >= 3 => channel::touches(d, pts, pane, p),
        (FibRetracement | FibExtension, _) => fib::touches(d, pts, pane, p, HIT_DISTANCE),
        (Rectangle, _) if pts.len() >= 2 => Rect::from_two_pos(pts[0], pts[1])
            .expand(HIT_DISTANCE)
            .contains(p),
        (PriceRange | DateRange | DateAndPriceRange, _) if pts.len() >= 2 => {
            measurer_rect(d, pts, pane).expand(HIT_DISTANCE).contains(p)
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
