//! Drawing tools, TradingView's left toolbar: trend lines, horizontal and
//! vertical lines, Fib retracement, rectangles and range measurers.
//!
//! Drawings are anchored in (time, price), so they stay put across
//! timeframes and chart types. [`Drawings`] holds the drawings of a chart
//! and the interaction state (tool, selection, placing, dragging); the chart
//! feeds it pointer events in screen points through a [`Mapper`].

use crate::draw::{Scene, Tag, draw_tag, price_axis_tag};
use crate::render::Canvas;
use crate::scale::{format_percent, format_price, format_signed_price};
use crate::time_axis::SlotTimes;
use crate::viewport::Viewport;
use ecolor::Color32;
use emath::{Align2, Pos2, Rect, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tt_core::CandleSeries;
use tt_theme::with_alpha;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Pointer distance (points) that counts as touching a line or handle.
const HIT_DISTANCE: f32 = 5.0;
const HANDLE_RADIUS: f32 = 5.0;
/// A press that moves further than this places a two-point drawing by
/// dragging (otherwise the second point needs a second click).
const DRAG_TO_PLACE: f32 = 4.0;
/// Magnet: snap to a bar's open / high / low / close within this distance.
const MAGNET_DISTANCE: f32 = 30.0;

// TradingView's drawing colors.
const BLUE: Color32 = Color32::from_rgb(0x29, 0x62, 0xff);
const GRAY: Color32 = Color32::from_rgb(0x78, 0x7b, 0x86);
const PURPLE: Color32 = Color32::from_rgb(0x9c, 0x27, 0xb0);
const RED: Color32 = Color32::from_rgb(0xf2, 0x36, 0x45);

/// Fib retracement levels and colors (TradingView's defaults).
pub const FIB_LEVELS: [(f64, Color32); 11] = [
    (0.0, GRAY),
    (0.236, RED),
    (0.382, Color32::from_rgb(0xff, 0x98, 0x00)),
    (0.5, Color32::from_rgb(0x4c, 0xaf, 0x50)),
    (0.618, Color32::from_rgb(0x08, 0x99, 0x81)),
    (0.786, Color32::from_rgb(0x00, 0xbc, 0xd4)),
    (1.0, GRAY),
    (1.618, BLUE),
    (2.618, RED),
    (3.618, PURPLE),
    (4.236, Color32::from_rgb(0xe9, 0x1e, 0x63)),
];

/// Colors offered for drawings (the floating toolbar's swatches).
pub const PALETTE: [Color32; 12] = [
    BLUE,
    RED,
    Color32::from_rgb(0x08, 0x99, 0x81),
    Color32::from_rgb(0xff, 0x98, 0x00),
    Color32::from_rgb(0xff, 0xeb, 0x3b),
    PURPLE,
    Color32::from_rgb(0x00, 0xbc, 0xd4),
    Color32::from_rgb(0xe9, 0x1e, 0x63),
    Color32::from_rgb(0x4c, 0xaf, 0x50),
    GRAY,
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0x13, 0x17, 0x22),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DrawingTool {
    TrendLine,
    Ray,
    ExtendedLine,
    HorizontalLine,
    HorizontalRay,
    VerticalLine,
    CrossLine,
    FibRetracement,
    Rectangle,
    PriceRange,
    DateRange,
    DateAndPriceRange,
}

impl DrawingTool {
    /// TradingView's "Lines" group.
    pub const LINES: [DrawingTool; 7] = [
        DrawingTool::TrendLine,
        DrawingTool::Ray,
        DrawingTool::ExtendedLine,
        DrawingTool::HorizontalLine,
        DrawingTool::HorizontalRay,
        DrawingTool::VerticalLine,
        DrawingTool::CrossLine,
    ];
    /// TradingView's "Measurers".
    pub const MEASURERS: [DrawingTool; 3] = [
        DrawingTool::PriceRange,
        DrawingTool::DateRange,
        DrawingTool::DateAndPriceRange,
    ];

    /// Points to place (one-point tools are placed with a single click).
    pub fn points(self) -> usize {
        match self {
            DrawingTool::HorizontalLine
            | DrawingTool::HorizontalRay
            | DrawingTool::VerticalLine
            | DrawingTool::CrossLine => 1,
            _ => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DrawingTool::TrendLine => "Trend line",
            DrawingTool::Ray => "Ray",
            DrawingTool::ExtendedLine => "Extended line",
            DrawingTool::HorizontalLine => "Horizontal line",
            DrawingTool::HorizontalRay => "Horizontal ray",
            DrawingTool::VerticalLine => "Vertical line",
            DrawingTool::CrossLine => "Cross line",
            DrawingTool::FibRetracement => "Fib retracement",
            DrawingTool::Rectangle => "Rectangle",
            DrawingTool::PriceRange => "Price range",
            DrawingTool::DateRange => "Date range",
            DrawingTool::DateAndPriceRange => "Date and price range",
        }
    }

    /// Keyboard shortcut, as shown in menus (TradingView's).
    pub fn shortcut(self) -> Option<&'static str> {
        Some(match self {
            DrawingTool::TrendLine => "Alt + T",
            DrawingTool::HorizontalLine => "Alt + H",
            DrawingTool::HorizontalRay => "Alt + J",
            DrawingTool::VerticalLine => "Alt + V",
            DrawingTool::CrossLine => "Alt + C",
            DrawingTool::FibRetracement => "Alt + F",
            DrawingTool::Rectangle => "Alt + Shift + R",
            _ => return None,
        })
    }

    /// Lucide icon for toolbars and menus.
    pub fn icon(self) -> lucide_icons::Icon {
        use lucide_icons::Icon;
        match self {
            DrawingTool::TrendLine => Icon::Slash,
            DrawingTool::Ray => Icon::MoveUpRight,
            DrawingTool::ExtendedLine => Icon::MoveDiagonal,
            DrawingTool::HorizontalLine => Icon::Minus,
            DrawingTool::HorizontalRay => Icon::MoveRight,
            DrawingTool::VerticalLine => Icon::SeparatorVertical,
            DrawingTool::CrossLine => Icon::Plus,
            DrawingTool::FibRetracement => Icon::Rows3,
            DrawingTool::Rectangle => Icon::RectangleHorizontal,
            DrawingTool::PriceRange => Icon::MoveVertical,
            DrawingTool::DateRange => Icon::MoveHorizontal,
            DrawingTool::DateAndPriceRange => Icon::RulerDimensionLine,
        }
    }

    fn default_color(self) -> Color32 {
        match self {
            DrawingTool::FibRetracement => GRAY,
            DrawingTool::Rectangle => PURPLE,
            _ => BLUE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl LineStyle {
    pub const ALL: [LineStyle; 3] = [LineStyle::Solid, LineStyle::Dashed, LineStyle::Dotted];

    pub fn label(self) -> &'static str {
        match self {
            LineStyle::Solid => "Solid",
            LineStyle::Dashed => "Dashed",
            LineStyle::Dotted => "Dotted",
        }
    }
}

/// A point of a drawing: bar open time (interpolated between bars) and price.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub time_ms: i64,
    pub price: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Drawing {
    /// Runtime id (not saved).
    #[serde(skip, default = "next_id")]
    pub id: u64,
    pub tool: DrawingTool,
    pub points: Vec<Anchor>,
    pub color: Color32,
    pub width: f32,
    #[serde(default)]
    pub style: LineStyle,
    #[serde(default)]
    pub locked: bool,
}

impl Drawing {
    pub fn new(tool: DrawingTool, points: Vec<Anchor>) -> Self {
        Self {
            id: next_id(),
            tool,
            points,
            color: tool.default_color(),
            width: if tool == DrawingTool::FibRetracement {
                1.0
            } else {
                2.0
            },
            style: LineStyle::Solid,
            locked: false,
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
struct Placing {
    drawing: Drawing,
    /// Points fixed so far (the rest follow the pointer).
    placed: usize,
    down: Pos2,
    screen: Vec<Pos2>,
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

    /// Escape: drops the drawing being placed, else releases the tool, else
    /// clears the selection. Returns whether anything changed.
    pub fn cancel(&mut self) -> bool {
        if self.placing.take().is_some() {
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
            pl.screen = pl.drawing.points.iter().map(|a| m.screen(*a)).collect();
        }
    }

    fn screen_of(&self, id: u64) -> Option<&[Pos2]> {
        self.screen
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, p)| p.as_slice())
    }

    /// The drawing (and part) under `p`, topmost first.
    pub fn hit(&self, p: Pos2) -> Option<(u64, DrawingPart)> {
        if self.hidden || !self.pane.contains(p) {
            return None;
        }
        // Handles of the selected drawing win.
        if let Some(id) = self.selected
            && let Some(pts) = self.screen_of(id)
            && let Some(k) = pts
                .iter()
                .position(|q| q.distance(p) <= HANDLE_RADIUS + 2.0)
        {
            return Some((id, DrawingPart::Handle(k)));
        }
        for d in self.items.iter().rev() {
            let Some(pts) = self.screen_of(d.id) else {
                continue;
            };
            if let Some(k) = pts
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

    /// Pointer pressed in the price pane.
    pub(crate) fn press(&mut self, p: Pos2, m: &Mapper<'_>, series: &CandleSeries) -> DrawingPress {
        if let Some(tool) = self.tool {
            let a = m.snapped(p, self.magnet, series);
            match &mut self.placing {
                None => {
                    let drawing = Drawing::new(tool, vec![a; tool.points()]);
                    let screen = drawing.points.iter().map(|a| m.screen(*a)).collect();
                    self.placing = Some(Placing {
                        drawing,
                        placed: 1,
                        down: p,
                        screen,
                    });
                    if tool.points() == 1 {
                        self.finish();
                        return DrawingPress::Placed;
                    }
                }
                Some(pl) => {
                    pl.drawing.points[pl.placed] = a;
                    pl.placed += 1;
                    if pl.placed >= pl.drawing.points.len() {
                        self.finish();
                        return DrawingPress::Placed;
                    }
                }
            }
            return DrawingPress::Handled;
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
    pub(crate) fn motion(&mut self, p: Pos2, m: &Mapper<'_>, series: &CandleSeries) -> Repaint {
        if let Some(pl) = &mut self.placing {
            let a = m.snapped(p, self.magnet, series);
            for slot in &mut pl.drawing.points[pl.placed..] {
                *slot = a;
            }
            pl.screen = pl.drawing.points.iter().map(|a| m.screen(*a)).collect();
            return Repaint::Overlay;
        }
        if let Some(drag) = &mut self.drag {
            let Some(d) = self.items.iter_mut().find(|d| d.id == drag.id) else {
                self.drag = None;
                return Repaint::None;
            };
            match drag.part {
                DrawingPart::Handle(k) => {
                    if let Some(pt) = d.points.get_mut(k) {
                        *pt = m.snapped(p, self.magnet, series);
                    }
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
            let pts = d.points.iter().map(|a| m.screen(*a)).collect();
            if let Some(s) = self.screen.iter_mut().find(|(i, _)| *i == d.id) {
                s.1 = pts;
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
    pub(crate) fn release(&mut self, p: Pos2) -> bool {
        if let Some(pl) = &mut self.placing
            && pl.placed == 1
            && pl.down.distance(p) > DRAG_TO_PLACE
        {
            // Dragged out: the release places the second point.
            pl.placed = pl.drawing.points.len();
            self.finish();
            return true;
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

    /// The pointer layer shows the drawing being placed.
    pub fn placing(&self) -> bool {
        self.placing.is_some()
    }
}

/// Whether `p` touches a drawing whose screen points are `pts`.
fn touches(tool: DrawingTool, pts: &[Pos2], pane: Rect, plot: Rect, p: Pos2) -> bool {
    let near = |a: Pos2, b: Pos2| dist_to_segment(p, a, b) <= HIT_DISTANCE;
    match tool {
        DrawingTool::HorizontalLine => (p.y - pts[0].y).abs() <= HIT_DISTANCE,
        DrawingTool::HorizontalRay => {
            (p.y - pts[0].y).abs() <= HIT_DISTANCE && p.x >= pts[0].x - HIT_DISTANCE
        }
        DrawingTool::VerticalLine => {
            (p.x - pts[0].x).abs() <= HIT_DISTANCE && plot.y_range().contains(p.y)
        }
        DrawingTool::CrossLine => {
            (p.y - pts[0].y).abs() <= HIT_DISTANCE || (p.x - pts[0].x).abs() <= HIT_DISTANCE
        }
        DrawingTool::TrendLine => near(pts[0], pts[1]),
        DrawingTool::Ray => {
            let (a, b) = extend(pts[0], pts[1], pane, false, true);
            near(a, b)
        }
        DrawingTool::ExtendedLine => {
            let (a, b) = extend(pts[0], pts[1], pane, true, true);
            near(a, b)
        }
        DrawingTool::FibRetracement | DrawingTool::Rectangle => Rect::from_two_pos(pts[0], pts[1])
            .expand(HIT_DISTANCE)
            .contains(p),
        DrawingTool::PriceRange | DrawingTool::DateRange | DrawingTool::DateAndPriceRange => {
            measurer_rect(tool, pts, pane)
                .expand(HIT_DISTANCE)
                .contains(p)
        }
    }
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
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
fn extend(a: Pos2, b: Pos2, r: Rect, back: bool, fwd: bool) -> (Pos2, Pos2) {
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

/// The shaded area of a measurer: like TradingView, the box between the two
/// points for all three (price / date range only label one axis).
fn measurer_rect(_tool: DrawingTool, pts: &[Pos2], _pane: Rect) -> Rect {
    Rect::from_two_pos(pts[0], pts[1])
}

fn styled_line(c: &mut dyn Canvas, a: Pos2, b: Pos2, width: f32, color: Color32, style: LineStyle) {
    match style {
        LineStyle::Solid => c.line(a, b, width, color),
        LineStyle::Dashed => c.dashed_line(
            a,
            b,
            width,
            color,
            6.0 * width.max(1.0),
            4.0 * width.max(1.0),
        ),
        LineStyle::Dotted => {
            c.dashed_line(a, b, width, color, width.max(1.0), 2.5 * width.max(1.0))
        }
    }
}

/// Draws the drawings into the base layer (clipped to the price pane;
/// vertical lines span all panes).
pub(crate) fn draw_base(c: &mut dyn Canvas, s: &Scene<'_>) {
    let ds = s.drawings;
    if ds.hidden || s.main_collapsed {
        return;
    }
    for d in &ds.items {
        if let Some(pts) = ds.screen_of(d.id) {
            draw_one(c, s, d, pts);
        }
    }
    c.set_clip(s.full);
}

/// Axis tags of horizontal lines / rays (base layer, after the price labels).
pub(crate) fn axis_tags(s: &Scene<'_>) -> Vec<Tag> {
    let ds = s.drawings;
    if ds.hidden || s.main_collapsed {
        return Vec::new();
    }
    let pane = s.vp_rect();
    ds.items
        .iter()
        .filter(|d| {
            matches!(
                d.tool,
                DrawingTool::HorizontalLine | DrawingTool::HorizontalRay | DrawingTool::CrossLine
            )
        })
        .filter_map(|d| {
            let y = s.vp.y_of(d.points[0].price);
            pane.y_range().contains(y).then(|| {
                price_axis_tag(
                    s,
                    y,
                    vec![format_price(d.points[0].price, s.precision)],
                    d.color,
                )
            })
        })
        .collect()
}

/// The pointer layer: the drawing being placed and handles of the selected
/// and hovered drawings.
pub(crate) fn draw_overlay(c: &mut dyn Canvas, s: &Scene<'_>) {
    let ds = s.drawings;
    if s.main_collapsed {
        return;
    }
    if let Some(pl) = &ds.placing {
        draw_one(c, s, &pl.drawing, &pl.screen);
        handles(c, s, &pl.screen, pl.drawing.color);
    }
    if ds.hidden {
        c.set_clip(s.full);
        return;
    }
    c.set_clip(s.vp_rect());
    for id in [ds.selected, ds.hovered.filter(|h| Some(*h) != ds.selected)]
        .into_iter()
        .flatten()
    {
        if let (Some(d), Some(pts)) = (ds.items.iter().find(|d| d.id == id), ds.screen_of(id)) {
            handles(c, s, pts, d.color);
        }
    }
    // Selected horizontal lines also show their price on the axis.
    c.set_clip(s.full);
    if let Some(d) = ds.selected_drawing() {
        let pts = ds.screen_of(d.id).unwrap_or(&[]);
        if matches!(
            d.tool,
            DrawingTool::TrendLine
                | DrawingTool::Ray
                | DrawingTool::ExtendedLine
                | DrawingTool::Rectangle
        ) {
            for (a, pt) in d.points.iter().zip(pts) {
                if s.vp_rect().y_range().contains(pt.y) {
                    draw_tag(
                        c,
                        s,
                        &price_axis_tag(s, pt.y, vec![format_price(a.price, s.precision)], d.color),
                    );
                }
            }
        }
    }
}

fn handles(c: &mut dyn Canvas, s: &Scene<'_>, pts: &[Pos2], color: Color32) {
    for p in pts {
        c.fill_circle(*p, HANDLE_RADIUS, color);
        c.fill_circle(*p, HANDLE_RADIUS - 1.5, s.theme.bg);
    }
}

fn draw_one(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    let pane = s.vp_rect();
    c.set_clip(pane);
    let (w, col, st) = (d.width, d.color, d.style);
    match d.tool {
        DrawingTool::TrendLine => styled_line(c, pts[0], pts[1], w, col, st),
        DrawingTool::Ray => {
            let (a, b) = extend(pts[0], pts[1], pane, false, true);
            styled_line(c, a, b, w, col, st);
        }
        DrawingTool::ExtendedLine => {
            let (a, b) = extend(pts[0], pts[1], pane, true, true);
            styled_line(c, a, b, w, col, st);
        }
        DrawingTool::HorizontalLine => styled_line(
            c,
            pos2(pane.left(), pts[0].y),
            pos2(pane.right(), pts[0].y),
            w,
            col,
            st,
        ),
        DrawingTool::HorizontalRay => {
            styled_line(c, pts[0], pos2(pane.right(), pts[0].y), w, col, st)
        }
        DrawingTool::VerticalLine | DrawingTool::CrossLine => {
            c.set_clip(s.plot);
            styled_line(
                c,
                pos2(pts[0].x, s.plot.top()),
                pos2(pts[0].x, s.plot.bottom()),
                w,
                col,
                st,
            );
            c.set_clip(pane);
            if d.tool == DrawingTool::CrossLine {
                styled_line(
                    c,
                    pos2(pane.left(), pts[0].y),
                    pos2(pane.right(), pts[0].y),
                    w,
                    col,
                    st,
                );
            }
        }
        DrawingTool::Rectangle => {
            let r = Rect::from_two_pos(pts[0], pts[1]);
            c.fill_rect(r, with_alpha(col, 0.2));
            let corners = [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
                r.left_top(),
            ];
            for e in corners.windows(2) {
                styled_line(c, e[0], e[1], w, col, st);
            }
        }
        DrawingTool::FibRetracement => fib(c, s, d, pts),
        DrawingTool::PriceRange | DrawingTool::DateRange | DrawingTool::DateAndPriceRange => {
            measurer(c, s, d, pts)
        }
    }
}

/// Fib retracement: level `l` sits at `end + (start - end) * l`, so 0 is at
/// the second point and 1 at the first (TradingView's convention).
fn fib(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    let (p0, p1) = (d.points[0].price, d.points[1].price);
    let (x0, x1) = (pts[0].x.min(pts[1].x), pts[0].x.max(pts[1].x));
    let sc = s.vp.scale;
    let level_price =
        |l: f64| sc.from_scaled(sc.to_scaled(p1) + (sc.to_scaled(p0) - sc.to_scaled(p1)) * l);
    let ys: Vec<(f32, f64, Color32, f64)> = FIB_LEVELS
        .iter()
        .map(|&(l, col)| {
            let price = level_price(l);
            (s.vp.y_of(price), l, col, price)
        })
        .collect();
    // Bands between consecutive levels.
    for w in ys.windows(2) {
        let r = Rect::from_x_y_ranges(x0..=x1, w[0].0.min(w[1].0)..=w[0].0.max(w[1].0));
        c.fill_rect(r, with_alpha(w[1].2, 0.12));
    }
    for (y, l, col, price) in &ys {
        styled_line(c, pos2(x0, *y), pos2(x1, *y), d.width, *col, d.style);
        let label = format!("{} ({})", trim_level(*l), format_price(*price, s.precision));
        c.text(
            pos2(x0 - 4.0, *y),
            Align2::RIGHT_CENTER,
            &label,
            s.style.scales.text_size - 1.0,
            *col,
        );
    }
    // The trend line between the two points.
    c.dashed_line(pts[0], pts[1], 1.0, with_alpha(d.color, 0.8), 4.0, 4.0);
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

/// Price range / date range / date and price range: a shaded box between the
/// two points, arrows along the measured axes and a label with the numbers.
fn measurer(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    let (a, b) = (d.points[0], d.points[1]);
    let up = b.price >= a.price;
    let col = if d.tool == DrawingTool::DateRange || up {
        d.color
    } else {
        RED
    };
    let r = Rect::from_two_pos(pts[0], pts[1]);
    c.fill_rect(r, with_alpha(col, 0.2));
    let mid = r.center();
    let (price, date) = match d.tool {
        DrawingTool::PriceRange => (true, false),
        DrawingTool::DateRange => (false, true),
        _ => (true, true),
    };
    if price {
        arrow(c, pos2(mid.x, pts[0].y), pos2(mid.x, pts[1].y), col);
    }
    if date {
        arrow(c, pos2(pts[0].x, mid.y), pos2(pts[1].x, mid.y), col);
    }
    let mut lines = Vec::new();
    if price {
        let diff = b.price - a.price;
        let pct = if a.price != 0.0 {
            diff / a.price * 100.0
        } else {
            0.0
        };
        lines.push(format!(
            "{} ({})",
            format_signed_price(diff, s.precision),
            format_percent(pct)
        ));
    }
    if date {
        let bars = ((pts[1].x - pts[0].x) / s.vp.bar_spacing).round() as i64;
        lines.push(format!(
            "{bars} bars, {}",
            format_span(b.time_ms - a.time_ms)
        ));
    }
    let size = s.style.scales.text_size;
    let line_h = size + 4.0;
    let width = lines
        .iter()
        .map(|l| c.text_size(l, size).x)
        .fold(0.0, f32::max)
        + 16.0;
    let h = line_h * lines.len() as f32 + 8.0;
    // Below the box when measuring down, above when up (TradingView).
    let top = if up {
        r.top() - h - 6.0
    } else {
        r.bottom() + 6.0
    };
    let label = Rect::from_min_size(pos2(mid.x - width / 2.0, top), vec2(width, h));
    c.fill_rounded_rect(label, 4.0, col);
    for (i, l) in lines.iter().enumerate() {
        c.text(
            pos2(
                label.center().x,
                label.top() + 4.0 + line_h * (i as f32 + 0.5),
            ),
            Align2::CENTER_CENTER,
            l,
            size,
            s.theme.text_on_accent,
        );
    }
}

fn arrow(c: &mut dyn Canvas, from: Pos2, to: Pos2, col: Color32) {
    c.line(from, to, 1.0, col);
    let d = to - from;
    if d.length() < 8.0 {
        return;
    }
    let dir = d.normalized();
    let side = Vec2::new(-dir.y, dir.x) * 4.0;
    let back = to - dir * 7.0;
    c.line(to, back + side, 1.0, col);
    c.line(to, back - side, 1.0, col);
}

/// Signed duration, e.g. `2d 3h`, `45m`, `-1h 30m`.
fn format_span(ms: i64) -> String {
    let sign = if ms < 0 { "-" } else { "" };
    let m = ms.unsigned_abs() / 60_000;
    let (d, h, mm) = (m / 1440, m / 60 % 24, m % 60);
    let s = match (d, h, mm) {
        (0, 0, mm) => format!("{mm}m"),
        (0, h, 0) => format!("{h}h"),
        (0, h, mm) => format!("{h}h {mm}m"),
        (d, 0, _) => format!("{d}d"),
        (d, h, _) => format!("{d}d {h}h"),
    };
    format!("{sign}{s}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::{PlotRect, PriceScale};

    fn mapper(times: &[i64]) -> Mapper<'_> {
        Mapper {
            vp: Viewport {
                bar_spacing: 10.0,
                right_offset_bars: 0.0,
                price_min: 0.0,
                price_max: 100.0,
                plot: PlotRect {
                    left: 0.0,
                    top: 0.0,
                    right: 1000.0,
                    bottom: 500.0,
                },
                bar_count: times.len(),
                scale: PriceScale::linear(),
            },
            slots: SlotTimes::new(times, 60_000),
        }
    }

    fn series(n: usize) -> CandleSeries {
        let mut s = CandleSeries::new();
        for i in 0..n {
            s.upsert(tt_core::Candle {
                open_time_ms: i as i64 * 60_000,
                open: 50.0,
                high: 60.0,
                low: 40.0,
                close: 55.0,
                volume: 1.0,
            });
        }
        s
    }

    #[test]
    fn anchors_round_trip_and_extend() {
        let times: Vec<i64> = (0..100).map(|i| i * 60_000).collect();
        let m = mapper(&times);
        let p = m.screen(Anchor {
            time_ms: 50 * 60_000,
            price: 25.0,
        });
        let a = m.anchor(p);
        assert_eq!(a.time_ms, 50 * 60_000);
        assert!((a.price - 25.0).abs() < 1e-3);
        // Past the last bar: extrapolated by the interval.
        let far = m.anchor(pos2(m.vp.plot.right + 0.0, 0.0));
        assert!(far.time_ms >= 99 * 60_000);

        let r = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
        let (a, b) = extend(pos2(10.0, 10.0), pos2(20.0, 20.0), r, true, true);
        assert!(a.distance(pos2(0.0, 0.0)) < 1e-3 && b.distance(pos2(100.0, 100.0)) < 1e-3);
        let (a, b) = extend(pos2(10.0, 10.0), pos2(20.0, 20.0), r, false, true);
        assert_eq!(a, pos2(10.0, 10.0));
        assert!(b.distance(pos2(100.0, 100.0)) < 1e-3);
    }

    #[test]
    fn place_select_drag_and_delete() {
        let times: Vec<i64> = (0..100).map(|i| i * 60_000).collect();
        let m = mapper(&times);
        let s = series(100);
        let pane = Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 500.0));
        let mut ds = Drawings {
            tool: Some(DrawingTool::TrendLine),
            ..Drawings::empty()
        };
        // Click, move, click.
        assert_eq!(ds.press(pos2(105.0, 100.0), &m, &s), DrawingPress::Handled);
        assert!(!ds.release(pos2(105.0, 100.0)));
        assert_eq!(ds.motion(pos2(305.0, 300.0), &m, &s), Repaint::Overlay);
        assert_eq!(ds.press(pos2(305.0, 300.0), &m, &s), DrawingPress::Placed);
        assert_eq!(ds.items.len(), 1);
        assert_eq!(ds.tool, None, "tool released without keep drawing");
        let id = ds.items[0].id;
        assert_eq!(ds.selected, Some(id));
        ds.layout(&m, pane, pane);
        assert!(ds.take_dirty());

        // Drag-to-place a rectangle in one gesture.
        ds.tool = Some(DrawingTool::Rectangle);
        ds.press(pos2(505.0, 100.0), &m, &s);
        ds.motion(pos2(605.0, 200.0), &m, &s);
        assert!(ds.release(pos2(605.0, 200.0)));
        assert_eq!(ds.items.len(), 2);
        ds.layout(&m, pane, pane);

        // Grab the trend line's body and move it right by 10 bars, up 10.
        let mid = pos2(205.0, 200.0);
        assert_eq!(ds.hit(mid), Some((id, DrawingPart::Body)));
        assert_eq!(ds.press(mid, &m, &s), DrawingPress::Handled);
        assert_eq!(ds.motion(mid + vec2(100.0, -50.0), &m, &s), Repaint::All);
        assert!(ds.release(mid + vec2(100.0, -50.0)));
        let d = &ds.items[0];
        assert_eq!(d.points[0].time_ms, 20 * 60_000);
        assert!((d.points[0].price - 90.0).abs() < 1e-3, "{:?}", d.points);

        // Magnet snaps to the bar's high.
        ds.magnet = true;
        let a = m.snapped(pos2(505.0, m.vp.y_of(61.0)), true, &s);
        assert_eq!(a.price, 60.0);

        // Empty space deselects; Delete removes the selection.
        assert_eq!(ds.press(pos2(900.0, 480.0), &m, &s), DrawingPress::Handled);
        assert_eq!(ds.selected, None);
        assert_eq!(ds.press(pos2(900.0, 480.0), &m, &s), DrawingPress::Ignored);
        ds.selected = Some(id);
        assert!(ds.delete_selected());
        assert_eq!(ds.items.len(), 1);

        // One-point tools place with a single click; Escape cancels a tool.
        ds.tool = Some(DrawingTool::HorizontalLine);
        assert_eq!(ds.press(pos2(10.0, 250.0), &m, &s), DrawingPress::Placed);
        ds.tool = Some(DrawingTool::Ray);
        assert!(ds.cancel());
        assert_eq!(ds.tool, None);
    }

    #[test]
    fn spans_and_serde() {
        assert_eq!(format_span(90 * 60_000), "1h 30m");
        assert_eq!(format_span(-(2 * 1440 + 180) * 60_000), "-2d 3h");
        let d = Drawing::new(
            DrawingTool::FibRetracement,
            vec![
                Anchor {
                    time_ms: 1,
                    price: 2.0
                };
                2
            ],
        );
        let json = serde_json::to_string(&d).unwrap();
        let back: Drawing = serde_json::from_str(&json).unwrap();
        assert_eq!(back.points, d.points);
        assert_eq!(back.tool, d.tool);
    }
}
