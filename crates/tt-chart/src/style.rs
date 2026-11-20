/// Non-color metrics for [`crate::ChartView`]. Colors come from [`tt_theme::Theme`].
/// Defaults follow TradingView's chart layout.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartStyle {
    /// Font size (points) of axis labels, tags and the OHLC readout.
    pub font_size: f32,
    /// Height of the time axis strip at the bottom.
    pub time_axis_height: f32,
    /// Target distance between price-axis labels / horizontal grid lines.
    pub price_label_spacing: f32,
    /// Target distance between time-axis labels / vertical grid lines.
    pub time_label_spacing: f32,
    /// Dotted (1 px on, 3 px off) instead of solid grid lines.
    pub dotted_grid: bool,
    /// Font family for emphasized labels (major time ticks). `None` = regular.
    pub bold_family: Option<egui::FontFamily>,
}

impl Default for ChartStyle {
    fn default() -> Self {
        Self {
            font_size: 12.5,
            time_axis_height: 28.0,
            price_label_spacing: 38.0,
            time_label_spacing: 80.0,
            dotted_grid: true,
            bold_family: None,
        }
    }
}
