//! Symbol tab: one style per chart type (TradingView keeps each type's
//! values, so switching type and back keeps its colors), plus "Data
//! modification".
//!
//! Every color is a [`ColorSlot`]: `None` follows the theme token named in
//! the field's doc comment.

use super::types::*;
use crate::ChartType;
use serde::{Deserialize, Serialize};

/// Candles, Hollow candles, Volume candles and Heikin Ashi (separate
/// instances).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CandleStyle {
    /// Bars are "up" when the close is above the previous close (not the
    /// open). Hollow candles ignore it (the type defines its coloring).
    pub color_by_prev_close: bool,
    pub body: bool,
    /// theme `up`.
    pub body_up: ColorSlot,
    /// theme `down`.
    pub body_down: ColorSlot,
    pub borders: bool,
    /// The body up color.
    pub border_up: ColorSlot,
    /// The body down color.
    pub border_down: ColorSlot,
    pub wick: bool,
    /// theme `up_wick`.
    pub wick_up: ColorSlot,
    /// theme `down_wick`.
    pub wick_down: ColorSlot,
}

impl Default for CandleStyle {
    fn default() -> Self {
        Self {
            color_by_prev_close: false,
            body: true,
            body_up: None,
            body_down: None,
            borders: true,
            border_up: None,
            border_down: None,
            wick: true,
            wick_up: None,
            wick_down: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BarStyle {
    pub color_by_prev_close: bool,
    /// HLC bars: no open tick.
    pub hlc: bool,
    /// 1 px bars regardless of zoom.
    pub thin: bool,
    /// theme `up`.
    pub up: ColorSlot,
    /// theme `down`.
    pub down: ColorSlot,
}

impl Default for BarStyle {
    fn default() -> Self {
        Self {
            color_by_prev_close: false,
            hlc: false,
            thin: true,
            up: None,
            down: None,
        }
    }
}

/// Line, Line with markers and Step line (separate instances).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineSeriesStyle {
    pub source: PriceSource,
    /// theme `series_palette[0]`.
    pub color: ColorSlot,
    pub width: f32,
    pub kind: LineKind,
}

impl Default for LineSeriesStyle {
    fn default() -> Self {
        Self {
            source: PriceSource::Close,
            color: None,
            width: 2.0,
            kind: LineKind::Solid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AreaStyle {
    pub source: PriceSource,
    /// theme `series_palette[0]`.
    pub line: ColorSlot,
    pub width: f32,
    pub kind: LineKind,
    /// The line color at 28 % opacity.
    pub fill_top: ColorSlot,
    /// The line color, fully transparent.
    pub fill_bottom: ColorSlot,
}

impl Default for AreaStyle {
    fn default() -> Self {
        Self {
            source: PriceSource::Close,
            line: None,
            width: 2.0,
            kind: LineKind::Solid,
            fill_top: None,
            fill_bottom: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HlcAreaStyle {
    /// Color: theme `up`.
    pub high: StrokeStyle,
    /// Color: theme `down`.
    pub low: StrokeStyle,
    /// Color: theme `text_muted`.
    pub close: StrokeStyle,
    /// theme `up` at 20 %.
    pub fill_high_close: ColorSlot,
    /// theme `down` at 20 %.
    pub fill_close_low: ColorSlot,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BaselineStyle {
    pub source: PriceSource,
    /// Base level in percent of the visible price range, from the bottom.
    pub level_pct: f32,
    /// theme `text_muted`.
    pub base_color: ColorSlot,
    /// theme `up`.
    pub top_line: ColorSlot,
    pub top_width: f32,
    /// theme `down`.
    pub bottom_line: ColorSlot,
    pub bottom_width: f32,
    /// Near the line: theme `up` at 28 %.
    pub top_fill_1: ColorSlot,
    /// Near the base: theme `up` at 5 %.
    pub top_fill_2: ColorSlot,
    /// Near the base: theme `down` at 5 %.
    pub bottom_fill_1: ColorSlot,
    /// Near the line: theme `down` at 28 %.
    pub bottom_fill_2: ColorSlot,
}

impl Default for BaselineStyle {
    fn default() -> Self {
        Self {
            source: PriceSource::Close,
            level_pct: 50.0,
            base_color: None,
            top_line: None,
            top_width: 2.0,
            bottom_line: None,
            bottom_width: 2.0,
            top_fill_1: None,
            top_fill_2: None,
            bottom_fill_1: None,
            bottom_fill_2: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnStyle {
    pub source: PriceSource,
    pub color_by_prev_close: bool,
    /// theme `up` at 50 %.
    pub up: ColorSlot,
    /// theme `down` at 50 %.
    pub down: ColorSlot,
}

impl Default for ColumnStyle {
    fn default() -> Self {
        Self {
            source: PriceSource::Close,
            color_by_prev_close: true,
            up: None,
            down: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HighLowStyle {
    /// theme `series_palette[0]`.
    pub body: ColorSlot,
    pub borders: bool,
    /// theme `series_palette[0]`.
    pub border: ColorSlot,
    /// High / low value labels at each bar.
    pub labels: bool,
    /// theme `series_palette[0]`.
    pub label: ColorSlot,
}

impl Default for HighLowStyle {
    fn default() -> Self {
        Self {
            body: None,
            borders: true,
            border: None,
            labels: true,
            label: None,
        }
    }
}

/// Brick colors (Renko, Line break). Projection = the forming brick.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BrickColors {
    /// theme `up`.
    pub up: ColorSlot,
    /// theme `down`.
    pub down: ColorSlot,
    /// The up color.
    pub border_up: ColorSlot,
    /// The down color.
    pub border_down: ColorSlot,
    /// The up color mixed 60 % into the background.
    pub proj_up: ColorSlot,
    /// The down color mixed 60 % into the background.
    pub proj_down: ColorSlot,
    /// The projection up color.
    pub proj_border_up: ColorSlot,
    /// The projection down color.
    pub proj_border_down: ColorSlot,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RenkoInputs {
    pub source: BrickSource,
    pub method: BoxMethod,
    /// ATR method.
    pub atr_length: u32,
    /// Traditional method, in price units (rounded to the tick).
    pub box_size: f64,
    /// Percentage (LTP) method: percent of the last close.
    pub percent: f64,
    pub wicks: bool,
}

impl Default for RenkoInputs {
    fn default() -> Self {
        Self {
            source: BrickSource::Close,
            method: BoxMethod::Atr,
            atr_length: 14,
            box_size: 3.0,
            percent: 1.0,
            wicks: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RenkoStyle {
    pub colors: BrickColors,
    /// theme `up_wick`.
    pub wick_up: ColorSlot,
    /// theme `down_wick`.
    pub wick_down: ColorSlot,
    pub inputs: RenkoInputs,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineBreakStyle {
    pub colors: BrickColors,
    /// Number of lines a reversal must exceed.
    pub lines: u32,
}

impl Default for LineBreakStyle {
    fn default() -> Self {
        Self {
            colors: BrickColors::default(),
            lines: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KagiInputs {
    pub source: PriceSource,
    pub method: BoxMethod,
    pub atr_length: u32,
    /// Traditional method, in price units.
    pub reversal: f64,
    /// Percentage (LTP) method.
    pub percent: f64,
}

impl Default for KagiInputs {
    fn default() -> Self {
        Self {
            source: PriceSource::Close,
            method: BoxMethod::Atr,
            atr_length: 14,
            reversal: 1.0,
            percent: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KagiStyle {
    /// Yang line: theme `up`.
    pub up: ColorSlot,
    /// Yin line: theme `down`.
    pub down: ColorSlot,
    pub proj_up: ColorSlot,
    pub proj_down: ColorSlot,
    pub inputs: KagiInputs,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PnfInputs {
    pub source: PnfSource,
    pub method: BoxMethod,
    pub atr_length: u32,
    /// Traditional method, in price units.
    pub box_size: f64,
    /// Percentage (LTP) method.
    pub percent: f64,
    /// Reversal amount, in boxes.
    pub reversal: u32,
    /// Requires a reversal of 1.
    pub one_step_back: bool,
}

impl Default for PnfInputs {
    fn default() -> Self {
        Self {
            source: PnfSource::Close,
            method: BoxMethod::Atr,
            atr_length: 14,
            box_size: 1.0,
            percent: 1.0,
            reversal: 3,
            one_step_back: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PnfStyle {
    /// X columns: theme `up`.
    pub up: ColorSlot,
    /// O columns: theme `down`.
    pub down: ColorSlot,
    pub proj_up: ColorSlot,
    pub proj_down: ColorSlot,
    pub inputs: PnfInputs,
}

/// Symbol › Data modification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DataModification {
    /// Hidden for crypto (24/7).
    pub session: Session,
    pub precision: Precision,
    pub time_zone: TimeZoneSetting,
}

/// The Symbol tab: every chart type's own style.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SymbolStyle {
    pub candles: CandleStyle,
    pub hollow_candles: CandleStyle,
    pub volume_candles: CandleStyle,
    pub heikin_ashi: CandleStyle,
    pub bars: BarStyle,
    pub line: LineSeriesStyle,
    pub line_with_markers: LineSeriesStyle,
    pub step_line: LineSeriesStyle,
    pub area: AreaStyle,
    pub hlc_area: HlcAreaStyle,
    pub baseline: BaselineStyle,
    pub columns: ColumnStyle,
    pub high_low: HighLowStyle,
    pub renko: RenkoStyle,
    pub line_break: LineBreakStyle,
    pub kagi: KagiStyle,
    pub pnf: PnfStyle,
    pub data: DataModification,
}

impl SymbolStyle {
    /// The candle style of a candle-like type (Candles for any other type).
    pub fn candle(&self, t: ChartType) -> &CandleStyle {
        match t {
            ChartType::HollowCandles => &self.hollow_candles,
            ChartType::VolumeCandles => &self.volume_candles,
            ChartType::HeikinAshi => &self.heikin_ashi,
            _ => &self.candles,
        }
    }

    pub fn candle_mut(&mut self, t: ChartType) -> &mut CandleStyle {
        match t {
            ChartType::HollowCandles => &mut self.hollow_candles,
            ChartType::VolumeCandles => &mut self.volume_candles,
            ChartType::HeikinAshi => &mut self.heikin_ashi,
            _ => &mut self.candles,
        }
    }

    /// The line style of a line type (Line for any other type).
    pub fn line(&self, t: ChartType) -> &LineSeriesStyle {
        match t {
            ChartType::LineWithMarkers => &self.line_with_markers,
            ChartType::StepLine => &self.step_line,
            _ => &self.line,
        }
    }

    pub fn line_mut(&mut self, t: ChartType) -> &mut LineSeriesStyle {
        match t {
            ChartType::LineWithMarkers => &mut self.line_with_markers,
            ChartType::StepLine => &mut self.step_line,
            _ => &mut self.line,
        }
    }

    /// "Color bars based on previous close" for the types that have it.
    pub fn color_by_prev_close(&self, t: ChartType) -> bool {
        match t {
            ChartType::Bars => self.bars.color_by_prev_close,
            ChartType::Columns => self.columns.color_by_prev_close,
            ChartType::Candles | ChartType::HeikinAshi | ChartType::VolumeCandles => {
                self.candle(t).color_by_prev_close
            }
            _ => false,
        }
    }
}
