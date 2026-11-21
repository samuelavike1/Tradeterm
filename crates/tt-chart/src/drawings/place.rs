//! Placing a new drawing: a fixed number of clicks (two-point tools can be
//! dragged out), open-ended (click to add points; double-click, Enter or
//! Escape finishes) and freehand (press, drag, release).
//!
//! Shift constraints (45° lines, squares, circles) go here (slice H).

use super::{
    DRAG_TO_PLACE, Drawing, DrawingPress, DrawingTool, Drawings, Mapper, Mods, Placement, Repaint,
};
use emath::Pos2;
use tt_core::CandleSeries;

/// Freehand strokes keep at most this many samples.
pub(crate) const MAX_FREEHAND_POINTS: usize = 2000;
/// Freehand samples closer than this (points) to the previous are skipped.
const FREEHAND_STEP: f32 = 1.0;

#[derive(Debug, Clone)]
pub(super) struct Placing {
    pub drawing: Drawing,
    /// Points fixed so far. The rest follow the pointer: for fixed-point
    /// tools all remaining points, for open-ended tools one trailing point;
    /// freehand tools have none.
    pub placed: usize,
    /// Where the first press was (drag-to-place).
    pub down: Pos2,
    pub screen: Vec<Pos2>,
}

impl Placing {
    fn placement(&self) -> Placement {
        self.drawing.tool.placement()
    }
}

impl Drawings {
    pub(super) fn place_press(
        &mut self,
        tool: DrawingTool,
        p: Pos2,
        _mods: Mods,
        double: bool,
        m: &Mapper<'_>,
        series: &CandleSeries,
    ) -> DrawingPress {
        let a = m.snapped(p, self.magnet, series);
        let Some(pl) = &mut self.placing else {
            let points = match tool.placement() {
                Placement::Points(n) => vec![a; n.max(1)],
                // The fixed first point and the one following the pointer.
                Placement::OpenEnded => vec![a; 2],
                Placement::Freehand => vec![a],
            };
            let drawing = Drawing::new(tool, points);
            let mut screen = Vec::new();
            m.screen_into(&drawing.points, &mut screen);
            self.placing = Some(Placing {
                drawing,
                placed: 1,
                down: p,
                screen,
            });
            if tool.placement() == Placement::Points(1) {
                self.finish();
                return DrawingPress::Placed;
            }
            return DrawingPress::Handled;
        };
        match pl.placement() {
            Placement::Points(n) => {
                pl.drawing.points[pl.placed] = a;
                pl.placed += 1;
                if pl.placed >= n {
                    self.finish();
                    return DrawingPress::Placed;
                }
            }
            Placement::OpenEnded => {
                // The first press of the double-click added the last point.
                if double {
                    return if self.finish_placing() {
                        DrawingPress::Placed
                    } else {
                        DrawingPress::Handled
                    };
                }
                pl.drawing.points[pl.placed] = a;
                pl.placed += 1;
                pl.drawing.points.push(a);
                m.screen_into(&pl.drawing.points, &mut pl.screen);
            }
            // Placing ends on release; a stray press changes nothing.
            Placement::Freehand => {}
        }
        DrawingPress::Handled
    }

    pub(super) fn place_motion(
        &mut self,
        p: Pos2,
        _mods: Mods,
        m: &Mapper<'_>,
        series: &CandleSeries,
    ) -> Repaint {
        let magnet = self.magnet;
        let Some(pl) = &mut self.placing else {
            return Repaint::None;
        };
        match pl.placement() {
            Placement::Points(_) | Placement::OpenEnded => {
                let a = m.snapped(p, magnet, series);
                for slot in &mut pl.drawing.points[pl.placed..] {
                    *slot = a;
                }
                m.screen_into(&pl.drawing.points, &mut pl.screen);
            }
            Placement::Freehand => {
                let far = pl
                    .screen
                    .last()
                    .is_none_or(|q| q.distance(p) >= FREEHAND_STEP);
                if !far || pl.drawing.points.len() >= MAX_FREEHAND_POINTS {
                    return Repaint::None;
                }
                pl.drawing.points.push(m.anchor(p));
                pl.screen.push(p);
                pl.placed = pl.drawing.points.len();
            }
        }
        Repaint::Overlay
    }

    /// Returns whether a drawing was committed.
    pub(super) fn place_release(&mut self, p: Pos2, _mods: Mods) -> bool {
        let Some(pl) = &mut self.placing else {
            return false;
        };
        match pl.placement() {
            Placement::Points(n) => {
                if pl.placed == 1 && n >= 2 && pl.down.distance(p) > DRAG_TO_PLACE {
                    // Dragged out: the release places the second point.
                    pl.placed = 2;
                    if pl.placed >= n {
                        self.finish();
                        return true;
                    }
                }
                false
            }
            Placement::OpenEnded => false,
            Placement::Freehand => {
                if pl.drawing.points.len() >= 2 {
                    self.finish();
                    true
                } else {
                    self.placing = None;
                    false
                }
            }
        }
    }

    /// Finishes an open-ended drawing (double-click, Enter, Escape) with the
    /// points fixed so far; fewer than two drops it. Returns whether a
    /// drawing was committed.
    pub fn finish_placing(&mut self) -> bool {
        let Some(pl) = &mut self.placing else {
            return false;
        };
        if pl.placement() != Placement::OpenEnded {
            return false;
        }
        if pl.placed < 2 {
            self.placing = None;
            return false;
        }
        let n = pl.placed;
        pl.drawing.points.truncate(n);
        pl.screen.truncate(n);
        self.finish();
        true
    }
}
