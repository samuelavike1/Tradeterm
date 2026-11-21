//! Drawing tools, TradingView's left toolbar: trend lines, horizontal and
//! vertical lines, Fib retracement, rectangles and range measurers.
//!
//! Drawings are anchored in (time, price), so they stay put across
//! timeframes and chart types. [`Drawings`] holds the drawings of a chart
//! and the interaction state (tool, selection, placing, dragging); the chart
//! feeds it pointer events in screen points through a [`Mapper`].
//!
//! Layout (so tool families can be worked on independently):
//! - `model.rs`: [`Drawing`], [`Anchor`], [`DrawingTool`], [`Rgba`],
//!   [`Visibility`], [`DrawingList`] (lenient loading);
//! - `props.rs`: [`Props`] per tool family, defaults, overlay, saved form;
//! - `place.rs`: placement (fixed points, open-ended, freehand);
//! - `hit.rs`: hit testing; `render.rs`: drawing dispatch and helpers;
//! - family files: `lines.rs`, `channel.rs`, `fib.rs`, `shapes.rs`,
//!   `freehand.rs`, `text.rs`, `position.rs`, `measure.rs`; `stats.rs`.

mod channel;
mod fib;
mod freehand;
mod hit;
mod lines;
mod measure;
mod model;
mod place;
mod position;
pub mod props;
mod render;
mod shapes;
mod stats;
mod text;

#[cfg(test)]
mod tests;

pub use model::{
    Anchor, Drawing, DrawingList, DrawingMap, DrawingTool, FIB_LEVELS, LineStyle, PALETTE,
    Placement, Rgba, Span, ToolGroup, Visibility, deserialize_drawing_map,
};
pub use props::{Caps, Props, Stroke};
pub(crate) use render::{axis_tags, draw_base, draw_overlay};

use crate::time_axis::SlotTimes;
use crate::viewport::Viewport;
use emath::{Pos2, Rect, pos2};
use place::Placing;
use tt_core::CandleSeries;

/// Pointer distance (points) that counts as touching a line or handle.
const HIT_DISTANCE: f32 = 5.0;
const HANDLE_RADIUS: f32 = 5.0;
/// A press that moves further than this places a two-point drawing by
/// dragging (otherwise the second point needs a second click).
const DRAG_TO_PLACE: f32 = 4.0;
/// Magnet: snap to a bar's open / high / low / close within this distance.
const MAGNET_DISTANCE: f32 = 30.0;

/// Keyboard modifiers held during a pointer event. Not used yet; they will
/// constrain angles / axes (Shift), clone and invert the magnet (Ctrl).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
}

/// Screen ↔ (time, price) for the price pane.
pub(crate) struct Mapper<'a> {
    pub vp: Viewport,
    pub slots: SlotTimes<'a>,
}

impl Mapper<'_> {
    pub fn screen(&self, a: Anchor) -> Pos2 {
        let slot = self.slots.slot_of(a.time_ms).unwrap_or(0.0);
        pos2(self.vp.x_of(slot as f32), self.vp.y_of(a.price))
    }

    /// The anchor under `p`, snapped to the bar slot.
    pub fn anchor(&self, p: Pos2) -> Anchor {
        let slot = self.vp.slot_at(p.x);
        Anchor {
            time_ms: self.slots.time_at(slot).unwrap_or(0),
            price: self.vp.price_at(p.y),
        }
    }

    /// Like [`Self::anchor`]; with the magnet on, the price snaps to the
    /// nearest open / high / low / close of the bar under `p`.
    pub fn snapped(&self, p: Pos2, magnet: bool, series: &CandleSeries) -> Anchor {
        let mut a = self.anchor(p);
        if !magnet {
            return a;
        }
        let slot = self.vp.slot_at(p.x);
        if slot < 0 || slot as usize >= series.len() {
            return a;
        }
        let i = slot as usize;
        let ohlc = [
            series.opens()[i],
            series.highs()[i],
            series.lows()[i],
            series.closes()[i],
        ];
        if let Some((price, d)) = ohlc
            .iter()
            .map(|&v| (v, (self.vp.y_of(v) - p.y).abs()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            && d <= MAGNET_DISTANCE
        {
            a.price = price;
        }
        a
    }

    /// Moves `a` by whole bar slots and a price delta in scale space.
    fn shifted(&self, a: Anchor, slots: f64, from_price: f64, to_price: f64) -> Anchor {
        let s = self.vp.scale;
        let slot = self.slots.slot_of(a.time_ms).unwrap_or(0.0) + slots;
        Anchor {
            time_ms: self.slots.time_at_frac(slot).unwrap_or(a.time_ms),
            price: s.from_scaled(
                s.to_scaled(a.price) + s.to_scaled(to_price) - s.to_scaled(from_price),
            ),
        }
    }

    /// Screen points of `points` into `out` (reusing its allocation).
    fn screen_into(&self, points: &[Anchor], out: &mut Vec<Pos2>) {
        out.clear();
        out.extend(points.iter().map(|a| self.screen(*a)));
    }
}

/// Part of a drawing under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingPart {
    /// Point `k`'s handle.
    Handle(usize),
    Body,
}

/// What a press did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingPress {
    /// Not on a drawing and no tool active: the chart handles it.
    Ignored,
    /// Placing, selecting or starting a drag.
    Handled,
    /// A drawing was completed (and the tool may have been released).
    Placed,
}

/// What needs repainting after pointer motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repaint {
    None,
    /// Only the pointer layer (hover, the drawing being placed).
    Overlay,
    /// Both layers (a drawing moved).
    All,
}

#[derive(Debug, Clone)]
struct Drag {
    id: u64,
    part: DrawingPart,
    start_slot: f64,
    start_price: f64,
    orig: Vec<Anchor>,
    moved: bool,
}

/// A chart's drawings and the drawing interaction.
#[derive(Debug, Clone)]
pub struct Drawings {
    pub items: Vec<Drawing>,
    /// Active tool (`None` = cursor).
    pub tool: Option<DrawingTool>,
    /// Snap to bar open / high / low / close.
    pub magnet: bool,
    /// Keep the tool after placing a drawing.
    pub keep_drawing: bool,
    /// Lock all drawings (no selecting or moving).
    pub locked: bool,
    pub hidden: bool,
    pub selected: Option<u64>,
    pub hovered: Option<u64>,
    placing: Option<Placing>,
    drag: Option<Drag>,
    /// Screen points of each drawing in the last base frame.
    screen: Vec<(u64, Vec<Pos2>)>,
    /// Price pane and all panes in the last frame.
    pane: Rect,
    plot: Rect,
    /// Changed since [`Self::take_dirty`].
    dirty: bool,
}

impl Default for Drawings {
    fn default() -> Self {
        Self::empty()
    }
}

impl Drawings {
    pub const fn empty() -> Self {
        Self {
            items: Vec::new(),
            tool: None,
            magnet: false,
            keep_drawing: false,
            locked: false,
            hidden: false,
            selected: None,
            hovered: None,
            placing: None,
            drag: None,
            screen: Vec::new(),
            pane: Rect::NOTHING,
            plot: Rect::NOTHING,
            dirty: false,
        }
    }

    /// Replaces the drawings (e.g. another symbol's), keeping the tool.
    pub fn set_items(&mut self, items: Vec<Drawing>) {
        self.items = items;
        self.selected = None;
        self.hovered = None;
        self.placing = None;
        self.drag = None;
        self.screen.clear();
        self.dirty = false;
    }

    /// Whether drawings changed since the last call (for saving).
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn selected_drawing(&self) -> Option<&Drawing> {
        let id = self.selected?;
        self.items.iter().find(|d| d.id == id)
    }

    /// Edits the selected drawing (color, width, style, lock).
    pub fn edit_selected(&mut self, f: impl FnOnce(&mut Drawing)) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        match self.items.iter_mut().find(|d| d.id == id) {
            Some(d) => {
                f(d);
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn delete_selected(&mut self) -> bool {
        let Some(id) = self.selected.take() else {
            return false;
        };
        self.items.retain(|d| d.id != id);
        self.screen.retain(|(i, _)| *i != id);
        self.dirty = true;
        true
    }

    pub fn remove_all(&mut self) {
        self.set_items(Vec::new());
        self.dirty = true;
    }

    /// Escape: finishes an open-ended drawing (path, polyline) or drops the
    /// drawing being placed, else releases the tool, else clears the
    /// selection. Returns whether anything changed.
    pub fn cancel(&mut self) -> bool {
        if self.placing.is_some() {
            if !self.finish_placing() {
                self.placing = None;
            }
            return true;
        }
        if self.tool.take().is_some() {
            return true;
        }
        self.selected.take().is_some()
    }

    /// A drawing is being placed or dragged.
    pub fn busy(&self) -> bool {
        self.placing.is_some() || self.drag.is_some()
    }

    /// The pointer layer shows the drawing being placed.
    pub fn placing(&self) -> bool {
        self.placing.is_some()
    }

    /// Recomputes screen points after a base layout.
    pub(crate) fn layout(&mut self, m: &Mapper<'_>, pane: Rect, plot: Rect) {
        self.pane = pane;
        self.plot = plot;
        self.screen = self
            .items
            .iter()
            .map(|d| (d.id, d.points.iter().map(|a| m.screen(*a)).collect()))
            .collect();
        if let Some(pl) = &mut self.placing {
            m.screen_into(&pl.drawing.points, &mut pl.screen);
        }
    }

    fn screen_of(&self, id: u64) -> Option<&[Pos2]> {
        self.screen
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, p)| p.as_slice())
    }

    /// Pointer pressed in the price pane (`double`: the second press of a
    /// double-click).
    pub(crate) fn press(
        &mut self,
        p: Pos2,
        mods: Mods,
        double: bool,
        m: &Mapper<'_>,
        series: &CandleSeries,
    ) -> DrawingPress {
        if let Some(tool) = self.tool {
            return self.place_press(tool, p, mods, double, m, series);
        }
        let Some((id, part)) = self.hit(p) else {
            if self.selected.take().is_some() {
                return DrawingPress::Handled;
            }
            return DrawingPress::Ignored;
        };
        self.selected = Some(id);
        let locked = self.locked || self.items.iter().any(|d| d.id == id && d.locked);
        if !locked && let Some(d) = self.items.iter().find(|d| d.id == id) {
            self.drag = Some(Drag {
                id,
                part,
                start_slot: m.vp.index_at(p.x) as f64,
                start_price: m.vp.price_at(p.y),
                orig: d.points.clone(),
                moved: false,
            });
        }
        DrawingPress::Handled
    }

    /// Pointer moved (pressed or not).
    pub(crate) fn motion(
        &mut self,
        p: Pos2,
        mods: Mods,
        m: &Mapper<'_>,
        series: &CandleSeries,
    ) -> Repaint {
        if self.placing.is_some() {
            return self.place_motion(p, mods, m, series);
        }
        if let Some(drag) = &mut self.drag {
            let Some(d) = self.items.iter_mut().find(|d| d.id == drag.id) else {
                self.drag = None;
                return Repaint::None;
            };
            match drag.part {
                DrawingPart::Handle(k) => {
                    hit::drag_handle(d, k, m.snapped(p, self.magnet, series), m);
                }
                DrawingPart::Body => {
                    let dslot = (m.vp.index_at(p.x) as f64 - drag.start_slot).round();
                    let to = m.vp.price_at(p.y);
                    for (pt, o) in d.points.iter_mut().zip(&drag.orig) {
                        *pt = m.shifted(*o, dslot, drag.start_price, to);
                    }
                }
            }
            drag.moved = true;
            if let Some(s) = self.screen.iter_mut().find(|(i, _)| *i == d.id) {
                m.screen_into(&d.points, &mut s.1);
            }
            self.dirty = true;
            return Repaint::All;
        }
        let hovered = self.hit(p).map(|(id, _)| id);
        if hovered != self.hovered {
            self.hovered = hovered;
            return Repaint::Overlay;
        }
        Repaint::None
    }

    /// Pointer released; returns whether drawings changed (save them).
    pub(crate) fn release(&mut self, p: Pos2, mods: Mods) -> bool {
        if self.placing.is_some() {
            return self.place_release(p, mods);
        }
        match self.drag.take() {
            Some(drag) => drag.moved,
            None => false,
        }
    }

    /// Commits the drawing being placed.
    fn finish(&mut self) {
        let Some(pl) = self.placing.take() else {
            return;
        };
        let id = pl.drawing.id;
        self.screen.push((id, pl.screen));
        self.items.push(pl.drawing);
        self.selected = Some(id);
        self.dirty = true;
        if !self.keep_drawing {
            self.tool = None;
        }
    }
}
