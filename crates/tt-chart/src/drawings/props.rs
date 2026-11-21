//! Drawing style properties: typed [`Props`] per tool family, per-tool
//! defaults, the load-time overlay merge and the saved form of a drawing.
//!
//! Compatibility rules (see the drawings spec, section 6.3):
//! - Old drawings (`{tool, points, color, width, style?, locked?}`) migrate:
//!   the tool's defaults with the legacy line fields applied.
//! - Saved `props` are merged over the *tool's* defaults before they are
//!   read, so fields added later get the tool's default and unknown keys are
//!   ignored.
//! - The legacy `color`, `width` and `style` are still written (mirrored
//!   from the main line), so older builds keep reading new files.
//! - Enum variants are saved by name: add variants, never rename them.
//!
//! None of this runs per frame; rendering reads the typed props directly.

use super::model::{
    Anchor, BLUE, CYAN, Drawing, DrawingTool, GRAY, LineStyle, ORANGE, PINK, PURPLE, Placement,
    Rgba, Visibility, next_id,
};
use ecolor::Color32;
use serde::{Deserialize, Serialize};

/// A line: color, width (1 to 4 points) and style.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stroke {
    pub color: Rgba,
    pub width: u8,
    pub style: LineStyle,
}

impl Stroke {
    pub const fn new(color: Color32, width: u8, style: LineStyle) -> Self {
        Self {
            color: Rgba::opaque(color),
            width,
            style,
        }
    }

    pub const fn solid(color: Color32, width: u8) -> Self {
        Self::new(color, width, LineStyle::Solid)
    }
}

impl Default for Stroke {
    fn default() -> Self {
        Self::solid(BLUE, 2)
    }
}

/// A background: on/off and a color (with opacity).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fill {
    pub on: bool,
    pub color: Rgba,
}

impl Fill {
    pub fn new(on: bool, color: Color32, opacity: f32) -> Self {
        Self {
            on,
            color: Rgba::opaque(color).with_opacity(opacity),
        }
    }
}

impl Default for Fill {
    fn default() -> Self {
        Self::new(true, BLUE, 0.2)
    }
}

/// Text formatting (not the text itself).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyle {
    pub color: Rgba,
    pub size: u8,
    pub bold: bool,
    pub italic: bool,
}

impl TextStyle {
    pub const fn new(color: Color32, size: u8) -> Self {
        Self {
            color: Rgba::opaque(color),
            size,
            bold: false,
            italic: false,
        }
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::new(BLUE, 14)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HAlign {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VAlign {
    Top,
    Middle,
    #[default]
    Bottom,
}

/// Where a drawing's text goes.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TextLayout {
    /// Show the text.
    pub on: bool,
    pub h: HAlign,
    pub v: VAlign,
    /// Rotated along a vertical line (vertical line tool).
    pub vertical: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineEnd {
    #[default]
    Normal,
    Arrow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ends {
    pub left: LineEnd,
    pub right: LineEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Extend {
    pub left: bool,
    pub right: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StatsPosition {
    Left,
    Center,
    #[default]
    Right,
    Auto,
}

/// The "Stats" checklist of line and range tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub price_range: bool,
    pub percent: bool,
    pub pips: bool,
    pub bars: bool,
    pub date_time: bool,
    pub distance: bool,
    pub angle: bool,
    pub volume: bool,
    pub position: StatsPosition,
    /// Off: stats only while selected.
    pub always: bool,
}

/// A Fib or channel level.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Level {
    pub coeff: f64,
    pub color: Rgba,
    pub on: bool,
    /// Own line (channel levels); `None` = the drawing's line.
    pub stroke: Option<Stroke>,
}

impl Default for Level {
    fn default() -> Self {
        Self {
            coeff: 0.0,
            color: Rgba::opaque(GRAY),
            on: true,
            stroke: None,
        }
    }
}

/// Width and style shared by all levels (their colors are per level).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LevelsLine {
    pub width: u8,
    pub style: LineStyle,
}

impl Default for LevelsLine {
    fn default() -> Self {
        Self {
            width: 1,
            style: LineStyle::Solid,
        }
    }
}

/// Trend line, ray, extended line, info line, trend angle, arrow.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LineProps {
    pub line: Stroke,
    pub ends: Ends,
    pub extend: Extend,
    pub middle_point: bool,
    pub price_labels: bool,
    pub stats: Stats,
    pub text_style: TextStyle,
    pub text: TextLayout,
}

/// Horizontal line and horizontal ray.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HLineProps {
    pub line: Stroke,
    /// Tag on the price axis.
    pub price_label: bool,
    pub text_style: TextStyle,
    pub text: TextLayout,
}

impl Default for HLineProps {
    fn default() -> Self {
        Self {
            line: Stroke::default(),
            price_label: true,
            text_style: TextStyle::new(BLUE, 12),
            text: TextLayout {
                v: VAlign::Top,
                ..TextLayout::default()
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VLineProps {
    pub line: Stroke,
    /// Through the indicator panes too (off: price pane only).
    pub extend: bool,
    /// Tag on the time axis.
    pub time_label: bool,
    pub text_style: TextStyle,
    pub text: TextLayout,
}

impl Default for VLineProps {
    fn default() -> Self {
        Self {
            line: Stroke::default(),
            extend: true,
            time_label: true,
            text_style: TextStyle::default(),
            text: TextLayout {
                on: false,
                h: HAlign::Right,
                v: VAlign::Top,
                vertical: true,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrossProps {
    pub line: Stroke,
    pub price_label: bool,
    pub time_label: bool,
}

impl Default for CrossProps {
    fn default() -> Self {
        Self {
            line: Stroke::default(),
            price_label: true,
            time_label: true,
        }
    }
}

/// Parallel channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelProps {
    pub line: Stroke,
    pub levels: Vec<Level>,
    pub extend: Extend,
    pub background: Fill,
    pub text_style: TextStyle,
    pub text: TextLayout,
}

impl Default for ChannelProps {
    fn default() -> Self {
        let level = |coeff: f64, on: bool, stroke: Option<Stroke>| Level {
            coeff,
            color: Rgba::opaque(BLUE),
            on,
            stroke,
        };
        Self {
            line: Stroke::default(),
            levels: vec![
                level(0.0, true, None),
                level(0.5, true, Some(Stroke::new(BLUE, 1, LineStyle::Dashed))),
                level(1.0, true, None),
                level(-0.25, false, None),
                level(0.25, false, None),
                level(0.75, false, None),
                level(1.25, false, None),
            ],
            extend: Extend::default(),
            background: Fill::default(),
            text_style: TextStyle::default(),
            text: TextLayout::default(),
        }
    }
}

/// Fib retracement and trend-based Fib extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FibProps {
    pub trend_line_on: bool,
    pub trend_line: Stroke,
    pub levels_line: LevelsLine,
    /// TradingView's 24 level slots.
    pub levels: Vec<Level>,
    pub extend: Extend,
    pub background_on: bool,
    /// Band opacity (`0.0..=1.0`); each band uses its upper level's color.
    pub background_opacity: f32,
    pub reverse: bool,
    pub show_prices: bool,
    pub show_levels: bool,
    /// Levels as percents instead of values.
    pub percents: bool,
    pub label_h: HAlign,
    pub label_v: VAlign,
    pub font_size: u8,
    /// Interpolate levels in log space (when the chart is in log scale).
    pub log_scale: bool,
}

impl Default for FibProps {
    fn default() -> Self {
        let mut levels: Vec<Level> = super::model::FIB_LEVELS
            .iter()
            .map(|&(coeff, c)| Level {
                coeff,
                color: Rgba::opaque(c),
                on: true,
                stroke: None,
            })
            .collect();
        // The hidden slots (colors are a palette cycle until checked in TV).
        let hidden = [
            1.272, 1.414, 2.0, 2.272, 2.414, 3.0, 3.272, 3.414, 4.0, 4.272, 4.414, 4.618, 4.764,
        ];
        for (k, coeff) in hidden.into_iter().enumerate() {
            levels.push(Level {
                coeff,
                color: Rgba::opaque(super::model::PALETTE[k % 10]),
                on: false,
                stroke: None,
            });
        }
        Self {
            trend_line_on: true,
            trend_line: Stroke::new(GRAY, 1, LineStyle::Dashed),
            levels_line: LevelsLine::default(),
            levels,
            extend: Extend::default(),
            background_on: true,
            background_opacity: 0.2,
            reverse: false,
            show_prices: true,
            show_levels: true,
            percents: false,
            label_h: HAlign::Left,
            label_v: VAlign::Bottom,
            font_size: 12,
            log_scale: false,
        }
    }
}

/// Rectangle, circle, ellipse, polyline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShapeProps {
    pub border: Stroke,
    pub background: Fill,
    pub middle_line_on: bool,
    pub middle_line: Stroke,
    pub extend: Extend,
    pub text_style: TextStyle,
    pub text: TextLayout,
}

impl ShapeProps {
    fn colored(c: Color32) -> Self {
        Self {
            border: Stroke::solid(c, 2),
            background: Fill::new(true, c, 0.2),
            middle_line_on: false,
            middle_line: Stroke::new(c, 1, LineStyle::Dashed),
            extend: Extend::default(),
            text_style: TextStyle::new(c, 14),
            text: TextLayout {
                h: HAlign::Left,
                ..TextLayout::default()
            },
        }
    }
}

impl Default for ShapeProps {
    fn default() -> Self {
        Self::colored(PURPLE)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrushProps {
    pub line: Stroke,
    pub ends: Ends,
    /// Fills the closed shape.
    pub background: Fill,
}

impl Default for BrushProps {
    fn default() -> Self {
        Self {
            line: Stroke::solid(CYAN, 2),
            ends: Ends::default(),
            background: Fill::new(false, CYAN, 0.5),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PathProps {
    pub line: Stroke,
    pub ends: Ends,
}

impl Default for PathProps {
    fn default() -> Self {
        Self {
            line: Stroke::default(),
            ends: Ends {
                left: LineEnd::Normal,
                right: LineEnd::Arrow,
            },
        }
    }
}

/// Text, note, callout, price label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextBoxProps {
    pub text_style: TextStyle,
    pub background: Fill,
    pub border_on: bool,
    pub border: Stroke,
    /// The note's pin.
    pub marker: Rgba,
    pub wrap: bool,
    pub wrap_width: u16,
}

impl Default for TextBoxProps {
    fn default() -> Self {
        Self {
            text_style: TextStyle::default(),
            background: Fill {
                on: false,
                color: Rgba::new(91, 133, 191, 77),
            },
            border_on: false,
            border: Stroke {
                color: Rgba::new(102, 123, 139, 255),
                width: 1,
                style: LineStyle::Solid,
            },
            marker: Rgba::opaque(BLUE),
            wrap: false,
            wrap_width: 200,
        }
    }
}

/// Long and short position: inputs and style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PositionProps {
    pub account_size: f64,
    pub lot_size: f64,
    pub risk: f64,
    /// Risk in percent of the account (else in currency).
    pub risk_percent: bool,
    pub leverage: f64,
    /// Quantity decimals; `None` = the symbol's quantity step.
    pub qty_precision: Option<u8>,
    pub line: Stroke,
    pub stop_color: Rgba,
    pub target_color: Rgba,
    pub text_color: Rgba,
    pub font_size: u8,
    pub compact: bool,
    pub price_labels: bool,
    pub always_stats: bool,
}

impl Default for PositionProps {
    fn default() -> Self {
        Self {
            account_size: 1000.0,
            lot_size: 1.0,
            risk: 25.0,
            risk_percent: true,
            leverage: 1.0,
            qty_precision: None,
            line: Stroke::solid(GRAY, 1),
            stop_color: Rgba::new(242, 54, 69, 51),
            target_color: Rgba::new(8, 153, 129, 51),
            text_color: Rgba::opaque(Color32::WHITE),
            font_size: 12,
            compact: false,
            price_labels: true,
            always_stats: false,
        }
    }
}

/// Price range, date range, date and price range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RangeProps {
    pub line: Stroke,
    pub border_on: bool,
    pub border: Stroke,
    pub background: Fill,
    pub extend: Extend,
    pub stats: Stats,
    pub label: TextStyle,
    pub label_background: Fill,
    pub text_style: TextStyle,
    pub text: TextLayout,
    /// Hue used instead of the line / background / label colors (keeping
    /// their opacities) when a price-measuring range measures down.
    pub down_color: Rgba,
}

impl Default for RangeProps {
    fn default() -> Self {
        Self {
            line: Stroke::default(),
            border_on: false,
            border: Stroke {
                color: Rgba::opaque(BLUE).with_opacity(0.9),
                width: 1,
                style: LineStyle::Solid,
            },
            background: Fill::new(true, BLUE, 0.2),
            extend: Extend::default(),
            stats: Stats {
                price_range: true,
                percent: true,
                pips: true,
                bars: true,
                date_time: true,
                ..Stats::default()
            },
            label: TextStyle::new(Color32::WHITE, 12),
            label_background: Fill::new(true, BLUE, 0.9),
            text_style: TextStyle::default(),
            text: TextLayout::default(),
            down_color: Rgba::opaque(super::model::RED),
        }
    }
}

/// A drawing's style, one variant per tool family. Saved with a `kind` tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Props {
    Line(LineProps),
    HLine(HLineProps),
    VLine(VLineProps),
    Cross(CrossProps),
    Channel(ChannelProps),
    Fib(FibProps),
    Shape(ShapeProps),
    Brush(BrushProps),
    Path(PathProps),
    TextBox(TextBoxProps),
    Position(PositionProps),
    Range(RangeProps),
}

/// Which style controls a drawing has (decides the toolbar's buttons).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Caps(u8);

impl Caps {
    /// Line / border color.
    pub const LINE_COLOR: Caps = Caps(1);
    pub const LINE_WIDTH: Caps = Caps(2);
    pub const LINE_STYLE: Caps = Caps(4);
    /// Background color.
    pub const FILL: Caps = Caps(8);
    /// Text color / size.
    pub const TEXT: Caps = Caps(16);
    pub const LINE: Caps = Caps(1 | 2 | 4);

    pub const fn contains(self, other: Caps) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for Caps {
    type Output = Caps;
    fn bitor(self, o: Caps) -> Caps {
        Caps(self.0 | o.0)
    }
}

/// The controls two drawings have in common (multi-selection).
impl std::ops::BitAnd for Caps {
    type Output = Caps;
    fn bitand(self, o: Caps) -> Caps {
        Caps(self.0 & o.0)
    }
}

impl Props {
    /// The tool's factory style (TradingView's defaults).
    pub fn defaults(tool: DrawingTool) -> Props {
        use DrawingTool::*;
        match tool {
            TrendLine => Props::Line(LineProps::default()),
            Ray => Props::Line(LineProps {
                extend: Extend {
                    left: false,
                    right: true,
                },
                ..LineProps::default()
            }),
            ExtendedLine => Props::Line(LineProps {
                extend: Extend {
                    left: true,
                    right: true,
                },
                ..LineProps::default()
            }),
            InfoLine => Props::Line(LineProps {
                stats: Stats {
                    price_range: true,
                    percent: true,
                    pips: true,
                    bars: true,
                    date_time: true,
                    distance: true,
                    angle: true,
                    volume: false,
                    position: StatsPosition::Center,
                    always: true,
                },
                ..LineProps::default()
            }),
            TrendAngle => Props::Line(LineProps {
                text_style: TextStyle::new(BLUE, 12),
                ..LineProps::default()
            }),
            Arrow => Props::Line(LineProps {
                ends: Ends {
                    left: LineEnd::Normal,
                    right: LineEnd::Arrow,
                },
                ..LineProps::default()
            }),
            HorizontalLine | HorizontalRay => Props::HLine(HLineProps::default()),
            VerticalLine => Props::VLine(VLineProps::default()),
            CrossLine => Props::Cross(CrossProps::default()),
            ParallelChannel => Props::Channel(ChannelProps::default()),
            FibRetracement | FibExtension => Props::Fib(FibProps::default()),
            Rectangle => Props::Shape(ShapeProps::colored(PURPLE)),
            Ellipse => Props::Shape(ShapeProps::colored(PINK)),
            Circle => Props::Shape(ShapeProps::colored(ORANGE)),
            Polyline => Props::Shape(ShapeProps::colored(Color32::from_rgb(0x7e, 0x57, 0xc2))),
            Brush => Props::Brush(BrushProps::default()),
            Path => Props::Path(PathProps::default()),
            Text => Props::TextBox(TextBoxProps::default()),
            Note => Props::TextBox(TextBoxProps {
                text_style: TextStyle::new(Color32::WHITE, 14),
                background: Fill::new(true, BLUE, 0.7),
                border_on: true,
                border: Stroke::solid(BLUE, 1),
                ..TextBoxProps::default()
            }),
            Callout => {
                let c = Color32::from_rgb(0x00, 0x97, 0xa7);
                Props::TextBox(TextBoxProps {
                    text_style: TextStyle::new(Color32::WHITE, 14),
                    background: Fill::new(true, c, 0.7),
                    border_on: true,
                    border: Stroke::solid(c, 1),
                    ..TextBoxProps::default()
                })
            }
            PriceLabel => Props::TextBox(TextBoxProps {
                text_style: TextStyle {
                    bold: true,
                    ..TextStyle::new(Color32::WHITE, 14)
                },
                background: Fill::new(true, BLUE, 1.0),
                border_on: true,
                border: Stroke::solid(BLUE, 1),
                ..TextBoxProps::default()
            }),
            LongPosition | ShortPosition => Props::Position(PositionProps::default()),
            PriceRange => Props::Range(RangeProps {
                stats: Stats {
                    price_range: true,
                    percent: true,
                    pips: true,
                    ..Stats::default()
                },
                ..RangeProps::default()
            }),
            DateRange => Props::Range(RangeProps {
                stats: Stats {
                    bars: true,
                    date_time: true,
                    ..Stats::default()
                },
                ..RangeProps::default()
            }),
            DateAndPriceRange => Props::Range(RangeProps::default()),
        }
    }

    /// The `kind` tag this variant is saved with.
    pub fn kind(&self) -> &'static str {
        match self {
            Props::Line(_) => "Line",
            Props::HLine(_) => "HLine",
            Props::VLine(_) => "VLine",
            Props::Cross(_) => "Cross",
            Props::Channel(_) => "Channel",
            Props::Fib(_) => "Fib",
            Props::Shape(_) => "Shape",
            Props::Brush(_) => "Brush",
            Props::Path(_) => "Path",
            Props::TextBox(_) => "TextBox",
            Props::Position(_) => "Position",
            Props::Range(_) => "Range",
        }
    }

    /// Reads saved props (or a template) as an overlay on the tool's
    /// defaults: missing fields get the tool's default values, unknown keys
    /// are ignored. Fails on a different family or on unreadable values.
    pub fn overlay(tool: DrawingTool, saved: &serde_json::Value) -> Result<Props, String> {
        let defaults = Props::defaults(tool);
        if let Some(kind) = saved.get("kind").and_then(|k| k.as_str())
            && kind != defaults.kind()
        {
            return Err(format!(
                "props of kind {kind:?} for a {tool:?} (expected {:?})",
                defaults.kind()
            ));
        }
        let mut base = serde_json::to_value(&defaults).map_err(|e| e.to_string())?;
        merge(&mut base, saved);
        serde_json::from_value(base).map_err(|e| e.to_string())
    }

    /// Applies the style fields of drawings saved before `props` existed.
    pub(crate) fn apply_legacy(
        &mut self,
        color: Option<Color32>,
        width: Option<f32>,
        style: Option<LineStyle>,
    ) {
        if let Some(c) = color {
            // Legacy colors are premultiplied; ours were all opaque.
            let c = Rgba::from_color32(c);
            self.set_line_color(c);
            // Fills followed the line color before props existed: the
            // rectangle at 20 %, ranges keeping their opacities.
            match self {
                Props::Shape(p) => {
                    p.background = Fill {
                        on: true,
                        color: c.with_opacity(0.2),
                    }
                }
                Props::Range(p) => {
                    p.background.color = p.background.color.with_rgb_of(c);
                    p.label_background.color = p.label_background.color.with_rgb_of(c);
                }
                _ => {}
            }
        }
        if let Some(w) = width.filter(|w| w.is_finite()) {
            self.set_line_width(w.round().clamp(1.0, 4.0) as u8);
        }
        if let Some(s) = style {
            self.set_line_style(s);
        }
    }

    pub fn capabilities(&self) -> Caps {
        match self {
            Props::Cross(_) | Props::Fib(_) | Props::Path(_) | Props::Position(_) => Caps::LINE,
            Props::Line(_) | Props::HLine(_) | Props::VLine(_) => Caps::LINE | Caps::TEXT,
            Props::Channel(_) | Props::Shape(_) | Props::Range(_) => {
                Caps::LINE | Caps::FILL | Caps::TEXT
            }
            Props::Brush(_) => Caps::LINE | Caps::FILL,
            Props::TextBox(_) => Caps::TEXT | Caps::FILL,
        }
    }

    /// The main line (what the toolbar's line buttons edit). For the Fib it
    /// is the trend line; [`Self::line_width`] / [`Self::line_style`] are
    /// the levels' (as the toolbar edited them before props existed).
    fn main_stroke(&self) -> Option<&Stroke> {
        Some(match self {
            Props::Line(p) => &p.line,
            Props::HLine(p) => &p.line,
            Props::VLine(p) => &p.line,
            Props::Cross(p) => &p.line,
            Props::Channel(p) => &p.line,
            Props::Fib(p) => &p.trend_line,
            Props::Shape(p) => &p.border,
            Props::Brush(p) => &p.line,
            Props::Path(p) => &p.line,
            Props::Position(p) => &p.line,
            Props::Range(p) => &p.line,
            Props::TextBox(_) => return None,
        })
    }

    fn main_stroke_mut(&mut self) -> Option<&mut Stroke> {
        Some(match self {
            Props::Line(p) => &mut p.line,
            Props::HLine(p) => &mut p.line,
            Props::VLine(p) => &mut p.line,
            Props::Cross(p) => &mut p.line,
            Props::Channel(p) => &mut p.line,
            Props::Fib(p) => &mut p.trend_line,
            Props::Shape(p) => &mut p.border,
            Props::Brush(p) => &mut p.line,
            Props::Path(p) => &mut p.line,
            Props::Position(p) => &mut p.line,
            Props::Range(p) => &mut p.line,
            Props::TextBox(_) => return None,
        })
    }

    /// The main line as one stroke (color, width, style).
    pub fn primary_stroke(&self) -> Stroke {
        Stroke {
            color: self.line_color(),
            width: self.line_width(),
            style: self.line_style(),
        }
    }

    pub fn line_color(&self) -> Rgba {
        match self {
            Props::TextBox(p) => p.text_style.color,
            _ => self.main_stroke().map_or(Rgba::opaque(BLUE), |s| s.color),
        }
    }

    /// Sets the main line color (backgrounds have their own color, see
    /// [`Self::set_fill_color`]).
    pub fn set_line_color(&mut self, c: Rgba) {
        match self {
            Props::TextBox(p) => p.text_style.color = c,
            _ => {
                if let Some(s) = self.main_stroke_mut() {
                    s.color = c;
                }
            }
        }
    }

    /// The background (what the toolbar's background button edits).
    pub fn fill(&self) -> Option<&Fill> {
        match self {
            Props::Channel(p) => Some(&p.background),
            Props::Shape(p) => Some(&p.background),
            Props::Brush(p) => Some(&p.background),
            Props::TextBox(p) => Some(&p.background),
            Props::Range(p) => Some(&p.background),
            _ => None,
        }
    }

    /// Sets the background color (and turns the background on).
    pub fn set_fill_color(&mut self, c: Rgba) {
        if let Some(f) = self.fill_mut() {
            f.color = c;
            f.on = true;
        }
    }

    /// The text formatting (what the toolbar's text color button edits).
    pub fn text_style(&self) -> Option<&TextStyle> {
        match self {
            Props::Line(p) => Some(&p.text_style),
            Props::HLine(p) => Some(&p.text_style),
            Props::VLine(p) => Some(&p.text_style),
            Props::Channel(p) => Some(&p.text_style),
            Props::Shape(p) => Some(&p.text_style),
            Props::TextBox(p) => Some(&p.text_style),
            Props::Range(p) => Some(&p.text_style),
            Props::Cross(_) | Props::Fib(_) | Props::Brush(_) | Props::Path(_) => None,
            Props::Position(_) => None,
        }
    }

    pub fn set_text_color(&mut self, c: Rgba) {
        if let Some(t) = self.text_style_mut() {
            t.color = c;
        }
    }

    /// Whether the drawing's text is shown (families with a text layout;
    /// text boxes always show theirs).
    pub fn text_on(&self) -> bool {
        match self {
            Props::Line(p) => p.text.on,
            Props::HLine(p) => p.text.on,
            Props::VLine(p) => p.text.on,
            Props::Channel(p) => p.text.on,
            Props::Shape(p) => p.text.on,
            Props::Range(p) => p.text.on,
            Props::TextBox(_) => true,
            _ => false,
        }
    }

    /// Turns the text layout on or off (no-op for families without one).
    pub fn set_text_on(&mut self, on: bool) {
        match self {
            Props::Line(p) => p.text.on = on,
            Props::HLine(p) => p.text.on = on,
            Props::VLine(p) => p.text.on = on,
            Props::Channel(p) => p.text.on = on,
            Props::Shape(p) => p.text.on = on,
            Props::Range(p) => p.text.on = on,
            _ => {}
        }
    }

    pub fn line_width(&self) -> u8 {
        match self {
            Props::Fib(p) => p.levels_line.width,
            _ => self.main_stroke().map_or(1, |s| s.width),
        }
    }

    pub fn set_line_width(&mut self, w: u8) {
        let w = w.clamp(1, 4);
        match self {
            Props::Fib(p) => p.levels_line.width = w,
            _ => {
                if let Some(s) = self.main_stroke_mut() {
                    s.width = w;
                }
            }
        }
    }

    pub fn line_style(&self) -> LineStyle {
        match self {
            Props::Fib(p) => p.levels_line.style,
            _ => self.main_stroke().map_or(LineStyle::Solid, |s| s.style),
        }
    }

    pub fn set_line_style(&mut self, st: LineStyle) {
        match self {
            Props::Fib(p) => p.levels_line.style = st,
            _ => {
                if let Some(s) = self.main_stroke_mut() {
                    s.style = st;
                }
            }
        }
    }

    pub fn fill_mut(&mut self) -> Option<&mut Fill> {
        match self {
            Props::Channel(p) => Some(&mut p.background),
            Props::Shape(p) => Some(&mut p.background),
            Props::Brush(p) => Some(&mut p.background),
            Props::TextBox(p) => Some(&mut p.background),
            Props::Range(p) => Some(&mut p.background),
            _ => None,
        }
    }

    pub fn text_style_mut(&mut self) -> Option<&mut TextStyle> {
        match self {
            Props::Line(p) => Some(&mut p.text_style),
            Props::HLine(p) => Some(&mut p.text_style),
            Props::VLine(p) => Some(&mut p.text_style),
            Props::Channel(p) => Some(&mut p.text_style),
            Props::Shape(p) => Some(&mut p.text_style),
            Props::TextBox(p) => Some(&mut p.text_style),
            Props::Range(p) => Some(&mut p.text_style),
            Props::Cross(_) | Props::Fib(_) | Props::Brush(_) | Props::Path(_) => None,
            Props::Position(_) => None,
        }
    }
}

/// Recursive object merge: `over`'s keys replace `base`'s; nested objects
/// merge; arrays and scalars are replaced whole.
pub fn merge(base: &mut serde_json::Value, over: &serde_json::Value) {
    match (base, over) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(k) {
                    Some(slot) => merge(slot, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, o) => *b = o.clone(),
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// The saved form of a [`Drawing`]: `props` plus the legacy line fields
/// older builds require (`color` and `width`; `style` is optional there).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DrawingRepr {
    tool: DrawingTool,
    points: Vec<Anchor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    props: Option<serde_json::Value>,
    /// Legacy: the main line color (premultiplied `[r, g, b, a]`).
    #[serde(default)]
    color: Option<Color32>,
    /// Legacy: the main line width.
    #[serde(default)]
    width: Option<f32>,
    /// Legacy: the main line style.
    #[serde(default)]
    style: Option<LineStyle>,
    #[serde(default)]
    locked: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    text: String,
    #[serde(default, skip_serializing_if = "is_false")]
    hidden: bool,
    #[serde(default, skip_serializing_if = "Visibility::is_all")]
    visibility: Visibility,
}

impl TryFrom<DrawingRepr> for Drawing {
    type Error = String;

    fn try_from(r: DrawingRepr) -> Result<Self, String> {
        let placement = r.tool.placement();
        let n = r.points.len();
        let points_ok = match placement {
            Placement::Points(k) => n == k,
            Placement::OpenEnded | Placement::Freehand => n >= placement.min_points(),
        };
        if !points_ok {
            return Err(format!("{:?} with {n} points", r.tool));
        }
        if r.points
            .iter()
            .any(|a| !a.price.is_finite() || a.price.is_nan())
        {
            return Err(format!("{:?} with a non-finite price", r.tool));
        }
        let props = match &r.props {
            Some(v) => Props::overlay(r.tool, v)?,
            None => {
                let mut p = Props::defaults(r.tool);
                p.apply_legacy(r.color, r.width, r.style);
                p
            }
        };
        Ok(Drawing {
            id: next_id(),
            tool: r.tool,
            points: r.points,
            props,
            text: r.text,
            locked: r.locked,
            hidden: r.hidden,
            visibility: r.visibility,
        })
    }
}

impl From<Drawing> for DrawingRepr {
    fn from(d: Drawing) -> Self {
        let stroke = d.props.primary_stroke();
        DrawingRepr {
            tool: d.tool,
            points: d.points,
            props: serde_json::to_value(&d.props).ok(),
            color: Some(stroke.color.to_color32()),
            width: Some(stroke.width as f32),
            style: Some(stroke.style),
            locked: d.locked,
            text: d.text,
            hidden: d.hidden,
            visibility: d.visibility,
        }
    }
}
