//! Chart settings: TradingView's chart "Settings" dialog, one struct per tab.
//!
//! Format v2 (nested per tab and per chart type). v1 files (one flat struct)
//! are read through [`legacy`] and migrated when they load, so they keep their
//! look; v1 keys are never written back.
//!
//! Rules: every struct is `#[serde(default)]` (partial / old files load,
//! unknown keys are ignored); colors that follow dark / light are
//! [`ColorSlot`]s (`None` = theme). Renderers read the style by reference,
//! never cloning it per frame.

mod legacy;
mod series;
mod types;

pub use legacy::{SharedCandleColor, SharedCandleFlag};
pub use series::*;
pub use types::*;

use ecolor::Color32;
use serde::{Deserialize, Serialize};
use tt_theme::Theme;

/// Current [`ChartStyle::version`].
pub const STYLE_VERSION: u32 = 2;

/// Chart appearance and behaviour. Defaults follow TradingView's chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "legacy::ChartStyleWire")]
pub struct ChartStyle {
    /// Format version ([`STYLE_VERSION`] once loaded).
    pub version: u32,
    pub symbol: SymbolStyle,
    pub status_line: StatusLineStyle,
    pub scales: ScalesStyle,
    pub canvas: CanvasStyle,
    /// Model only: needs a broker.
    pub trading: TradingStyle,
    /// Model only: needs an alerts engine.
    pub alerts: AlertsStyle,
    pub events: EventsStyle,
    /// Height of the time axis strip at the bottom.
    #[serde(skip)]
    pub time_axis_height: f32,
    /// Target distance between price-axis labels / horizontal grid lines.
    #[serde(skip)]
    pub price_label_spacing: f32,
    /// Target distance between time-axis labels / vertical grid lines.
    #[serde(skip)]
    pub time_label_spacing: f32,
}

impl Default for ChartStyle {
    fn default() -> Self {
        Self {
            version: STYLE_VERSION,
            symbol: SymbolStyle::default(),
            status_line: StatusLineStyle::default(),
            scales: ScalesStyle::default(),
            canvas: CanvasStyle::default(),
            trading: TradingStyle::default(),
            alerts: AlertsStyle::default(),
            events: EventsStyle::default(),
            time_axis_height: 28.0,
            price_label_spacing: 38.0,
            time_label_spacing: 80.0,
        }
    }
}

// ---- Status line -----------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusLineStyle {
    pub logo: bool,
    /// Symbol · timeframe · source.
    pub title: bool,
    pub title_format: TitleFormat,
    pub show_interval: bool,
    pub show_exchange: bool,
    pub market_status: bool,
    /// O / H / L / C.
    pub chart_values: bool,
    /// Change and change %.
    pub bar_change: bool,
    pub last_day_change: bool,
    pub volume: bool,
    pub indicator_titles: bool,
    pub indicator_inputs: bool,
    pub indicator_values: bool,
    pub indicator_background: bool,
    /// Transparency of the indicator legend background, 0–100 %.
    pub indicator_background_transparency: u8,
}

impl Default for StatusLineStyle {
    fn default() -> Self {
        Self {
            logo: true,
            title: true,
            title_format: TitleFormat::Ticker,
            show_interval: true,
            show_exchange: true,
            market_status: true,
            chart_values: true,
            bar_change: true,
            last_day_change: false,
            volume: false,
            indicator_titles: true,
            indicator_inputs: true,
            indicator_values: true,
            indicator_background: true,
            indicator_background_transparency: 50,
        }
    }
}

// ---- Scales and lines ------------------------------------------------------

/// A price-scale label and / or line (previous close, high / low, …).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineLabel {
    pub label: bool,
    pub line: bool,
    /// None = the field's default (see its doc comment).
    pub color: ColorSlot,
    pub width: f32,
}

impl Default for LineLabel {
    fn default() -> Self {
        Self {
            label: false,
            line: false,
            color: None,
            width: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BidAskStyle {
    pub label: bool,
    pub line: bool,
    /// TradingView blue #2962FF.
    pub bid: ColorSlot,
    /// TradingView red #F7525F.
    pub ask: ColorSlot,
    pub kind: LineKind,
    pub width: f32,
}

impl Default for BidAskStyle {
    fn default() -> Self {
        Self {
            label: false,
            line: false,
            bid: None,
            ask: None,
            kind: LineKind::Dotted,
            width: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrePostStyle {
    pub label: bool,
    pub line: bool,
    pub pre: ColorSlot,
    pub post: ColorSlot,
}

impl Default for PrePostStyle {
    fn default() -> Self {
        Self {
            label: true,
            line: true,
            pre: None,
            post: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScalesStyle {
    /// Font size (points) of axis labels, tags and the readout (Canvas ›
    /// Scales › Text).
    pub text_size: f32,
    /// Currency and unit label on the price scale.
    pub currency_label: Visibility3,
    /// A / L / % buttons at the bottom of the price axis.
    pub scale_buttons: Visibility3,
    /// Lock price to bar ratio: price units per bar (None = off).
    pub lock_ratio: Option<f64>,
    pub placement: ScalePlacement,
    pub no_overlapping_labels: bool,
    pub plus_button: bool,
    pub countdown: bool,
    /// Symbol last value: label and / or price line.
    pub symbol_label: LabelMode,
    pub symbol_value_mode: LastValueMode,
    pub price_line_width: f32,
    /// None = the last bar's color.
    pub price_line_color: ColorSlot,
    /// Previous day close. Color None = #555555.
    pub prev_close: LineLabel,
    /// Indicator value / name labels.
    pub indicators: LabelMode,
    /// High and low of the visible range. Color None = theme `text_muted`.
    pub high_low: LineLabel,
    /// Average close price. Color None = theme `text_muted`.
    pub avg_close: LineLabel,
    pub bid_ask: BidAskStyle,
    /// Pre / post market (n/a for crypto).
    pub pre_post: PrePostStyle,
    /// Weekday in date labels ("Fri 20 Nov '26").
    pub day_of_week: bool,
    pub date_format: DateFormat,
    pub hours_format: HoursFormat,
    /// Save chart left edge position when changing interval.
    pub keep_left_edge: bool,
}

impl Default for ScalesStyle {
    fn default() -> Self {
        Self {
            text_size: 12.5,
            currency_label: Visibility3::OnHover,
            scale_buttons: Visibility3::OnHover,
            lock_ratio: None,
            placement: ScalePlacement::Auto,
            no_overlapping_labels: true,
            plus_button: false,
            countdown: true,
            symbol_label: LabelMode::ValueLine,
            symbol_value_mode: LastValueMode::AccordingToScale,
            price_line_width: 1.0,
            price_line_color: None,
            prev_close: LineLabel::default(),
            indicators: LabelMode::Value,
            high_low: LineLabel::default(),
            avg_close: LineLabel::default(),
            bid_ask: BidAskStyle::default(),
            pre_post: PrePostStyle::default(),
            day_of_week: true,
            date_format: DateFormat::DayMonYy,
            hours_format: HoursFormat::H24,
            keep_left_edge: false,
        }
    }
}

// ---- Canvas ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WatermarkStyle {
    pub visible: bool,
    /// None = theme `text` at 6 %.
    pub color: ColorSlot,
    /// "BTCUSDT, 1h".
    pub ticker_interval: bool,
    /// "Bitcoin / TetherUS".
    pub description: bool,
}

impl Default for WatermarkStyle {
    fn default() -> Self {
        Self {
            visible: false,
            color: None,
            ticker_interval: true,
            description: true,
        }
    }
}

/// Empty space around the bars.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Margins {
    /// Percent of the price pane above the bars.
    pub top: f32,
    /// Percent of the price pane below the bars.
    pub bottom: f32,
    /// Bars right of the newest bar.
    pub right: f32,
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 8.0,
            bottom: 8.0,
            right: 5.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasStyle {
    pub background_kind: BackgroundKind,
    /// Solid color / gradient top: theme `bg`.
    pub background: ColorSlot,
    /// Gradient bottom: the background color.
    pub background_bottom: ColorSlot,
    pub grid_lines: GridLines,
    /// theme `grid`.
    pub grid_v_color: ColorSlot,
    pub grid_v_kind: LineKind,
    /// theme `grid`.
    pub grid_h_color: ColorSlot,
    pub grid_h_kind: LineKind,
    /// theme `border`.
    pub separators: ColorSlot,
    /// theme `crosshair`.
    pub crosshair: ColorSlot,
    pub crosshair_width: f32,
    pub crosshair_kind: LineKind,
    pub watermark: WatermarkStyle,
    /// Scales text: theme `text_muted`.
    pub scale_text: ColorSlot,
    /// Scales lines: theme `border`.
    pub scale_lines: ColorSlot,
    pub nav_buttons: ButtonsVisibility,
    pub pane_buttons: ButtonsVisibility,
    pub margins: Margins,
}

impl Default for CanvasStyle {
    fn default() -> Self {
        Self {
            background_kind: BackgroundKind::Solid,
            background: None,
            background_bottom: None,
            grid_lines: GridLines::Both,
            grid_v_color: None,
            // Our look (TradingView: solid).
            grid_v_kind: LineKind::Dotted,
            grid_h_color: None,
            grid_h_kind: LineKind::Dotted,
            separators: None,
            crosshair: None,
            crosshair_width: 1.0,
            crosshair_kind: LineKind::Dashed,
            watermark: WatermarkStyle::default(),
            scale_text: None,
            scale_lines: None,
            nav_buttons: ButtonsVisibility::OnHover,
            pane_buttons: ButtonsVisibility::OnHover,
            margins: Margins::default(),
        }
    }
}

// ---- Trading / Alerts / Events (model only) --------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TradingStyle {
    pub buy_sell_buttons: bool,
    pub instant_orders: bool,
    pub execution_sound: bool,
    pub notifications: TradeNotifications,
    pub positions: bool,
    pub positions_pnl: PnlDisplay,
    pub reverse_button: bool,
    pub orders: bool,
    pub brackets_pnl: bool,
    pub brackets_pnl_display: PnlDisplay,
    pub executions: bool,
    pub execution_labels: bool,
    pub extended_price_line: bool,
    pub alignment: LineAlignment,
    pub orders_on_screenshots: bool,
    pub executions_on_screenshots: bool,
    pub positions_on_screenshots: bool,
}

impl Default for TradingStyle {
    fn default() -> Self {
        Self {
            buy_sell_buttons: true,
            instant_orders: false,
            execution_sound: false,
            notifications: TradeNotifications::RejectionOnly,
            positions: true,
            positions_pnl: PnlDisplay::Money,
            reverse_button: true,
            orders: true,
            brackets_pnl: true,
            brackets_pnl_display: PnlDisplay::Money,
            executions: true,
            execution_labels: false,
            extended_price_line: true,
            alignment: LineAlignment::Right,
            orders_on_screenshots: true,
            executions_on_screenshots: true,
            positions_on_screenshots: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AlertsStyle {
    pub lines: bool,
    /// theme `accent`.
    pub line_color: ColorSlot,
    pub only_active: bool,
    pub volume_on: bool,
    /// 0–100.
    pub volume: u8,
    /// Hide toasts after 20 s.
    pub auto_hide_toasts: bool,
}

impl Default for AlertsStyle {
    fn default() -> Self {
        Self {
            lines: true,
            line_color: None,
            only_active: false,
            volume_on: true,
            volume: 50,
            auto_hide_toasts: false,
        }
    }
}

/// Vertical lines at the start of each trading day (intraday charts).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionBreaks {
    pub visible: bool,
    /// None = TradingView blue #4985e7.
    pub color: ColorSlot,
    pub kind: LineKind,
    pub width: f32,
}

impl Default for SessionBreaks {
    fn default() -> Self {
        Self {
            visible: false,
            color: None,
            kind: LineKind::Dashed,
            width: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EventsStyle {
    pub session_breaks: SessionBreaks,
}

// ---- Lookups ---------------------------------------------------------------

impl ChartStyle {
    /// Resolves a slot against its theme token.
    #[inline]
    pub fn resolve(&self, slot: ColorSlot, token: Color32) -> Color32 {
        resolve(slot, token)
    }

    /// `base` with this style's color overrides applied: the Candles body /
    /// wick colors drive `up` / `down` (and the volume colors through them),
    /// so other types, the tab strip and the status bar follow the candles.
    pub fn theme(&self, base: &Theme) -> Theme {
        let c = &self.symbol.candles;
        let v = &self.canvas;
        let mut t = base.clone();
        if let Some(up) = c.body_up {
            t.up = up.to_color32();
            t.volume_up = tt_theme::with_alpha(t.up, t.volume_up.a() as f32 / 255.0);
        }
        if let Some(down) = c.body_down {
            t.down = down.to_color32();
            t.volume_down = tt_theme::with_alpha(t.down, t.volume_down.a() as f32 / 255.0);
        }
        if let Some(x) = c.wick_up {
            t.up_wick = x.to_color32();
        }
        if let Some(x) = c.wick_down {
            t.down_wick = x.to_color32();
        }
        if let Some(x) = v.background {
            t.bg = x.to_color32();
        }
        if let Some(x) = v.crosshair {
            t.crosshair = x.to_color32();
        }
        if let Some(x) = v.scale_text {
            t.text_muted = x.to_color32();
        }
        // `scale_lines` is not a theme token: only the axis borders use it
        // (separators and legend outlines keep `border`).
        t
    }

    /// Grid color for vertical (time) or horizontal (price) lines.
    pub fn grid_color(&self, theme: &Theme, vertical: bool) -> Color32 {
        let slot = if vertical {
            self.canvas.grid_v_color
        } else {
            self.canvas.grid_h_color
        };
        resolve(slot, theme.grid)
    }

    /// Candle border color of `style` (defaults to the body color, which is
    /// `theme.up` / `theme.down`).
    pub fn border_color(style: &CandleStyle, theme: &Theme, up: bool) -> Color32 {
        if up {
            resolve(style.border_up, theme.up)
        } else {
            resolve(style.border_down, theme.down)
        }
    }

    /// Up / down colors for the tab strip and status bar (the Candles body).
    pub fn up_down_overrides(&self) -> (Option<Color32>, Option<Color32>) {
        let c = &self.symbol.candles;
        (
            c.body_up.map(Rgba::to_color32),
            c.body_down.map(Rgba::to_color32),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_apply_and_serde_round_trips() {
        let mut s = ChartStyle::default();
        s.apply_shared_color(SharedCandleColor::UpBody, Some(Rgba::rgb(1, 2, 3)));
        s.canvas.background = Some(Rgba::rgb(9, 9, 9));
        s.status_line.volume = true;
        let t = s.theme(&Theme::dark());
        assert_eq!(t.up, Color32::from_rgb(1, 2, 3));
        assert_eq!(t.bg, Color32::from_rgb(9, 9, 9));
        assert_eq!(t.down, Theme::dark().down);
        assert_eq!(
            ChartStyle::border_color(&s.symbol.candles, &t, true),
            Color32::from_rgb(1, 2, 3),
            "borders follow the body"
        );
        let json = serde_json::to_string(&s).unwrap();
        let back: ChartStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        // Partial settings get defaults.
        let partial: ChartStyle =
            serde_json::from_str(r#"{"version":2,"scales":{"countdown":false}}"#).unwrap();
        assert!(!partial.scales.countdown && partial.scales.symbol_label.shows_line());
        let empty: ChartStyle = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, ChartStyle::default());
    }

    #[test]
    fn defaults_match_v1() {
        let s = ChartStyle::default();
        assert_eq!(s.canvas.margins.top, 8.0);
        assert!(s.scales.countdown);
        assert_eq!(s.scales.text_size, 12.5);
    }

    #[test]
    fn type_lookups() {
        let mut s = SymbolStyle::default();
        s.heikin_ashi.wick = false;
        s.step_line.width = 3.0;
        assert!(!s.candle(crate::ChartType::HeikinAshi).wick);
        assert!(s.candle(crate::ChartType::Candles).wick);
        assert_eq!(s.line(crate::ChartType::StepLine).width, 3.0);
        assert_eq!(s.line(crate::ChartType::Line).width, 2.0);
    }
}
