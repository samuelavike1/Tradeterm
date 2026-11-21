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

    /// TradingView's "Lines" group (available tools).
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

    /// Whether the tool is implemented (offered in toolbars). A slice that
    /// implements a tool flips its arm here.
    pub fn is_available(self) -> bool {
        use DrawingTool::*;
        match self {
            TrendLine | Ray | ExtendedLine | HorizontalLine | HorizontalRay | VerticalLine
            | CrossLine | FibRetracement | Rectangle | PriceRange | DateRange
            | DateAndPriceRange => true,
            InfoLine | TrendAngle | ParallelChannel | FibExtension | Circle | Ellipse | Path
            | Polyline | Arrow | Brush | Text | Note | Callout | PriceLabel | LongPosition
            | ShortPosition => false,
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

/// A straight-alpha (not premultiplied) sRGB color, saved as `"#RRGGBBAA"`.
/// Also reads `"#RRGGBB"` and `[r, g, b, a]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    /// The opaque color of `c` (its alpha is dropped).
    pub const fn opaque(c: Color32) -> Self {
        Self([c.r(), c.g(), c.b(), 255])
    }

    pub const fn r(self) -> u8 {
        self.0[0]
    }
    pub const fn g(self) -> u8 {
        self.0[1]
    }
    pub const fn b(self) -> u8 {
        self.0[2]
    }
    pub const fn a(self) -> u8 {
        self.0[3]
    }

    /// Opacity in `0.0..=1.0`.
    pub fn opacity(self) -> f32 {
        self.0[3] as f32 / 255.0
    }

    /// The same color at `opacity` (`0.0..=1.0`).
    pub fn with_opacity(self, opacity: f32) -> Self {
        let a = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        Self([self.0[0], self.0[1], self.0[2], a])
    }

    /// The same opacity with the color of `rgb`.
    pub fn with_rgb_of(self, rgb: Rgba) -> Self {
        Self([rgb.0[0], rgb.0[1], rgb.0[2], self.0[3]])
    }

    /// The (premultiplied) color to draw with.
    pub const fn to_color32(self) -> Color32 {
        let [r, g, b, a] = self.0;
        Color32::from_rgba_unmultiplied_const(r, g, b, a)
    }

    pub fn from_color32(c: Color32) -> Self {
        Self(c.to_srgba_unmultiplied())
    }

    /// `#RRGGBBAA`.
    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.0;
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }

    /// Parses `#RRGGBB` or `#RRGGBBAA` (the `#` is optional).
    pub fn from_hex(s: &str) -> Option<Self> {
        let h = s.strip_prefix('#').unwrap_or(s);
        if !matches!(h.len(), 6 | 8) || !h.is_ascii() {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        let a = if h.len() == 8 { byte(6)? } else { 255 };
        Some(Self([byte(0)?, byte(2)?, byte(4)?, a]))
    }
}

impl From<Rgba> for Color32 {
    fn from(c: Rgba) -> Self {
        c.to_color32()
    }
}

impl From<Color32> for Rgba {
    fn from(c: Color32) -> Self {
        Self::from_color32(c)
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Hex(String),
            Array([u8; 4]),
        }
        match Repr::deserialize(d)? {
            Repr::Hex(s) => Rgba::from_hex(&s)
                .ok_or_else(|| serde::de::Error::custom(format!("invalid color {s:?}"))),
            Repr::Array(a) => Ok(Rgba(a)),
        }
    }
}

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
