//! Drawing data: tools, anchors, colors, visibility and the saved drawing.
//!
//! [`Drawing`] is (de)serialized through [`DrawingRepr`](super::props) so
//! that old files migrate and older builds can still read new files; see
//! `props.rs`. [`DrawingList`] loads a symbol's drawings element by element,
//! so one drawing this build can't read never discards the others.

use super::props::{DrawingRepr, Props};
use ecolor::Color32;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::atomic::{AtomicU64, Ordering};
use tt_core::{TimeUnit, Timeframe};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

// TradingView's drawing colors.
pub(crate) const BLUE: Color32 = Color32::from_rgb(0x29, 0x62, 0xff);
pub(crate) const GRAY: Color32 = Color32::from_rgb(0x78, 0x7b, 0x86);
pub(crate) const PURPLE: Color32 = Color32::from_rgb(0x9c, 0x27, 0xb0);
pub(crate) const RED: Color32 = Color32::from_rgb(0xf2, 0x36, 0x45);
pub(crate) const ORANGE: Color32 = Color32::from_rgb(0xff, 0x98, 0x00);
pub(crate) const GREEN: Color32 = Color32::from_rgb(0x4c, 0xaf, 0x50);
pub(crate) const TEAL: Color32 = Color32::from_rgb(0x08, 0x99, 0x81);
pub(crate) const CYAN: Color32 = Color32::from_rgb(0x00, 0xbc, 0xd4);
pub(crate) const PINK: Color32 = Color32::from_rgb(0xe9, 0x1e, 0x63);

/// Fib retracement levels and colors (TradingView's defaults).
pub const FIB_LEVELS: [(f64, Color32); 11] = [
    (0.0, GRAY),
    (0.236, RED),
    (0.382, ORANGE),
    (0.5, GREEN),
    (0.618, TEAL),
    (0.786, CYAN),
    (1.0, GRAY),
    (1.618, BLUE),
    (2.618, RED),
    (3.618, PURPLE),
    (4.236, PINK),
];

/// Colors offered for drawings (the floating toolbar's swatches).
pub const PALETTE: [Color32; 12] = [
    BLUE,
    RED,
    TEAL,
    ORANGE,
    Color32::from_rgb(0xff, 0xeb, 0x3b),
    PURPLE,
    CYAN,
    PINK,
    GREEN,
    GRAY,
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0x13, 0x17, 0x22),
];

/// Every drawing tool, including planned ones ([`DrawingTool::is_available`]
/// says which are implemented). Variants are saved by name: add new ones,
/// never rename one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DrawingTool {
    // Lines.
    TrendLine,
    Ray,
    InfoLine,
    ExtendedLine,
    TrendAngle,
    HorizontalLine,
    HorizontalRay,
    VerticalLine,
    CrossLine,
    ParallelChannel,
    // Fibonacci.
    FibRetracement,
    FibExtension,
    // Shapes.
    Rectangle,
    Circle,
    Ellipse,
    Path,
    Polyline,
    Arrow,
    Brush,
    // Annotation.
    Text,
    Note,
    Callout,
    PriceLabel,
    // Forecasting.
    LongPosition,
    ShortPosition,
    // Measurers.
    PriceRange,
    DateRange,
    DateAndPriceRange,
}

/// The left toolbar's groups (TradingView's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolGroup {
    Lines,
    Fib,
    Shapes,
    Annotation,
    Forecasting,
    Measurers,
}

/// How a tool is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// A fixed number of clicks (two-point tools can also be dragged out).
    Points(usize),
    /// Click to add points; double-click, Enter or Escape finishes.
    OpenEnded,
    /// Press, drag and release records the pointer's path.
    Freehand,
}

impl Placement {
    /// Fewest points a finished drawing has.
    pub fn min_points(self) -> usize {
        match self {
            Placement::Points(n) => n,
            Placement::OpenEnded | Placement::Freehand => 2,
        }
    }
}

impl DrawingTool {
    pub const ALL: [DrawingTool; 28] = [
        DrawingTool::TrendLine,
        DrawingTool::Ray,
        DrawingTool::InfoLine,
        DrawingTool::ExtendedLine,
        DrawingTool::TrendAngle,
        DrawingTool::HorizontalLine,
        DrawingTool::HorizontalRay,
        DrawingTool::VerticalLine,
        DrawingTool::CrossLine,
        DrawingTool::ParallelChannel,
        DrawingTool::FibRetracement,
        DrawingTool::FibExtension,
        DrawingTool::Rectangle,
        DrawingTool::Circle,
        DrawingTool::Ellipse,
        DrawingTool::Path,
        DrawingTool::Polyline,
        DrawingTool::Arrow,
        DrawingTool::Brush,
        DrawingTool::Text,
        DrawingTool::Note,
        DrawingTool::Callout,
        DrawingTool::PriceLabel,
        DrawingTool::LongPosition,
        DrawingTool::ShortPosition,
        DrawingTool::PriceRange,
        DrawingTool::DateRange,
        DrawingTool::DateAndPriceRange,
    ];

    /// The left toolbar's Lines flyout: TradingView's "Lines" group, plus
    /// the arrow until a Shapes flyout exists.
    pub const LINES: [DrawingTool; 11] = [
        DrawingTool::TrendLine,
        DrawingTool::Ray,
        DrawingTool::InfoLine,
        DrawingTool::ExtendedLine,
        DrawingTool::TrendAngle,
        DrawingTool::HorizontalLine,
        DrawingTool::HorizontalRay,
        DrawingTool::VerticalLine,
        DrawingTool::CrossLine,
        DrawingTool::ParallelChannel,
        DrawingTool::Arrow,
    ];
    /// TradingView's Fibonacci group.
    pub const FIB: [DrawingTool; 2] = [DrawingTool::FibRetracement, DrawingTool::FibExtension];
    /// TradingView's "Measurers".
    pub const MEASURERS: [DrawingTool; 3] = [
        DrawingTool::PriceRange,
        DrawingTool::DateRange,
        DrawingTool::DateAndPriceRange,
    ];

    /// Whether the tool is implemented (offered in toolbars). A slice that
    /// implements a tool flips its arm here.
    pub fn is_available(self) -> bool {
        use DrawingTool::*;
        match self {
            TrendLine | Ray | ExtendedLine | InfoLine | TrendAngle | Arrow | HorizontalLine
            | HorizontalRay | VerticalLine | CrossLine | ParallelChannel | FibRetracement
            | FibExtension | Rectangle | PriceRange | DateRange | DateAndPriceRange => true,
            Circle | Ellipse | Path | Polyline | Brush | Text | Note | Callout | PriceLabel
            | LongPosition | ShortPosition => false,
        }
    }

    /// The implemented tools, in toolbar order.
    pub fn available() -> impl Iterator<Item = DrawingTool> {
        Self::ALL.into_iter().filter(|t| t.is_available())
    }

    pub fn group(self) -> ToolGroup {
        use DrawingTool::*;
        match self {
            TrendLine | Ray | InfoLine | ExtendedLine | TrendAngle | HorizontalLine
            | HorizontalRay | VerticalLine | CrossLine | ParallelChannel => ToolGroup::Lines,
            FibRetracement | FibExtension => ToolGroup::Fib,
            Rectangle | Circle | Ellipse | Path | Polyline | Arrow | Brush => ToolGroup::Shapes,
            Text | Note | Callout | PriceLabel => ToolGroup::Annotation,
            LongPosition | ShortPosition => ToolGroup::Forecasting,
            PriceRange | DateRange | DateAndPriceRange => ToolGroup::Measurers,
        }
    }

    pub fn placement(self) -> Placement {
        use DrawingTool::*;
        match self {
            HorizontalLine | HorizontalRay | VerticalLine | CrossLine | Text | Note
            | PriceLabel | LongPosition | ShortPosition => Placement::Points(1),
            ParallelChannel | FibExtension => Placement::Points(3),
            Path | Polyline => Placement::OpenEnded,
            Brush => Placement::Freehand,
            TrendLine | Ray | InfoLine | ExtendedLine | TrendAngle | FibRetracement | Rectangle
            | Circle | Ellipse | Arrow | Callout | PriceRange | DateRange | DateAndPriceRange => {
                Placement::Points(2)
            }
        }
    }

    pub fn label(self) -> &'static str {
        use DrawingTool::*;
        match self {
            TrendLine => "Trend line",
            Ray => "Ray",
            InfoLine => "Info line",
            ExtendedLine => "Extended line",
            TrendAngle => "Trend angle",
            HorizontalLine => "Horizontal line",
            HorizontalRay => "Horizontal ray",
            VerticalLine => "Vertical line",
            CrossLine => "Cross line",
            ParallelChannel => "Parallel channel",
            FibRetracement => "Fib retracement",
            FibExtension => "Trend-based Fib extension",
            Rectangle => "Rectangle",
            Circle => "Circle",
            Ellipse => "Ellipse",
            Path => "Path",
            Polyline => "Polyline",
            Arrow => "Arrow",
            Brush => "Brush",
            Text => "Text",
            Note => "Note",
            Callout => "Callout",
            PriceLabel => "Price label",
            LongPosition => "Long position",
            ShortPosition => "Short position",
            PriceRange => "Price range",
            DateRange => "Date range",
            DateAndPriceRange => "Date and price range",
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
        use DrawingTool::*;
        use lucide_icons::Icon;
        match self {
            TrendLine => Icon::Slash,
            Ray => Icon::MoveUpRight,
            InfoLine => Icon::Info,
            ExtendedLine => Icon::MoveDiagonal,
            TrendAngle => Icon::Radius,
            HorizontalLine => Icon::Minus,
            HorizontalRay => Icon::MoveRight,
            VerticalLine => Icon::SeparatorVertical,
            CrossLine => Icon::Plus,
            ParallelChannel => Icon::Equal,
            FibRetracement => Icon::Rows3,
            FibExtension => Icon::Rows4,
            Rectangle => Icon::RectangleHorizontal,
            Circle => Icon::Circle,
            Ellipse => Icon::Diameter,
            Path => Icon::Spline,
            Polyline => Icon::Pentagon,
            Arrow => Icon::ArrowUpRight,
            Brush => Icon::Brush,
            Text => Icon::Type,
            Note => Icon::StickyNote,
            Callout => Icon::MessageSquare,
            PriceLabel => Icon::Tag,
            LongPosition => Icon::TrendingUp,
            ShortPosition => Icon::TrendingDown,
            PriceRange => Icon::MoveVertical,
            DateRange => Icon::MoveHorizontal,
            DateAndPriceRange => Icon::RulerDimensionLine,
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

/// Drawing colors share the chart settings' straight-alpha color type.
pub use crate::style::Rgba;

/// A point of a drawing: bar open time (interpolated between bars) and price.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub time_ms: i64,
    pub price: f64,
}

/// One interval unit's row of the Visibility tab: shown when on and the
/// interval's multiplier is within `min..=max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub on: bool,
    pub min: u16,
    pub max: u16,
}

impl Span {
    pub const fn full(max: u16) -> Self {
        Self {
            on: true,
            min: 1,
            max,
        }
    }

    /// Whether a multiplier of this row's unit is shown.
    pub fn contains(self, n: u32) -> bool {
        self.on && (self.min as u32..=self.max as u32).contains(&n)
    }
}

/// Quick presets of the Visibility tab ("Visibility on intervals ▸").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityPreset {
    CurrentAndAbove,
    CurrentAndBelow,
    CurrentOnly,
    All,
}

impl VisibilityPreset {
    pub const ALL: [VisibilityPreset; 4] = [
        VisibilityPreset::CurrentAndAbove,
        VisibilityPreset::CurrentAndBelow,
        VisibilityPreset::CurrentOnly,
        VisibilityPreset::All,
    ];

    pub fn label(self) -> &'static str {
        match self {
            VisibilityPreset::CurrentAndAbove => "Current interval and above",
            VisibilityPreset::CurrentAndBelow => "Current interval and below",
            VisibilityPreset::CurrentOnly => "Current interval only",
            VisibilityPreset::All => "All intervals",
        }
    }
}

/// On which intervals a drawing shows (TradingView's Visibility tab).
/// Evaluation and presets come with the settings dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Visibility {
    pub seconds: Span,
    pub minutes: Span,
    pub hours: Span,
    pub days: Span,
    pub weeks: Span,
    pub months: Span,
}

impl Visibility {
    pub const ALL: Visibility = Visibility {
        seconds: Span::full(59),
        minutes: Span::full(59),
        hours: Span::full(24),
        days: Span::full(366),
        weeks: Span::full(52),
        months: Span::full(12),
    };

    /// Shown on every interval (the default).
    pub fn is_all(&self) -> bool {
        *self == Self::ALL
    }

    /// The rows in tab order.
    pub const UNITS: [TimeUnit; 6] = TimeUnit::ALL;

    /// The row of an interval unit.
    pub fn span(&self, unit: TimeUnit) -> Span {
        match unit {
            TimeUnit::Second => self.seconds,
            TimeUnit::Minute => self.minutes,
            TimeUnit::Hour => self.hours,
            TimeUnit::Day => self.days,
            TimeUnit::Week => self.weeks,
            TimeUnit::Month => self.months,
        }
    }

    pub fn span_mut(&mut self, unit: TimeUnit) -> &mut Span {
        match unit {
            TimeUnit::Second => &mut self.seconds,
            TimeUnit::Minute => &mut self.minutes,
            TimeUnit::Hour => &mut self.hours,
            TimeUnit::Day => &mut self.days,
            TimeUnit::Week => &mut self.weeks,
            TimeUnit::Month => &mut self.months,
        }
    }

    /// Largest multiplier of a row (the end of its range).
    pub fn row_max(unit: TimeUnit) -> u16 {
        Self::ALL.span(unit).max
    }

    /// The row and multiplier an interval is tested against. Minute counts
    /// of an hour or more (90m, 120m) count as hours (TradingView's rows
    /// stop at 59 minutes), so no interval falls outside every row.
    pub fn row_of(tf: Timeframe) -> (TimeUnit, u32) {
        match (tf.unit(), tf.count()) {
            (TimeUnit::Minute, n) if n >= 60 => (TimeUnit::Hour, n / 60),
            (u, n) => (u, n),
        }
    }

    /// Whether a drawing with this visibility shows on interval `tf`: the
    /// interval's row is on and its multiplier is within the row's range.
    pub fn shows(&self, tf: Timeframe) -> bool {
        let (unit, n) = Self::row_of(tf);
        self.span(unit).contains(n)
    }

    /// A preset relative to the current interval `tf`.
    pub fn preset(p: VisibilityPreset, tf: Timeframe) -> Visibility {
        use std::cmp::Ordering::*;
        let (cur, n) = Self::row_of(tf);
        let n = n.min(u16::MAX as u32) as u16;
        let mut v = Self::ALL;
        if p == VisibilityPreset::All {
            return v;
        }
        for unit in Self::UNITS {
            let s = v.span_mut(unit);
            match (p, unit.cmp(&cur)) {
                (_, Equal) => {
                    let n = n.clamp(1, s.max);
                    match p {
                        VisibilityPreset::CurrentAndAbove => s.min = n,
                        VisibilityPreset::CurrentAndBelow => s.max = n,
                        _ => {
                            s.min = n;
                            s.max = n;
                        }
                    }
                }
                (VisibilityPreset::CurrentAndAbove, Less)
                | (VisibilityPreset::CurrentAndBelow, Greater)
                | (VisibilityPreset::CurrentOnly, _) => s.on = false,
                _ => {}
            }
        }
        v
    }
}

impl Default for Visibility {
    fn default() -> Self {
        Self::ALL
    }
}

/// A drawing. Saved via `DrawingRepr` (see `props.rs`): old files migrate
/// on load and the legacy `color` / `width` / `style` keep being written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "DrawingRepr", into = "DrawingRepr")]
pub struct Drawing {
    /// Runtime id (not saved).
    pub id: u64,
    pub tool: DrawingTool,
    pub points: Vec<Anchor>,
    /// Style (the template payload).
    pub props: Props,
    /// Text content (not part of templates).
    pub text: String,
    pub locked: bool,
    pub hidden: bool,
    pub visibility: Visibility,
}

impl Drawing {
    /// A drawing with the tool's default style.
    pub fn new(tool: DrawingTool, points: Vec<Anchor>) -> Self {
        Self {
            id: next_id(),
            tool,
            points,
            props: Props::defaults(tool),
            text: String::new(),
            locked: false,
            hidden: false,
            visibility: Visibility::ALL,
        }
    }

    /// The main line's color (what the floating toolbar's color button
    /// edits).
    pub fn color(&self) -> Color32 {
        self.props.line_color().to_color32()
    }

    pub fn set_color(&mut self, c: Color32) {
        self.props.set_line_color(Rgba::from_color32(c));
    }

    /// The main line's width in points (1 to 4).
    pub fn width(&self) -> f32 {
        self.props.line_width() as f32
    }

    pub fn set_width(&mut self, w: f32) {
        self.props.set_line_width(w.round().clamp(1.0, 4.0) as u8);
    }

    pub fn style(&self) -> LineStyle {
        self.props.line_style()
    }

    pub fn set_style(&mut self, s: LineStyle) {
        self.props.set_line_style(s);
    }

    /// Applies a template or default style: only `props` change; points,
    /// text, lock, hidden and visibility stay. A style of another tool
    /// family is ignored (returns false).
    pub fn apply_style(&mut self, props: &Props) -> bool {
        if props.kind() != self.props.kind() {
            return false;
        }
        self.props = props.clone();
        true
    }

    /// Whether the drawing shows on interval `tf` (`None`: unknown, shown).
    pub fn shown_on(&self, tf: Option<Timeframe>) -> bool {
        !self.hidden && tf.is_none_or(|tf| self.visibility.shows(tf))
    }
}

/// A symbol's drawings as saved: the drawings this build can read, plus the
/// raw JSON of those it can't (e.g. a tool from a newer build), which is
/// written back unchanged on save so nothing is lost.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrawingList {
    pub items: Vec<Drawing>,
    pub unknown: Vec<serde_json::Value>,
}

impl DrawingList {
    pub fn new(items: Vec<Drawing>) -> Self {
        Self {
            items,
            unknown: Vec::new(),
        }
    }

    /// No drawings at all (readable or not).
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.unknown.is_empty()
    }

    /// Reads one drawing per element, keeping the unreadable ones raw.
    pub fn from_value(v: serde_json::Value) -> Self {
        let mut list = Self::default();
        let elements = match v {
            serde_json::Value::Array(a) => a,
            serde_json::Value::Null => Vec::new(),
            other => {
                tracing::warn!("drawings: expected a list, keeping the value as is");
                list.unknown.push(other);
                return list;
            }
        };
        for e in elements {
            match Drawing::deserialize(&e) {
                Ok(d) => list.items.push(d),
                Err(err) => {
                    tracing::warn!("skipping a drawing this build can't read: {err}");
                    list.unknown.push(e);
                }
            }
        }
        list
    }
}

impl Serialize for DrawingList {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(self.items.len() + self.unknown.len()))?;
        for d in &self.items {
            seq.serialize_element(d)?;
        }
        for v in &self.unknown {
            seq.serialize_element(v)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for DrawingList {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::from_value(serde_json::Value::deserialize(d)?))
    }
}

/// Drawings per symbol key.
pub type DrawingMap = std::collections::BTreeMap<String, DrawingList>;

/// `deserialize_with` for a [`DrawingMap`] field: never fails on content.
/// A value that is not a map is dropped (with a warning) instead of failing
/// the whole settings file; each list loads leniently ([`DrawingList`]).
pub fn deserialize_drawing_map<'de, D: Deserializer<'de>>(d: D) -> Result<DrawingMap, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Object(m) => m
            .into_iter()
            .map(|(k, v)| (k, DrawingList::from_value(v)))
            .collect(),
        serde_json::Value::Null => DrawingMap::new(),
        _ => {
            tracing::warn!("drawings: expected a map of symbol to drawings, ignoring them");
            DrawingMap::new()
        }
    })
}
