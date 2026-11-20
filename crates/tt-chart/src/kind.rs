use serde::{Deserialize, Serialize};

/// How bars are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ChartType {
    #[default]
    Candles,
    HollowCandles,
    Bars,
    Line,
    Area,
    HeikinAshi,
}

impl ChartType {
    pub const ALL: [ChartType; 6] = [
        ChartType::Candles,
        ChartType::HollowCandles,
        ChartType::Bars,
        ChartType::Line,
        ChartType::Area,
        ChartType::HeikinAshi,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ChartType::Candles => "Candles",
            ChartType::HollowCandles => "Hollow candles",
            ChartType::Bars => "OHLC bars",
            ChartType::Line => "Line",
            ChartType::Area => "Area",
            ChartType::HeikinAshi => "Heikin Ashi",
        }
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
}

impl ScaleMode {
    pub const ALL: [ScaleMode; 3] = [
        ScaleMode::Linear,
        ScaleMode::Logarithmic,
        ScaleMode::Percent,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ScaleMode::Linear => "Linear",
            ScaleMode::Logarithmic => "Log",
            ScaleMode::Percent => "%",
        }
    }
}
