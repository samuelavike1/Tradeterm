//! The selection set (click, Ctrl + click, rubber band), clipboard paste and
//! clone, visual order and arrow-key nudges.

use super::history::{Change, Edit, EditKind};
use super::{Anchor, Drawing, DrawingTool, Drawings, Mapper, VisualOrder, model};
use emath::{Pos2, Rect};

/// A paste onto drawings at the same anchors moves this far (points) right
/// and down per step, so the copy is visible.
const PASTE_OFFSET: f32 = 20.0;

impl Drawings {
    /// Selected drawing ids (the first is the primary one).
    pub fn selection(&self) -> &[u64] {
        &self.selection
    }

    pub fn is_selected(&self, id: u64) -> bool {
        self.selection.contains(&id)
    }

    /// The primary selected drawing (the floating toolbar shows its style).
    pub fn selected_drawing(&self) -> Option<&Drawing> {
        self.selection
            .iter()
            .find_map(|id| self.items.iter().find(|d| d.id == *id))
    }

    /// The selected drawings, in visual order.
    pub fn selected_drawings(&self) -> impl Iterator<Item = &Drawing> {
        self.items.iter().filter(|d| self.selection.contains(&d.id))
    }

    /// Selects only `id`.
    pub fn select_only(&mut self, id: u64) {
        self.selection.clear();
        self.selection.push(id);
    }

    /// Ctrl + click: adds `id` to the selection or removes it.
    pub fn toggle_selected(&mut self, id: u64) {
        match self.selection.iter().position(|&s| s == id) {
            Some(k) => {
                self.selection.remove(k);
            }
            None => self.selection.push(id),
        }
    }

    /// Returns whether anything was selected.
    pub fn clear_selection(&mut self) -> bool {
        let any = !self.selection.is_empty();
        self.selection.clear();
        any
    }

    /// Rubber band: selects the visible drawings `r` intersects.
    pub(super) fn select_in(&mut self, r: Rect) {
        self.selection.clear();
        for d in &self.items {
            if !self.is_shown(d) {
                continue;
            }
            if let Some((_, pts)) = self.screen.iter().find(|(i, _)| *i == d.id)
                && intersects(d.tool, pts, r, self.plot)
            {
                self.selection.push(d.id);
            }
        }
    }

    /// Copies of the selected drawings, in visual order (the clipboard).
    pub fn copy_selected(&self) -> Vec<Drawing> {
        self.selected_drawings().cloned().collect()
    }

    /// Adds copies of `src` (new ids, shown, unlocked) and selects them, as
    /// one undo step. They keep their anchors unless that puts a copy
    /// exactly on an existing drawing; then all move right and down by
    /// [`PASTE_OFFSET`] until none does.
    pub(crate) fn paste(&mut self, src: &[Drawing], m: &Mapper<'_>) -> bool {
        if src.is_empty() || self.busy() {
            return false;
        }
        let mut copies: Vec<Drawing> = src
            .iter()
            .map(|d| Drawing {
                id: model::next_id(),
                hidden: false,
                locked: false,
                ..d.clone()
            })
            .collect();
        let bars = (PASTE_OFFSET / m.vp.bar_spacing.max(0.1)).round().max(1.0) as f64;
        let (from, to) = (m.vp.price_at(0.0), m.vp.price_at(PASTE_OFFSET));
        for _ in 0..50 {
            let collides = copies.iter().any(|c| {
                self.items
                    .iter()
                    .any(|d| d.tool == c.tool && d.points == c.points)
            });
            if !collides {
                break;
            }
            for c in &mut copies {
                for a in &mut c.points {
                    *a = m.shifted(*a, bars, from, to);
                }
            }
        }
        self.selection.clear();
        let mut changes = Vec::with_capacity(copies.len());
        for c in copies {
            self.selection.push(c.id);
            self.screen
                .push((c.id, c.points.iter().map(|a| m.screen(*a)).collect()));
            changes.push(Change {
                id: c.id,
                before: None,
                after: Some((self.items.len(), c.clone())),
            });
            self.items.push(c);
        }
        self.record(Edit::new(changes));
        true
    }

    /// Clone (context menu): pastes copies of the selection.
    pub(crate) fn clone_selected(&mut self, m: &Mapper<'_>) -> bool {
        let src = self.copy_selected();
        self.paste(&src, m)
    }

    /// Edits one drawing (object tree: lock, hide, rename) as one undo
    /// step. Returns whether it changed.
    pub fn edit_drawing(&mut self, id: u64, f: impl FnOnce(&mut Drawing)) -> bool {
        let Some(i) = self.items.iter().position(|d| d.id == id) else {
            return false;
        };
        let mut d = self.items[i].clone();
        f(&mut d);
        if d == self.items[i] {
            return false;
        }
        if d.hidden || d.locked {
            self.selection.retain(|s| *s != id);
        }
        self.replace_drawing(d)
    }

    /// Removes one drawing (object tree trash), undoable.
    pub fn remove_drawing(&mut self, id: u64) -> bool {
        self.selection.retain(|s| *s != id);
        self.remove_where(|d| d.id == id)
    }

    /// Moves drawing `id` to `index` in the visual order (object tree
    /// drag), as one undo step.
    pub fn move_drawing(&mut self, id: u64, index: usize) -> bool {
        if self.busy() {
            return false;
        }
        let Some(from) = self.items.iter().position(|d| d.id == id) else {
            return false;
        };
        let to = index.min(self.items.len() - 1);
        if to == from {
            return false;
        }
        let before: Vec<u64> = self.items.iter().map(|d| d.id).collect();
        let d = self.items.remove(from);
        self.items.insert(to, d);
        let after: Vec<u64> = self.items.iter().map(|d| d.id).collect();
        self.record(Edit {
            kind: EditKind::Other,
            changes: Vec::new(),
            order: Some((before, after)),
        });
        true
    }

    /// Changes the selection's place in the visual order (the selected
    /// drawings keep their relative order).
    pub fn reorder_selected(&mut self, op: VisualOrder) -> bool {
        if self.selection.is_empty() || self.busy() {
            return false;
        }
        let before: Vec<u64> = self.items.iter().map(|d| d.id).collect();
        let sel = &self.selection;
        let is_sel = |d: &Drawing| sel.contains(&d.id);
        let n = self.items.len();
        match op {
            // Stable sorts: unselected stay in order, as do selected.
            VisualOrder::BringToFront => self.items.sort_by_key(|d| is_sel(d)),
            VisualOrder::SendToBack => self.items.sort_by_key(|d| !is_sel(d)),
            VisualOrder::BringForward => {
                for i in (0..n.saturating_sub(1)).rev() {
                    if is_sel(&self.items[i]) && !is_sel(&self.items[i + 1]) {
                        self.items.swap(i, i + 1);
                    }
                }
            }
            VisualOrder::SendBackward => {
                for i in 1..n {
                    if is_sel(&self.items[i]) && !is_sel(&self.items[i - 1]) {
                        self.items.swap(i, i - 1);
                    }
                }
            }
        }
        let after: Vec<u64> = self.items.iter().map(|d| d.id).collect();
        if after == before {
            return false;
        }
        self.record(Edit {
            kind: EditKind::Other,
            changes: Vec::new(),
            order: Some((before, after)),
        });
        true
    }

    /// Arrow keys: moves the unlocked selected drawings by `bars` bar slots
    /// and `dprice` (price units). Repeated nudges are one undo step.
    pub(crate) fn nudge(&mut self, bars: i32, dprice: f64, m: &Mapper<'_>) -> bool {
        if self.selection.is_empty() || self.locked || self.busy() {
            return false;
        }
        let mut changes = Vec::new();
        for (i, d) in self.items.iter_mut().enumerate() {
            if d.locked || !self.selection.contains(&d.id) {
                continue;
            }
            let before = d.clone();
            for a in &mut d.points {
                *a = Anchor {
                    time_ms: m.shift_time(*a, bars as f64),
                    price: a.price + dprice,
                };
            }
            if let Some((_, s)) = self.screen.iter_mut().find(|(id, _)| *id == d.id) {
                m.screen_into(&d.points, s);
            }
            changes.push(Change {
                id: d.id,
                before: Some((i, before)),
                after: Some((i, d.clone())),
            });
        }
        if changes.is_empty() {
            return false;
        }
        self.record(Edit {
            kind: EditKind::Nudge,
            changes,
            order: None,
        });
        true
    }
}

/// Whether a drawing with screen points `pts` meets the rubber band `r`.
pub(super) fn intersects(tool: DrawingTool, pts: &[Pos2], r: Rect, plot: Rect) -> bool {
    use DrawingTool::*;
    let Some(&p0) = pts.first() else {
        return false;
    };
    let ys = r.y_range();
    let xs = r.x_range();
    match tool {
        HorizontalLine => ys.contains(p0.y),
        HorizontalRay => ys.contains(p0.y) && r.right() >= p0.x,
        VerticalLine => xs.contains(p0.x) && plot.y_range().intersects(ys),
        CrossLine => ys.contains(p0.y) || xs.contains(p0.x),
        Rectangle | Ellipse | FibRetracement | FibExtension | PriceRange | DateRange
        | DateAndPriceRange | LongPosition | ShortPosition => {
            let mut b = Rect::from_min_max(p0, p0);
            for p in pts {
                b.extend_with(*p);
            }
            b.intersects(r)
        }
        _ => {
            pts.iter().any(|p| r.contains(*p))
                || pts.windows(2).any(|w| segment_meets_rect(w[0], w[1], r))
        }
    }
}

/// Whether the segment `a`-`b` crosses rectangle `r`.
fn segment_meets_rect(a: Pos2, b: Pos2, r: Rect) -> bool {
    if r.contains(a) || r.contains(b) {
        return true;
    }
    let c = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    ];
    // A segment crossing the rectangle crosses one of its edges.
    (0..4).any(|k| segments_cross(a, b, c[k], c[(k + 1) % 4]))
}

fn segments_cross(p: Pos2, q: Pos2, a: Pos2, b: Pos2) -> bool {
    let cross = |o: Pos2, u: Pos2, v: Pos2| (u.x - o.x) * (v.y - o.y) - (u.y - o.y) * (v.x - o.x);
    let d1 = cross(a, b, p);
    let d2 = cross(a, b, q);
    let d3 = cross(p, q, a);
    let d4 = cross(p, q, b);
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}
