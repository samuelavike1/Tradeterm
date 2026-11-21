//! Undo / redo of drawing changes (spec 6.5): a bounded stack of diffs per
//! chart. Each [`Edit`] holds the drawings it touched before and after the
//! gesture (with their index in the list, so a restored drawing goes back
//! to its place in the visual order), plus the order itself for visual
//! order operations. Steps are recorded at gesture end, never per frame.

use super::Drawing;
use std::collections::VecDeque;

/// Undo steps kept per chart.
pub(crate) const MAX_HISTORY: usize = 100;

/// One drawing's state before and after an edit (`None`: not present, so
/// the edit created or deleted it), with its index in the list.
#[derive(Debug, Clone)]
pub(crate) struct Change {
    pub id: u64,
    pub before: Option<(usize, Drawing)>,
    pub after: Option<(usize, Drawing)>,
}

/// Consecutive edits of the same kind and drawings merge into one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditKind {
    Other,
    /// Arrow-key nudges (holding an arrow key is one undo step).
    Nudge,
}

#[derive(Debug, Clone)]
pub(crate) struct Edit {
    pub kind: EditKind,
    pub changes: Vec<Change>,
    /// Drawing ids in visual order before and after (order operations).
    pub order: Option<(Vec<u64>, Vec<u64>)>,
}

impl Edit {
    pub fn new(changes: Vec<Change>) -> Self {
        Self {
            kind: EditKind::Other,
            changes,
            order: None,
        }
    }

    fn is_empty(&self) -> bool {
        self.changes.is_empty() && self.order.as_ref().is_none_or(|(a, b)| a == b)
    }

    fn same_targets(&self, other: &Edit) -> bool {
        self.changes.len() == other.changes.len()
            && self
                .changes
                .iter()
                .zip(&other.changes)
                .all(|(a, b)| a.id == b.id)
    }
}

/// A chart's undo and redo stacks.
#[derive(Debug, Clone, Default)]
pub(crate) struct History {
    undo: VecDeque<Edit>,
    redo: Vec<Edit>,
}

impl History {
    pub const fn new() -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
        }
    }

    /// Records a finished gesture; clears the redo stack.
    pub fn push(&mut self, e: Edit) {
        if e.is_empty() {
            return;
        }
        self.redo.clear();
        if e.kind != EditKind::Other
            && let Some(top) = self.undo.back_mut()
            && top.kind == e.kind
            && top.same_targets(&e)
        {
            // Keep the oldest "before", take the newest "after".
            for (t, n) in top.changes.iter_mut().zip(e.changes) {
                t.after = n.after;
            }
            return;
        }
        if self.undo.len() >= MAX_HISTORY {
            self.undo.pop_front();
        }
        self.undo.push_back(e);
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.undo.len()
    }

    /// Reverts the last edit on `items`; returns the drawings it touched
    /// that still exist.
    pub fn undo(&mut self, items: &mut Vec<Drawing>) -> Option<Vec<u64>> {
        let e = self.undo.pop_back()?;
        let ids = apply(items, &e, false);
        self.redo.push(e);
        Some(ids)
    }

    /// Re-applies the last undone edit.
    pub fn redo(&mut self, items: &mut Vec<Drawing>) -> Option<Vec<u64>> {
        let e = self.redo.pop()?;
        let ids = apply(items, &e, true);
        self.undo.push_back(e);
        Some(ids)
    }
}

/// Moves `items` from one side of `e` to the other (`forward`: before →
/// after).
fn apply(items: &mut Vec<Drawing>, e: &Edit, forward: bool) -> Vec<u64> {
    fn pick(c: &Change, after: bool) -> &Option<(usize, Drawing)> {
        if after { &c.after } else { &c.before }
    }
    let side = |c| pick(c, forward);
    let other = |c| pick(c, !forward);
    // Deleted on this side.
    items.retain(|d| {
        !e.changes
            .iter()
            .any(|c| c.id == d.id && side(c).is_none() && other(c).is_some())
    });
    // Changed in place.
    for c in &e.changes {
        if let (Some((_, d)), Some(_)) = (side(c), other(c))
            && let Some(slot) = items.iter_mut().find(|x| x.id == c.id)
        {
            *slot = d.clone();
        }
    }
    // Created on this side, at their recorded index (ascending, so each
    // index refers to the list with the earlier ones already in).
    let mut created: Vec<&(usize, Drawing)> = e
        .changes
        .iter()
        .filter(|c| other(c).is_none())
        .filter_map(|c| side(c).as_ref())
        .collect();
    created.sort_by_key(|(i, _)| *i);
    for (i, d) in created {
        if !items.iter().any(|x| x.id == d.id) {
            items.insert((*i).min(items.len()), d.clone());
        }
    }
    if let Some((before, after)) = &e.order {
        let order = if forward { after } else { before };
        reorder(items, order);
    }
    e.changes
        .iter()
        .filter(|c| side(c).is_some())
        .map(|c| c.id)
        .collect()
}

/// Sorts `items` into `order` (ids not in it go last, in their order).
pub(crate) fn reorder(items: &mut [Drawing], order: &[u64]) {
    items.sort_by_key(|d| {
        order
            .iter()
            .position(|&id| id == d.id)
            .unwrap_or(usize::MAX)
    });
}
