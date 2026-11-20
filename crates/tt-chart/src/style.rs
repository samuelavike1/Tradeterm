use egui::Color32;

/// Colors and metrics for [`crate::ChartView`]. Defaults follow TradingView's dark theme.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartStyle {
    pub background: Color32,
    pub grid: Color32,
    pub axis_line: Color32,
    pub axis_text: Color32,
    pub text: Color32,
    pub up: Color32,
    pub down: Color32,
    pub crosshair: Color32,
    pub label_bg: Color32,
    pub label_text: Color32,
    /// Font size (points) of axis labels and the OHLC readout.
    pub font_size: f32,
    /// Height of the time axis strip at the bottom.
    pub time_axis_height: f32,
}

impl Default for ChartStyle {
    fn default() -> Self {
        Self {
            background: Color32::from_rgb(0x13, 0x17, 0x22),
            grid: Color32::from_rgb(0x1f, 0x23, 0x2e),
            axis_line: Color32::from_rgb(0x2a, 0x2e, 0x39),
            axis_text: Color32::from_rgb(0xb2, 0xb5, 0xbe),
            text: Color32::from_rgb(0xd1, 0xd4, 0xdc),
            up: Color32::from_rgb(0x26, 0xa6, 0x9a),
            down: Color32::from_rgb(0xef, 0x53, 0x50),
            crosshair: Color32::from_rgb(0x75, 0x79, 0x85),
            label_bg: Color32::from_rgb(0x36, 0x3a, 0x45),
            label_text: Color32::from_rgb(0xff, 0xff, 0xff),
            font_size: 11.5,
            time_axis_height: 24.0,
        }
    }
}
