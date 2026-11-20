use ecolor::Color32;
use serde::{Deserialize, Serialize};
use tt_theme::Theme;

/// When the A / L / % scale buttons at the bottom of the price axis show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScaleButtons {
    /// While the pointer is over the price axis (TradingView's default).
    #[default]
    OnHover,
    Always,
    Hidden,
}

impl ScaleButtons {
    pub const ALL: [ScaleButtons; 3] = [
        ScaleButtons::OnHover,
        ScaleButtons::Always,
        ScaleButtons::Hidden,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ScaleButtons::OnHover => "Visible on mouse over",
            ScaleButtons::Always => "Always visible",
            ScaleButtons::Hidden => "Hidden",
        }
    }
}

/// User color overrides on top of the theme (`None` = theme color).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChartColors {
    pub up_body: Option<Color32>,
    pub down_body: Option<Color32>,
    pub up_border: Option<Color32>,
    pub down_border: Option<Color32>,
    pub up_wick: Option<Color32>,
    pub down_wick: Option<Color32>,
    pub background: Option<Color32>,
    pub grid_vertical: Option<Color32>,
    pub grid_horizontal: Option<Color32>,
    pub crosshair: Option<Color32>,
    pub scale_text: Option<Color32>,
    pub scale_lines: Option<Color32>,
}

/// Chart appearance and behaviour, TradingView's "Settings" dialog.
/// Defaults follow TradingView's chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChartStyle {
    /// Font size (points) of axis labels, tags and the readout ("Scales: Text").
    pub font_size: f32,
    /// Height of the time axis strip at the bottom.
    #[serde(skip)]
    pub time_axis_height: f32,
    /// Target distance between price-axis labels / horizontal grid lines.
    #[serde(skip)]
    pub price_label_spacing: f32,
    /// Target distance between time-axis labels / vertical grid lines.
    #[serde(skip)]
    pub time_label_spacing: f32,
    /// Dotted instead of solid grid lines.
    pub dotted_grid: bool,

    // ---- Symbol -----------------------------------------------------------
    /// Bars are "up" when the close is above the previous close (not the open).
    pub color_by_prev_close: bool,
    pub body: bool,
    pub borders: bool,
    pub wick: bool,

    // ---- Status line --------------------------------------------------------
    /// Symbol · timeframe · source.
    pub status_title: bool,
    /// O / H / L / C.
    pub status_values: bool,
    /// Change and change %.
    pub status_change: bool,
    pub status_volume: bool,
    pub legend_titles: bool,
    pub legend_inputs: bool,
    pub legend_values: bool,

    // ---- Scales and lines ---------------------------------------------------
    pub scale_buttons: ScaleButtons,
    pub no_overlapping_labels: bool,
    pub countdown: bool,
    pub last_price_label: bool,
    pub last_price_line: bool,
    pub indicator_value_labels: bool,
    /// Weekday in the crosshair's date label ("Fri 20 Nov '26").
    pub day_of_week: bool,

    // ---- Canvas -------------------------------------------------------------
    pub grid_vertical: bool,
    pub grid_horizontal: bool,
    /// Empty space above / below the bars, in percent of the price pane.
    pub margin_top: f32,
    pub margin_bottom: f32,
    /// Empty space right of the newest bar after a reset, in bars.
    pub margin_right: f32,
    pub colors: ChartColors,
}

impl Default for ChartStyle {
    fn default() -> Self {
        Self {
            font_size: 12.5,
            time_axis_height: 28.0,
            price_label_spacing: 38.0,
            time_label_spacing: 80.0,
            dotted_grid: true,
            color_by_prev_close: false,
            body: true,
            borders: true,
            wick: true,
            status_title: true,
            status_values: true,
            status_change: true,
            status_volume: false,
            legend_titles: true,
            legend_inputs: true,
            legend_values: true,
            scale_buttons: ScaleButtons::OnHover,
            no_overlapping_labels: true,
            countdown: true,
            last_price_label: true,
            last_price_line: true,
            indicator_value_labels: true,
            day_of_week: true,
            grid_vertical: true,
            grid_horizontal: true,
            margin_top: 8.0,
            margin_bottom: 8.0,
            margin_right: 5.0,
            colors: ChartColors::default(),
        }
    }
}

impl ChartStyle {
    /// `base` with this style's color overrides applied.
    pub fn theme(&self, base: &Theme) -> Theme {
        let c = &self.colors;
        let mut t = base.clone();
        if let Some(v) = c.up_body {
            t.up = v;
            t.volume_up = tt_theme::with_alpha(v, t.volume_up.a() as f32 / 255.0);
        }
        if let Some(v) = c.down_body {
            t.down = v;
            t.volume_down = tt_theme::with_alpha(v, t.volume_down.a() as f32 / 255.0);
        }
        if let Some(v) = c.up_wick {
            t.up_wick = v;
        }
        if let Some(v) = c.down_wick {
            t.down_wick = v;
        }
        if let Some(v) = c.background {
            t.bg = v;
        }
        if let Some(v) = c.crosshair {
            t.crosshair = v;
        }
        if let Some(v) = c.scale_text {
            t.text_muted = v;
        }
        if let Some(v) = c.scale_lines {
            t.border = v;
        }
        t
    }

    /// Grid color for vertical (time) or horizontal (price) lines.
    pub fn grid_color(&self, theme: &Theme, vertical: bool) -> Color32 {
        let o = if vertical {
            self.colors.grid_vertical
        } else {
            self.colors.grid_horizontal
        };
        o.unwrap_or(theme.grid)
    }

    /// Candle border color (defaults to the body color).
    pub fn border_color(&self, theme: &Theme, up: bool) -> Color32 {
        let o = if up {
            self.colors.up_border
        } else {
            self.colors.down_border
        };
        o.unwrap_or(if up { theme.up } else { theme.down })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_apply_and_serde_round_trips() {
        let mut s = ChartStyle::default();
        s.colors.up_body = Some(Color32::from_rgb(1, 2, 3));
        s.colors.background = Some(Color32::from_rgb(9, 9, 9));
        s.status_volume = true;
        let t = s.theme(&Theme::dark());
        assert_eq!(t.up, Color32::from_rgb(1, 2, 3));
        assert_eq!(t.bg, Color32::from_rgb(9, 9, 9));
        assert_eq!(t.down, Theme::dark().down);
        assert_eq!(
            s.border_color(&t, true),
            Color32::from_rgb(1, 2, 3),
            "borders follow the body"
        );
        let json = serde_json::to_string(&s).unwrap();
        let back: ChartStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        // Old / partial settings get defaults.
        let partial: ChartStyle = serde_json::from_str(r#"{"countdown":false}"#).unwrap();
        assert!(!partial.countdown && partial.last_price_line);
    }
}
