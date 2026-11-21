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
//! - `select.rs`: selection set, rubber band, clipboard, visual order,
//!   nudge; `history.rs`: undo / redo; `constrain.rs`: Shift constraints;
//! - `hit.rs`: hit testing; `render.rs`: drawing dispatch and helpers;
//! - family files: `lines.rs`, `channel.rs`, `fib.rs`, `shapes.rs`,
//!   `freehand.rs`, `text.rs`, `position.rs`, `measure.rs`; `stats.rs`.

mod channel;
mod constrain;
mod fib;
pub mod form;
mod freehand;
mod history;
mod hit;
mod lines;
mod measure;
mod model;
mod place;
mod position;
pub mod props;
mod render;
mod select;
mod shapes;
mod stats;
pub mod templates;
mod text;

#[cfg(test)]
mod tests;

pub use model::{
    Anchor, Drawing, DrawingList, DrawingMap, DrawingTool, FIB_LEVELS, LineStyle, PALETTE,
    Placement, Rgba, Span, ToolGroup, Visibility, VisibilityPreset, deserialize_drawing_map,
};
pub use props::{Caps, Props, Stroke};
pub(crate) use render::{axis_tags, draw_base, draw_overlay};
pub use shapes::ResizeCursor;
pub use templates::{DrawingTemplates, NamedTemplate, ToolTemplates};

use crate::time_axis::SlotTimes;
use crate::viewport::Viewport;
use constrain::{Axis, constraint, pick_axis};
use emath::{Pos2, Rect, pos2};
use history::{Change, Edit, EditKind, History};
use place::Placing;
use tt_core::{CandleSeries, Timeframe};

/// Pointer distance (points) that counts as touching a line or handle.
const HIT_DISTANCE: f32 = 5.0;
const HANDLE_RADIUS: f32 = 5.0;
/// A press that moves further than this places a two-point drawing by
/// dragging (otherwise the second point needs a second click).
const DRAG_TO_PLACE: f32 = 4.0;
/// Magnet: snap to a bar's open / high / low / close within this distance.
const MAGNET_DISTANCE: f32 = 30.0;

/// Keyboard modifiers held during a pointer event: Shift constrains angles,
/// shapes and drag axes; Ctrl toggles selection, rubber-band selects,
/// clones on drag and inverts the magnet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
}

/// The magnet (spec 4.5): snap placed points and dragged handles to the
/// O / H / L / C of the bar under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MagnetMode {
    #[default]
    Off,
    /// Snaps only within a distance, so free placement is still possible.
    Weak,
    /// Always snaps.
    Strong,
}

impl MagnetMode {
    pub const ALL: [MagnetMode; 3] = [MagnetMode::Off, MagnetMode::Weak, MagnetMode::Strong];

    pub fn is_on(self) -> bool {
        self != MagnetMode::Off
    }

    /// Ctrl inverts the magnet for a gesture: off → strong, on → off.
    pub fn with_ctrl(self, ctrl: bool) -> Self {
        match (ctrl, self) {
            (false, m) => m,
            (true, MagnetMode::Off) => MagnetMode::Strong,
            (true, _) => MagnetMode::Off,
        }
    }

    /// The next mode (a toolbar button cycling off → weak → strong).
    pub fn next(self) -> Self {
        match self {
            MagnetMode::Off => MagnetMode::Weak,
            MagnetMode::Weak => MagnetMode::Strong,
            MagnetMode::Strong => MagnetMode::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MagnetMode::Off => "Magnet off",
            MagnetMode::Weak => "Weak magnet",
            MagnetMode::Strong => "Strong magnet",
        }
    }
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
    /// nearest open / high / low / close of the bar under `p` (weak: only
    /// within [`MAGNET_DISTANCE`]).
    pub fn snapped(&self, p: Pos2, magnet: MagnetMode, series: &CandleSeries) -> Anchor {
        let mut a = self.anchor(p);
        if !magnet.is_on() {
            return a;
        }
        let reach = match magnet {
            MagnetMode::Weak => MAGNET_DISTANCE,
            _ => f32::INFINITY,
        };
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
            && d <= reach
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

    /// The anchor for pointer `p` under a Shift constraint relative to
    /// `from` (at `from_pt` on screen). On an exact axis the reference's
    /// price or time is kept, so a horizontal line stays exactly level.
    fn constrained(
        &self,
        c: constrain::Constraint,
        from: Anchor,
        from_pt: Pos2,
        p: Pos2,
    ) -> Anchor {
        let q = constrain::apply(c, from_pt, p);
        let mut a = self.anchor(q);
        if q.y == from_pt.y {
            a.price = from.price;
        }
        if q.x == from_pt.x {
            a.time_ms = from.time_ms;
        }
        a
    }

    /// `a`'s time moved by `slots` bars (price unchanged).
    fn shift_time(&self, a: Anchor, slots: f64) -> i64 {
        let slot = self.slots.slot_of(a.time_ms).unwrap_or(0.0) + slots;
        self.slots.time_at_frac(slot).unwrap_or(a.time_ms)
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
    /// Double-click on a drawing (now selected): open its settings.
    Open(u64),
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

/// What a right-click in the chart is on (which context menu to show).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingMenuTarget {
    /// Not in the price pane, or a gesture is in progress: no drawing menu.
    None,
    /// The click finished the path / polyline being placed (no menu).
    Finished,
    /// On a drawing (now selected).
    Drawing(u64),
    /// Empty space in the price pane (Paste, Remove drawings).
    Empty,
}

/// Visual order operations on the selection (context menu, toolbar "…").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualOrder {
    BringToFront,
    BringForward,
    SendBackward,
    SendToBack,
}

impl VisualOrder {
    pub const ALL: [VisualOrder; 4] = [
        VisualOrder::BringToFront,
        VisualOrder::BringForward,
        VisualOrder::SendBackward,
        VisualOrder::SendToBack,
    ];

    pub fn label(self) -> &'static str {
        match self {
            VisualOrder::BringToFront => "Bring to front",
            VisualOrder::BringForward => "Bring forward",
            VisualOrder::SendBackward => "Send backward",
            VisualOrder::SendToBack => "Send to back",
        }
    }
}

/// How a press on a drawing turns into a drag or a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragMode {
    /// Moves points or bodies from the first motion.
    Move,
    /// Ctrl + press on a body: dragging clones, a click toggles the
    /// drawing in the selection.
    CtrlPress,
    /// Press on a member of a multi-selection: dragging moves them all, a
    /// click selects only this one.
    MemberPress,
}

#[derive(Debug, Clone)]
struct Drag {
    /// The drawing pressed.
    id: u64,
    part: DrawingPart,
    down: Pos2,
    start_slot: f64,
    start_price: f64,
    /// The drawings being moved as they were at the press (after a clone:
    /// the clones).
    orig: Vec<Drawing>,
    mode: DragMode,
    cloned: bool,
    /// Shift body drag: the axis picked by the first motion.
    axis: Option<Axis>,
    moved: bool,
}

/// A chart's drawings and the drawing interaction.
#[derive(Debug, Clone)]
pub struct Drawings {
    /// In visual order (last on top).
    pub items: Vec<Drawing>,
    /// Active tool (`None` = cursor).
    pub tool: Option<DrawingTool>,
    /// Snap to bar open / high / low / close.
    pub magnet: MagnetMode,
    /// Keep the tool after placing a drawing.
    pub keep_drawing: bool,
    /// Lock all drawings (no selecting or moving).
    pub locked: bool,
    pub hidden: bool,
    pub hovered: Option<u64>,
    /// Selected drawing ids; the first is the primary one (the floating
    /// toolbar shows its style).
    selection: Vec<u64>,
    placing: Option<Placing>,
    drag: Option<Drag>,
    /// Ctrl + drag rubber band: press point and pointer.
    band: Option<(Pos2, Pos2)>,
    history: History,
    /// Screen points of each drawing in the last base frame.
    screen: Vec<(u64, Vec<Pos2>)>,
    /// Price pane and all panes in the last frame.
    pane: Rect,
    plot: Rect,
    /// Changed since [`Self::take_dirty`].
    dirty: bool,
    /// The chart's interval: drawings whose Visibility excludes it are not
    /// drawn or hit (`None`: shown on every interval).
    timeframe: Option<Timeframe>,
    /// Style new drawings start from, per tool (the tool's saved default
    /// template); tools not listed use `Props::defaults`.
    tool_defaults: Vec<(DrawingTool, Props)>,
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
            magnet: MagnetMode::Off,
            keep_drawing: false,
            locked: false,
            hidden: false,
            hovered: None,
            selection: Vec::new(),
            placing: None,
            drag: None,
            band: None,
            history: History::new(),
            screen: Vec::new(),
            pane: Rect::NOTHING,
            plot: Rect::NOTHING,
            dirty: false,
            timeframe: None,
            tool_defaults: Vec::new(),
        }
    }

    /// Sets the chart's interval (Visibility tab). Drawings that no longer
    /// show leave the selection and hover.
    pub fn set_timeframe(&mut self, tf: Option<Timeframe>) {
        if self.timeframe == tf {
            return;
        }
        self.timeframe = tf;
        let items = &self.items;
        let shown = |id: &u64| items.iter().any(|d| d.id == *id && d.shown_on(tf));
        self.selection.retain(shown);
        if self.hovered.is_some_and(|h| !shown(&h)) {
            self.hovered = None;
        }
    }

    pub fn timeframe(&self) -> Option<Timeframe> {
        self.timeframe
    }

    /// Whether `d` is drawn and can be hit: not hidden and visible on the
    /// chart's interval.
    pub fn is_shown(&self, d: &Drawing) -> bool {
        d.shown_on(self.timeframe)
    }

    /// Sets the style new drawings of each tool start from.
    pub fn set_tool_defaults(&mut self, defaults: Vec<(DrawingTool, Props)>) {
        self.tool_defaults = defaults;
    }

    /// The style a new drawing of `tool` starts from.
    pub fn new_props(&self, tool: DrawingTool) -> Props {
        self.tool_defaults
            .iter()
            .find(|(t, p)| *t == tool && p.kind() == Props::defaults(tool).kind())
            .map(|(_, p)| p.clone())
            .unwrap_or_else(|| Props::defaults(tool))
    }

    /// Edits the selected drawings' style from the floating toolbar.
    /// Consecutive edits of the same `slot` (e.g. dragging a color slider)
    /// on the same drawings are one undo step.
    pub fn edit_selected_style(&mut self, slot: u8, f: impl FnMut(&mut Drawing)) -> bool {
        self.edit_selected_kind(EditKind::Style(slot), f)
    }

    /// Changes drawings without recording undo steps (the settings
    /// dialog's live preview); [`Self::commit_preview`] records the result.
    pub fn preview(&mut self, ids: &[u64], mut f: impl FnMut(&mut Drawing)) -> bool {
        let mut any = false;
        for d in self.items.iter_mut().filter(|d| ids.contains(&d.id)) {
            f(d);
            any = true;
        }
        any
    }

    /// Puts drawings back as they were (the settings dialog's Cancel); not
    /// an undo step.
    pub fn restore(&mut self, before: &[Drawing]) {
        for b in before {
            if let Some(i) = self.index_of(b.id) {
                self.items[i] = b.clone();
            }
        }
    }

    /// The settings dialog's Ok: records the drawings' changes since
    /// `before` (their state at open) as one undo step.
    pub fn commit_preview(&mut self, before: &[Drawing]) -> bool {
        let after: Vec<Drawing> = before
            .iter()
            .filter_map(|b| self.items.iter().find(|d| d.id == b.id).cloned())
            .filter(|a| before.iter().any(|b| b.id == a.id && b != a))
            .collect();
        if after.is_empty() {
            return false;
        }
        self.restore(before);
        self.replace_drawings(after)
    }

    /// Replaces the drawings (e.g. another symbol's), keeping the tool. The
    /// undo history is cleared.
    pub fn set_items(&mut self, items: Vec<Drawing>) {
        self.items = items;
        self.selection.clear();
        self.hovered = None;
        self.placing = None;
        self.drag = None;
        self.band = None;
        self.history.clear();
        self.screen.clear();
        self.dirty = false;
    }

    /// Whether drawings changed since the last call (for saving).
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// Records a finished gesture for undo and marks the drawings changed.
    fn record(&mut self, e: Edit) {
        self.history.push(e);
        self.dirty = true;
    }

    fn index_of(&self, id: u64) -> Option<usize> {
        self.items.iter().position(|d| d.id == id)
    }

    /// Edits the selected drawings (color, width, style, lock, hide).
    pub fn edit_selected(&mut self, f: impl FnMut(&mut Drawing)) -> bool {
        self.edit_selected_kind(EditKind::Other, f)
    }

    fn edit_selected_kind(&mut self, kind: EditKind, mut f: impl FnMut(&mut Drawing)) -> bool {
        let mut changes = Vec::new();
        for (i, d) in self.items.iter_mut().enumerate() {
            if !self.selection.contains(&d.id) {
                continue;
            }
            let before = d.clone();
            f(d);
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
            kind,
            changes,
            order: None,
        });
        true
    }

    /// Replaces the drawing with `d`'s id (the settings dialog's Ok), as
    /// one undo step.
    pub fn replace_drawing(&mut self, d: Drawing) -> bool {
        self.replace_drawings(vec![d])
    }

    /// Replaces the drawings with these ids, as one undo step.
    pub fn replace_drawings(&mut self, ds: Vec<Drawing>) -> bool {
        let mut changes = Vec::new();
        for d in ds {
            let Some(i) = self.index_of(d.id) else {
                continue;
            };
            let before = std::mem::replace(&mut self.items[i], d.clone());
            changes.push(Change {
                id: d.id,
                before: Some((i, before)),
                after: Some((i, d)),
            });
        }
        if changes.is_empty() {
            return false;
        }
        self.record(Edit::new(changes));
        true
    }

    /// Removes the selected drawings.
    pub fn delete_selected(&mut self) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        let sel = std::mem::take(&mut self.selection);
        self.remove_where(|d| sel.contains(&d.id))
    }

    /// Removes every drawing (undoable).
    pub fn remove_all(&mut self) -> bool {
        self.selection.clear();
        self.placing = None;
        self.drag = None;
        self.band = None;
        self.remove_where(|_| true)
    }

    fn remove_where(&mut self, f: impl Fn(&Drawing) -> bool) -> bool {
        let changes: Vec<Change> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, d)| f(d))
            .map(|(i, d)| Change {
                id: d.id,
                before: Some((i, d.clone())),
                after: None,
            })
            .collect();
        if changes.is_empty() {
            return false;
        }
        self.items.retain(|d| !f(d));
        self.screen
            .retain(|(id, _)| !changes.iter().any(|c| c.id == *id));
        if self
            .hovered
            .is_some_and(|h| changes.iter().any(|c| c.id == h))
        {
            self.hovered = None;
        }
        self.record(Edit::new(changes));
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.busy() && self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        !self.busy() && self.history.can_redo()
    }

    /// Ctrl + Z: reverts the last drawing change (not while placing or
    /// dragging). Selects what it touched.
    pub fn undo(&mut self) -> bool {
        if self.busy() {
            return false;
        }
        let Some(ids) = self.history.undo(&mut self.items) else {
            return false;
        };
        self.after_history(ids);
        true
    }

    /// Ctrl + Y / Ctrl + Shift + Z.
    pub fn redo(&mut self) -> bool {
        if self.busy() {
            return false;
        }
        let Some(ids) = self.history.redo(&mut self.items) else {
            return false;
        };
        self.after_history(ids);
        true
    }

    fn after_history(&mut self, ids: Vec<u64>) {
        let items = &self.items;
        let exists = |id: &u64| items.iter().any(|d| d.id == *id);
        if ids.is_empty() {
            self.selection.retain(exists);
        } else {
            self.selection = ids;
            self.selection
                .retain(|id| items.iter().any(|d| d.id == *id));
        }
        self.screen
            .retain(|(id, _)| items.iter().any(|d| d.id == *id));
        if self.hovered.is_some_and(|h| !exists(&h)) {
            self.hovered = None;
        }
        // Screen points of restored drawings come with the next layout
        // (the chart repaints both layers after an undo).
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
        if self.band.take().is_some() {
            return true;
        }
        if self.tool.take().is_some() {
            return true;
        }
        self.clear_selection()
    }

    /// A drawing is being placed or dragged, or a rubber band is open.
    pub fn busy(&self) -> bool {
        self.placing.is_some() || self.drag.is_some() || self.band.is_some()
    }

    /// The resize pointer over (or while dragging) a rectangle's handle at
    /// `p` (TradingView: diagonal at the corners, horizontal / vertical at
    /// the edges); `None` elsewhere.
    pub fn resize_cursor(&self, p: Pos2) -> Option<ResizeCursor> {
        let (id, k) = match &self.drag {
            Some(Drag {
                id,
                part: DrawingPart::Handle(k),
                ..
            }) => (*id, *k),
            Some(_) => return None,
            None if self.busy() => return None,
            None => match self.hit(p)? {
                (id, DrawingPart::Handle(k)) => (id, k),
                _ => return None,
            },
        };
        let d = self.items.iter().find(|d| d.id == id)?;
        if d.tool != DrawingTool::Rectangle || d.locked || self.locked {
            return None;
        }
        shapes::rect_cursor(self.screen_of(id)?, k)
    }

    /// The pointer layer shows the drawing being placed.
    pub fn placing(&self) -> bool {
        self.placing.is_some()
    }

    /// A Ctrl + drag rubber band is open.
    pub fn banding(&self) -> bool {
        self.band.is_some()
    }

    /// Recomputes screen points after a base layout.
    pub(crate) fn layout(&mut self, m: &Mapper<'_>, pane: Rect, plot: Rect) {
        self.pane = pane;
        self.plot = plot;
        // Reuse the per-drawing point buffers (no allocation when the
        // drawings did not change).
        self.screen.truncate(self.items.len());
        for (k, d) in self.items.iter().enumerate() {
            match self.screen.get_mut(k) {
                Some((id, pts)) => {
                    *id = d.id;
                    m.screen_into(&d.points, pts);
                }
                None => self
                    .screen
                    .push((d.id, d.points.iter().map(|a| m.screen(*a)).collect())),
            }
        }
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
            if mods.ctrl && !self.hidden && !double {
                // Ctrl + drag on empty space: rubber-band select.
                self.band = Some((p, p));
                return DrawingPress::Handled;
            }
            if self.clear_selection() {
                return DrawingPress::Handled;
            }
            return DrawingPress::Ignored;
        };
        let was_selected = self.is_selected(id);
        if double && !mods.ctrl {
            // The first press selected it; keep a multi-selection of the
            // same drawing (the dialog edits them all).
            if !was_selected {
                self.select_only(id);
            }
            return DrawingPress::Open(id);
        }
        let mode = if mods.ctrl && part == DrawingPart::Body {
            DragMode::CtrlPress
        } else if was_selected && self.selection.len() > 1 && part == DrawingPart::Body {
            DragMode::MemberPress
        } else {
            DragMode::Move
        };
        if mode == DragMode::Move && !was_selected {
            self.select_only(id);
        }
        // What the drag moves: a handle moves its drawing; a body drag the
        // whole selection (the pressed drawing alone if it is not in it).
        let movable = |d: &Drawing| !self.locked && !d.locked;
        let orig: Vec<Drawing> = match (part, mode) {
            (DrawingPart::Handle(_), _) => self
                .items
                .iter()
                .filter(|d| d.id == id && movable(d))
                .cloned()
                .collect(),
            (DrawingPart::Body, DragMode::CtrlPress) if self.locked => Vec::new(),
            (DrawingPart::Body, DragMode::CtrlPress) => self
                .items
                .iter()
                .filter(|d| {
                    if was_selected {
                        self.selection.contains(&d.id)
                    } else {
                        d.id == id
                    }
                })
                .cloned()
                .collect(),
            (DrawingPart::Body, _) => self
                .items
                .iter()
                .filter(|d| self.selection.contains(&d.id) && movable(d))
                .cloned()
                .collect(),
        };
        self.drag = Some(Drag {
            id,
            part,
            down: p,
            start_slot: m.vp.index_at(p.x) as f64,
            start_price: m.vp.price_at(p.y),
            orig,
            mode,
            cloned: false,
            axis: None,
            moved: false,
        });
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
        if let Some((_, to)) = &mut self.band {
            *to = p;
            return Repaint::Overlay;
        }
        if self.drag.is_some() {
            return self.drag_motion(p, mods, m, series);
        }
        let hovered = self.hit(p).map(|(id, _)| id);
        if hovered != self.hovered {
            self.hovered = hovered;
            return Repaint::Overlay;
        }
        Repaint::None
    }

    fn drag_motion(
        &mut self,
        p: Pos2,
        mods: Mods,
        m: &Mapper<'_>,
        series: &CandleSeries,
    ) -> Repaint {
        let Some(drag) = &mut self.drag else {
            return Repaint::None;
        };
        let d = p - drag.down;
        if drag.mode != DragMode::Move {
            // A click until the pointer travels.
            if d.length() <= DRAG_TO_PLACE {
                return Repaint::None;
            }
            if drag.mode == DragMode::CtrlPress && !drag.orig.is_empty() {
                // Ctrl + drag: the copies follow the pointer, the originals
                // stay.
                let mut ids = Vec::with_capacity(drag.orig.len());
                for o in &mut drag.orig {
                    let src = o.id;
                    o.id = model::next_id();
                    o.locked = false;
                    ids.push(o.id);
                    self.items.push(o.clone());
                    let pts = self
                        .screen
                        .iter()
                        .find(|(i, _)| *i == src)
                        .map(|(_, p)| p.clone())
                        .unwrap_or_default();
                    self.screen.push((o.id, pts));
                }
                drag.cloned = true;
                self.selection = ids;
            }
            drag.mode = DragMode::Move;
        }
        drag.moved = true;
        if drag.orig.is_empty() {
            return Repaint::None;
        }
        let Self {
            items,
            screen,
            magnet,
            ..
        } = self;
        match drag.part {
            DrawingPart::Handle(k) => {
                let Some(d) = items.iter_mut().find(|d| d.id == drag.orig[0].id) else {
                    return Repaint::None;
                };
                let s = screen.iter_mut().find(|(i, _)| *i == d.id);
                let from = if !mods.shift {
                    None
                } else if d.tool == DrawingTool::Rectangle {
                    // Corners stay square relative to the opposite corner;
                    // edge handles move one side, unconstrained.
                    s.as_ref().and_then(|(_, pts)| {
                        let (a, pt) = shapes::opposite_corner(k, &d.points, pts)?;
                        Some((constrain::Constraint::Square, a, pt))
                    })
                } else {
                    constraint(d.tool, k, d.points.len()).and_then(|(c, r)| {
                        let pts = &s.as_ref()?.1;
                        Some((c, *d.points.get(r)?, *pts.get(r)?))
                    })
                };
                let a = match from {
                    Some((c, from, from_pt)) => m.constrained(c, from, from_pt, p),
                    None => m.snapped(p, magnet.with_ctrl(mods.ctrl), series),
                };
                hit::drag_handle(d, k, a, m);
                if let Some(s) = s {
                    m.screen_into(&d.points, &mut s.1);
                }
            }
            DrawingPart::Body => {
                if mods.shift {
                    if drag.axis.is_none() {
                        drag.axis = pick_axis(d);
                    }
                    if drag.axis.is_none() {
                        return Repaint::None;
                    }
                } else {
                    drag.axis = None;
                }
                let mut dslot = (m.vp.index_at(p.x) as f64 - drag.start_slot).round();
                let mut to = m.vp.price_at(p.y);
                match drag.axis {
                    Some(Axis::Horizontal) => to = drag.start_price,
                    Some(Axis::Vertical) => dslot = 0.0,
                    None => {}
                }
                for o in &drag.orig {
                    let Some(d) = items.iter_mut().find(|d| d.id == o.id) else {
                        continue;
                    };
                    for (pt, a) in d.points.iter_mut().zip(&o.points) {
                        *pt = m.shifted(*a, dslot, drag.start_price, to);
                    }
                    if let Some(s) = screen.iter_mut().find(|(i, _)| *i == d.id) {
                        m.screen_into(&d.points, &mut s.1);
                    }
                }
            }
        }
        self.dirty = true;
        Repaint::All
    }

    /// Pointer released; returns whether drawings changed (save them).
    pub(crate) fn release(&mut self, p: Pos2, mods: Mods) -> bool {
        if self.placing.is_some() {
            return self.place_release(p, mods);
        }
        if let Some((a, _)) = self.band.take() {
            if a.distance(p) > DRAG_TO_PLACE {
                self.select_in(Rect::from_two_pos(a, p));
            }
            return false;
        }
        let Some(drag) = self.drag.take() else {
            return false;
        };
        if !drag.moved {
            match drag.mode {
                DragMode::CtrlPress => self.toggle_selected(drag.id),
                DragMode::MemberPress => self.select_only(drag.id),
                DragMode::Move => {}
            }
            return false;
        }
        let changes: Vec<Change> = drag
            .orig
            .into_iter()
            .filter_map(|o| {
                let i = self.index_of(o.id)?;
                let now = &self.items[i];
                if drag.cloned {
                    Some(Change {
                        id: o.id,
                        before: None,
                        after: Some((i, now.clone())),
                    })
                } else if now.points != o.points {
                    Some(Change {
                        id: o.id,
                        after: Some((i, now.clone())),
                        before: Some((i, o)),
                    })
                } else {
                    None
                }
            })
            .collect();
        if changes.is_empty() {
            return false;
        }
        self.record(Edit::new(changes));
        true
    }

    /// Commits the drawing being placed.
    fn finish(&mut self) {
        let Some(pl) = self.placing.take() else {
            return;
        };
        let id = pl.drawing.id;
        self.screen.push((id, pl.screen));
        let after = (self.items.len(), pl.drawing.clone());
        self.items.push(pl.drawing);
        self.select_only(id);
        self.record(Edit::new(vec![Change {
            id,
            before: None,
            after: Some(after),
        }]));
        if !self.keep_drawing {
            self.tool = None;
        }
    }
}
