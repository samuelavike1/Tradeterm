/// Non-color metrics for [`crate::ChartView`]. Colors come from [`tt_theme::Theme`].
#[derive(Debug, Clone, PartialEq)]
pub struct ChartStyle {
    /// Font size (points) of axis labels and the OHLC readout.
    pub font_size: f32,
    /// Height of the time axis strip at the bottom.
    pub time_axis_height: f32,
}

impl Default for ChartStyle {
    fn default() -> Self {
        Self {
            font_size: 11.5,
            time_axis_height: 24.0,
        }
    }
}
