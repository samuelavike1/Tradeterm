//! Small UI building blocks and the app's motion.
//!
//! Motion is deliberately snappy: state changes ease over ~100–150 ms, hover
//! feedback is instant, and nothing that shows data (prices, charts,
//! crosshair) is ever animated. gpui-kit honours the OS "reduce motion"
//! setting (Windows: Animation effects off), snapping to the end state.

use ecolor::Color32;
use gpui_kit::base::{
    Easing, IterationCount, Keyframe, Keyframes, Timing, Transition, animate_keyframes,
};
use gpui_kit::*;
use std::time::Duration;
use tt_chart::ChartType;
use tt_chart_gpui::hsla;
use tt_theme::Theme;

pub const UI_FONT: &str = if cfg!(target_os = "windows") {
    "Segoe UI"
} else {
    ".SystemUIFont"
};

/// Standard snappy transition for state changes (kit: `tt_ui::motion`).
pub fn snappy(ms: u64) -> Transition {
    tt_ui::motion::snappy(ms)
}

/// Sets the UI palette: the kit's global `UiTheme` and gpui-component's
/// theme (mode, colors, radii, motion) from the same tokens.
pub fn apply_theme(theme: &Theme, cx: &mut App) {
    tt_ui::theme::sync(theme, cx);
}

/// Connection status dot; breathes (keyframed opacity) while connecting.
pub fn status_dot(color: Color32, pulsing: bool) -> StatusDot {
    StatusDot { color, pulsing }
}

#[derive(IntoElement)]
pub struct StatusDot {
    color: Color32,
    pulsing: bool,
}

impl RenderOnce for StatusDot {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let opacity = if self.pulsing {
            let frames = Keyframes::try_new([
                Keyframe::new(0.0, 0.35),
                Keyframe::new(0.5, 1.0).ease(Easing::EaseInOut),
                Keyframe::new(1.0, 0.35).ease(Easing::EaseInOut),
            ])
            .expect("valid keyframes");
            animate_keyframes(
                "status-pulse",
                &frames,
                Timing::new(Duration::from_millis(1100)).iterations(IterationCount::Infinite),
                window,
                cx,
            )
            .value
        } else {
            1.0
        };
        // The dot is one grid step (S2) across.
        div()
            .size(px(tt_ui::tokens::S2))
            .rounded_full()
            .bg(hsla(self.color))
            .opacity(opacity)
    }
}

/// Deterministic badge colour for an asset (stable across runs).
pub fn badge_color(theme: &Theme, key: &str) -> Color32 {
    let h = key
        .bytes()
        .fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let palette = &theme.series_palette;
    palette[(h as usize) % palette.len()]
}

/// Lucide icon for a chart type (toolbar button, menu rows).
pub fn chart_type_icon(t: ChartType) -> lucide_icons::Icon {
    use lucide_icons::Icon;
    match t {
        ChartType::Bars => Icon::ChartNoAxesGantt,
        ChartType::Candles | ChartType::HollowCandles | ChartType::HeikinAshi => {
            Icon::ChartCandlestick
        }
        ChartType::VolumeCandles => Icon::ChartBarStacked,
        ChartType::Line => Icon::ChartLine,
        ChartType::LineWithMarkers => Icon::ChartScatter,
        ChartType::StepLine => Icon::ChartNoAxesCombined,
        ChartType::Area | ChartType::HlcArea | ChartType::Baseline => Icon::ChartArea,
        ChartType::Columns => Icon::ChartColumn,
        ChartType::HighLow => Icon::ChartColumnBig,
        ChartType::Renko => Icon::Blocks,
        ChartType::LineBreak => Icon::ChartColumnStacked,
    }
}
