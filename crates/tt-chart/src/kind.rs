use serde::{Deserialize, Serialize};

/// How bars are drawn (TradingView's chart types that work from OHLCV bars).
///
/// Variant names are stable: they are stored in settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ChartType {
    Bars,
    #[default]
    Candles,
    HollowCandles,
    VolumeCandles,
    Line,
    LineWithMarkers,
    StepLine,
    Area,
    HlcArea,
    Baseline,
    Columns,
    HighLow,
    HeikinAshi,
    Renko,
    LineBreak,
}

impl ChartType {
    /// Menu groups, in TradingView's order (separated by dividers).
    pub const GROUPS: [&'static [ChartType]; 5] = [
        &[
            ChartType::Bars,
            ChartType::Candles,
            ChartType::HollowCandles,
            ChartType::VolumeCandles,
        ],
        &[
            ChartType::Line,
            ChartType::LineWithMarkers,
            ChartType::StepLine,
        ],
        &[ChartType::Area, ChartType::HlcArea, ChartType::Baseline],
        &[ChartType::Columns, ChartType::HighLow],
        &[
            ChartType::HeikinAshi,
            ChartType::Renko,
            ChartType::LineBreak,
        ],
    ];

    pub const ALL: [ChartType; 15] = [
        ChartType::Bars,
        ChartType::Candles,
        ChartType::HollowCandles,
        ChartType::VolumeCandles,
        ChartType::Line,
        ChartType::LineWithMarkers,
        ChartType::StepLine,
        ChartType::Area,
        ChartType::HlcArea,
        ChartType::Baseline,
        ChartType::Columns,
        ChartType::HighLow,
        ChartType::HeikinAshi,
        ChartType::Renko,
        ChartType::LineBreak,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ChartType::Bars => "Bars",
            ChartType::Candles => "Candles",
            ChartType::HollowCandles => "Hollow candles",
            ChartType::VolumeCandles => "Volume candles",
            ChartType::Line => "Line",
            ChartType::LineWithMarkers => "Line with markers",
            ChartType::StepLine => "Step line",
            ChartType::Area => "Area",
            ChartType::HlcArea => "HLC area",
            ChartType::Baseline => "Baseline",
            ChartType::Columns => "Columns",
            ChartType::HighLow => "High-low",
            ChartType::HeikinAshi => "Heikin Ashi",
            ChartType::Renko => "Renko",
            ChartType::LineBreak => "Line break",
        }
    }

    /// One-line explanation (menu tooltip, like TradingView's "?" help).
    pub fn description(self) -> &'static str {
        match self {
            ChartType::Bars => "OHLC bars: open tick left, close tick right",
            ChartType::Candles => "Candlesticks colored by close vs open",
            ChartType::HollowCandles => {
                "Hollow when close > open; color by close vs previous close"
            }
            ChartType::VolumeCandles => "Candles whose width grows with volume",
            ChartType::Line => "Line through closes",
            ChartType::LineWithMarkers => "Line through closes with a dot on each bar",
            ChartType::StepLine => "Closes as horizontal steps",
            ChartType::Area => "Filled area under the close line",
            ChartType::HlcArea => "High, low and close lines with filled bands",
            ChartType::Baseline => {
                "Closes vs a base level of the visible range: green above, red below"
            }
            ChartType::Columns => "Columns from the bottom to each close",
            ChartType::HighLow => "Bars spanning each high-low range",
            ChartType::HeikinAshi => "Smoothed candles from averaged prices",
            ChartType::Renko => {
                "Fixed-size bricks (box by ATR, fixed size or percent); ignores time"
            }
            ChartType::LineBreak => {
                "Line break (three lines by default): new line only on breakouts"
            }
        }
    }

    /// Plots one value per bar, the type's price source (close, hl2, ...):
    /// price scaling uses only that value.
    pub fn uses_source_only(self) -> bool {
        matches!(
            self,
            ChartType::Line
                | ChartType::LineWithMarkers
                | ChartType::StepLine
                | ChartType::Area
                | ChartType::Baseline
                | ChartType::Columns
        )
    }

    /// Drawn as a line (also when decimated).
    pub fn is_line_like(self) -> bool {
        self.uses_source_only()
    }
}

/// Vertical price scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScaleMode {
    #[default]
    Linear,
    Logarithmic,
    /// Percent change relative to the first visible close.
    Percent,
    /// The first visible close = 100.
    IndexedTo100,
}

impl ScaleMode {
    pub const ALL: [ScaleMode; 4] = [
        ScaleMode::Linear,
        ScaleMode::Percent,
        ScaleMode::IndexedTo100,
        ScaleMode::Logarithmic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ScaleMode::Linear => "Linear",
            ScaleMode::Logarithmic => "Log",
            ScaleMode::Percent => "%",
            ScaleMode::IndexedTo100 => "100",
        }
    }

    /// Name in TradingView's price scale menu.
    pub fn menu_label(self) -> &'static str {
        match self {
            ScaleMode::Linear => "Regular",
            ScaleMode::Logarithmic => "Logarithmic",
            ScaleMode::Percent => "Percent",
            ScaleMode::IndexedTo100 => "Indexed to 100",
        }
    }
}
