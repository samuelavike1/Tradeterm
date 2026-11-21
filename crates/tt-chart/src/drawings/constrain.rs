//! Shift constraints (spec 4.3), in screen space: 45° steps for lines and
//! the channel base, squares for rectangles and ellipses, and an axis lock
//! for body drags.

use super::DrawingTool;
use emath::{Pos2, Vec2, pos2, vec2};
use std::f32::consts::FRAC_PI_4;

/// Pointer travel (points) that picks the axis of a Shift body drag.
pub(crate) const AXIS_PICK_DISTANCE: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Constraint {
    /// Angle in 45° steps (horizontal, vertical, diagonals).
    Angle,
    /// Equal width and height.
    Square,
}

/// The constraint Shift applies to `tool`'s point `k`, and the point it is
/// relative to.
pub(crate) fn constraint(tool: DrawingTool, k: usize, n: usize) -> Option<(Constraint, usize)> {
    use DrawingTool::*;
    let c = match tool {
        TrendLine | Ray | InfoLine | ExtendedLine | TrendAngle | Arrow | FibRetracement
        | FibExtension | ParallelChannel | Path | Polyline => Constraint::Angle,
        Rectangle | Ellipse | Circle | PriceRange | DateRange | DateAndPriceRange => {
            Constraint::Square
        }
        _ => return None,
    };
    let reference = match tool {
        // Each vertex relative to the previous one (the first to the next).
        Path | Polyline | FibExtension => {
            if k == 0 {
                1
            } else {
                k - 1
            }
        }
        // Only the base line (points 0 and 1); the width point is free.
        ParallelChannel => match k {
            0 => 1,
            1 => 0,
            _ => return None,
        },
        _ => match (n, k) {
            (2, 0) => 1,
            (2, 1) => 0,
            _ => return None,
        },
    };
    (reference < n && reference != k).then_some((c, reference))
}

/// `p` constrained relative to `from`.
pub(crate) fn apply(c: Constraint, from: Pos2, p: Pos2) -> Pos2 {
    let d = p - from;
    match c {
        Constraint::Angle => {
            if d.length_sq() == 0.0 {
                return p;
            }
            let a = (d.y.atan2(d.x) / FRAC_PI_4).round() * FRAC_PI_4;
            let u = vec2(a.cos(), a.sin());
            // Project onto the snapped direction (the pointer stays close).
            let along = d.dot(u);
            let mut q = from + u * along;
            // Exact axes (no float drift into a 1e-7 slope).
            if u.y.abs() < 1e-4 {
                q.y = from.y;
            }
            if u.x.abs() < 1e-4 {
                q.x = from.x;
            }
            q
        }
        Constraint::Square => {
            let s = d.x.abs().max(d.y.abs());
            let sign = |v: f32| if v < 0.0 { -1.0 } else { 1.0 };
            pos2(from.x + sign(d.x) * s, from.y + sign(d.y) * s)
        }
    }
}

/// Axis of a Shift body drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Axis {
    /// Time changes, price stays.
    Horizontal,
    Vertical,
}

/// The axis the first few points of motion pick (`None` until then).
pub(crate) fn pick_axis(d: Vec2) -> Option<Axis> {
    if d.length() < AXIS_PICK_DISTANCE {
        return None;
    }
    Some(if d.x.abs() >= d.y.abs() {
        Axis::Horizontal
    } else {
        Axis::Vertical
    })
}
