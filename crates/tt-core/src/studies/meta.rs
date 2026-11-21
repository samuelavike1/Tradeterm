//! Declarative study metadata: inputs, plots, levels and fills.
//!
//! Each study file owns one `static META: StudyMeta`. The settings dialog,
//! the legend, defaults and the renderer read it instead of matching on
//! every [`Study`] variant. Everything here is `'static` data (no
//! allocation, no lookups beyond a short slice scan by id).

use super::{Placement, Source, Study, StudyKind};
use crate::Timeframe;
use serde::{Deserialize, Serialize};

/// `[r, g, b, a]`, unmultiplied sRGB (the same layout settings files use).
pub type Rgba = [u8; 4];

/// `#rrggbb` → [`Rgba`] at full opacity.
pub const fn rgb(hex: u32) -> Rgba {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255]
}

/// `#rrggbb` with an alpha byte.
pub const fn rgba(hex: u32, a: u8) -> Rgba {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, a]
}

/// How values are formatted on the axis and in the legend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueFormat {
    /// The symbol's price precision.
    Price,
    /// Abbreviated volume (`1.2K`, `3.4M`).
    Volume,
    /// A fixed number of decimals.
    Fixed(u8),
}

/// Widget and range of an input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputKind {
    Int {
        min: i64,
        max: i64,
        step: i64,
    },
    Float {
        min: f64,
        max: f64,
        step: f64,
    },
    Bool,
    /// Dropdown; the value is an index into the labels.
    Choice(&'static [&'static str]),
    /// Price source dropdown ([`Source::ALL`]).
    Source,
    /// Interval dropdown; `None` = the chart's interval.
    Timeframe,
    Color,
}

/// A value of an input, as read and written by [`Study::get`] / [`Study::set`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Choice(u16),
    Source(Source),
    Timeframe(Option<Timeframe>),
    Color(Rgba),
}

impl InputKind {
    /// `v` clamped to this input's range, or `None` if the types differ.
    pub fn clamp(&self, v: InputValue) -> Option<InputValue> {
        Some(match (*self, v) {
            (InputKind::Int { min, max, .. }, InputValue::Int(x)) => {
                InputValue::Int(x.clamp(min, max))
            }
            (InputKind::Float { min, max, .. }, InputValue::Float(x)) => {
                if !x.is_finite() {
                    return None;
                }
                InputValue::Float(x.clamp(min, max))
            }
            (InputKind::Bool, InputValue::Bool(_))
            | (InputKind::Source, InputValue::Source(_))
            | (InputKind::Timeframe, InputValue::Timeframe(_))
            | (InputKind::Color, InputValue::Color(_)) => v,
            (InputKind::Choice(opts), InputValue::Choice(i)) if (i as usize) < opts.len() => v,
            _ => return None,
        })
    }
}

/// One input of a study, in dialog order.
#[derive(Debug, Clone, Copy)]
pub struct InputDef {
    /// Stable id (`"length"`, `"src"`); never reuse or rename one.
    pub id: &'static str,
    /// Label in the settings dialog.
    pub title: &'static str,
    /// Small-caps group heading ("Smoothing", "Calculation").
    pub group: Option<&'static str>,
    /// Inputs with the same inline key share one row.
    pub inline: Option<&'static str>,
    pub tooltip: Option<&'static str>,
    pub kind: InputKind,
    /// Factory default (equal to `kind.default_study().get(id)`).
    pub default: InputValue,
    /// Shown after the title in the legend (Pine `display` flag).
    pub in_status_line: bool,
    /// Greyed out unless this holds (e.g. BB StdDev only for "SMA + BB").
    pub enabled_if: Option<fn(&Study) -> bool>,
}

impl InputDef {
    /// An input with no group, inline key, tooltip or condition.
    pub const fn new(
        id: &'static str,
        title: &'static str,
        kind: InputKind,
        default: InputValue,
        in_status_line: bool,
    ) -> Self {
        Self {
            id,
            title,
            group: None,
            inline: None,
            tooltip: None,
            kind,
            default,
            in_status_line,
            enabled_if: None,
        }
    }
}

/// What a plot means to the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotKind {
    /// A normal value series.
    Value,
    /// Columns from zero colored from the palette by sign and direction
    /// (MACD histogram: grow above, fall above, rise below, fall below).
    SignedColumns,
    /// Volume columns colored by bar direction (palette: falling, growing).
    VolumeColumns,
    /// Markers or labels (divergences).
    Shape,
}

/// How a plot is drawn (TradingView's plot types).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PlotType {
    #[default]
    Line,
    LineBr,
    Step,
    StepBr,
    StepDiamond,
    Histogram,
    Cross,
    Area,
    AreaBr,
    Columns,
    Circles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

/// Factory style of a plot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotDefault {
    /// The color (for palette plots: equal to the first palette entry).
    pub color: Rgba,
    pub width: u8,
    pub plot_type: PlotType,
    pub line_style: LineStyle,
    pub visible: bool,
}

impl PlotDefault {
    /// A visible 1 px solid line.
    pub const fn line(color: Rgba) -> Self {
        Self {
            color,
            width: 1,
            plot_type: PlotType::Line,
            line_style: LineStyle::Solid,
            visible: true,
        }
    }

    /// Visible columns.
    pub const fn columns(color: Rgba) -> Self {
        Self {
            plot_type: PlotType::Columns,
            ..Self::line(color)
        }
    }

    pub const fn hidden(self) -> Self {
        Self {
            visible: false,
            ..self
        }
    }
}

/// One output series. The order of `StudyMeta::plots` is the output order
/// of [`Study::compute`].
#[derive(Debug, Clone, Copy)]
pub struct PlotDef {
    /// Stable id, the key of per-plot styles in settings files.
    pub id: &'static str,
    pub title: &'static str,
    pub kind: PlotKind,
    pub default: PlotDefault,
    /// Named colors of multi-color plots (empty for single-color plots).
    pub palette: &'static [(&'static str, Rgba)],
    /// Greyed out and not drawn unless this holds (Pine `editable = …`).
    pub enabled_if: Option<fn(&Study) -> bool>,
}

impl PlotDef {
    /// A single-color value plot.
    pub const fn line(id: &'static str, title: &'static str, color: Rgba) -> Self {
        Self {
            id,
            title,
            kind: PlotKind::Value,
            default: PlotDefault::line(color),
            palette: &[],
            enabled_if: None,
        }
    }
}

/// A horizontal guide level (`hline`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HLineDef {
    pub id: &'static str,
    pub title: &'static str,
    pub value: f64,
    pub color: Rgba,
    pub width: u8,
    pub style: LineStyle,
}

/// One edge of a fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillEdge {
    /// A [`PlotDef`] by id.
    Plot(&'static str),
    /// A [`HLineDef`] by id.
    HLine(&'static str),
}

/// A filled band between two plots and/or levels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FillDef {
    pub id: &'static str,
    pub title: &'static str,
    pub between: (FillEdge, FillEdge),
    /// Opacity is in the alpha byte.
    pub color: Rgba,
    /// Second color of a gradient fill.
    pub color2: Option<Rgba>,
    pub visible: bool,
}

/// Everything static about a study kind.
#[derive(Debug)]
pub struct StudyMeta {
    pub kind: StudyKind,
    /// Full name ("Relative Strength Index").
    pub name: &'static str,
    /// Legend and dialog title ("RSI").
    pub short: &'static str,
    /// Default placement.
    pub placement: Placement,
    pub format: ValueFormat,
    /// Has the Calculation (timeframe) group.
    pub mtf: bool,
    pub inputs: &'static [InputDef],
    pub plots: &'static [PlotDef],
    pub hlines: &'static [HLineDef],
    pub fills: &'static [FillDef],
    /// Fixed scale hint (RSI 0..100).
    pub bounds: Option<(f64, f64)>,
}

impl StudyMeta {
    pub fn input(&self, id: &str) -> Option<&'static InputDef> {
        self.inputs.iter().find(|d| d.id == id)
    }

    pub fn plot(&self, id: &str) -> Option<&'static PlotDef> {
        self.plots.iter().find(|d| d.id == id)
    }

    /// Output index of a plot id.
    pub fn plot_index(&self, id: &str) -> Option<usize> {
        self.plots.iter().position(|d| d.id == id)
    }

    pub fn hline(&self, id: &str) -> Option<&'static HLineDef> {
        self.hlines.iter().find(|d| d.id == id)
    }

    pub fn fill(&self, id: &str) -> Option<&'static FillDef> {
        self.fills.iter().find(|d| d.id == id)
    }
}

// Shared input definitions.

/// A length input (1..=2000), shown in the status line.
pub const fn length(id: &'static str, title: &'static str, default: i64) -> InputDef {
    InputDef::new(
        id,
        title,
        InputKind::Int {
            min: 1,
            max: 2000,
            step: 1,
        },
        InputValue::Int(default),
        true,
    )
}

/// A smoothing length input (1..=500), shown in the status line.
pub const fn short_length(id: &'static str, title: &'static str, default: i64) -> InputDef {
    InputDef {
        kind: InputKind::Int {
            min: 1,
            max: 500,
            step: 1,
        },
        ..length(id, title, default)
    }
}

/// The "Source" input (id `src`), default close, shown in the status line.
pub const SOURCE: InputDef = InputDef::new(
    "src",
    "Source",
    InputKind::Source,
    InputValue::Source(Source::Close),
    true,
);

/// Guide-level color (TradingView `#787B86`; ours drew `theme.text_muted`).
pub const LEVEL_GRAY: u32 = 0x787b86;

const fn outer_level(id: &'static str, title: &'static str, value: f64) -> HLineDef {
    HLineDef {
        id,
        title,
        value,
        color: rgba(LEVEL_GRAY, 153),
        width: 1,
        style: LineStyle::Dashed,
    }
}

/// Upper / middle (50) / lower levels (ids `upper`, `middle`, `lower`) as
/// drawn today: outer dashed at 60 % alpha, middle dotted at 35 % alpha.
pub const fn oscillator_levels(titles: [&'static str; 3], upper: f64, lower: f64) -> [HLineDef; 3] {
    [
        outer_level("upper", titles[0], upper),
        HLineDef {
            id: "middle",
            title: titles[1],
            value: 50.0,
            color: rgba(LEVEL_GRAY, 89),
            width: 1,
            style: LineStyle::Dotted,
        },
        outer_level("lower", titles[2], lower),
    ]
}
