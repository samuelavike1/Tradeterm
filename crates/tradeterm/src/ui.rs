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

pub use tt_theme::badge_color;

/// An asset's logo, round-clipped to `size`, or `badge` (its letter badge)
/// when there is no cached logo or it fails to decode. The image is decoded
/// once by gpui's asset cache, off the UI thread.
pub fn logo_or_badge(
    logo: Option<std::sync::Arc<std::path::Path>>,
    size: Pixels,
    badge: impl Fn() -> AnyElement + 'static,
) -> AnyElement {
    match logo {
        Some(path) => img(path)
            .flex_none()
            .size(size)
            .rounded_full()
            .with_fallback(badge)
            .into_any_element(),
        None => badge(),
    }
}

/// A chart type's icon (our 1-stroke SVG set): toolbar button, quick
/// buttons, menu rows.
pub fn chart_type_icon(t: ChartType) -> tt_ui::Glyph {
    use tt_ui::Glyph;
    match t {
        ChartType::Bars => Glyph::CHART_BARS,
        ChartType::Candles => Glyph::CHART_CANDLES,
        ChartType::HollowCandles => Glyph::CHART_HOLLOW_CANDLES,
        ChartType::VolumeCandles => Glyph::CHART_VOLUME_CANDLES,
        ChartType::Line => Glyph::CHART_LINE,
        ChartType::LineWithMarkers => Glyph::CHART_LINE_MARKERS,
        ChartType::StepLine => Glyph::CHART_STEP_LINE,
        ChartType::Area => Glyph::CHART_AREA,
        ChartType::HlcArea => Glyph::CHART_HLC_AREA,
        ChartType::Baseline => Glyph::CHART_BASELINE,
        ChartType::Columns => Glyph::CHART_COLUMNS,
        ChartType::HighLow => Glyph::CHART_HIGH_LOW,
        ChartType::HeikinAshi => Glyph::CHART_HEIKIN_ASHI,
        ChartType::Renko => Glyph::CHART_RENKO,
        ChartType::LineBreak => Glyph::CHART_LINE_BREAK,
    }
}

/// A drawing tool's icon: our SVG where we have one (the left toolbar's
/// tools), else the tool's Lucide glyph.
pub fn drawing_tool_icon(t: tt_chart::DrawingTool) -> tt_ui::Glyph {
    use tt_chart::DrawingTool as D;
    use tt_ui::Glyph;
    match t {
        D::TrendLine => Glyph::DRAW_TREND_LINE,
        D::Ray => Glyph::DRAW_RAY,
        D::InfoLine => Glyph::DRAW_INFO_LINE,
        D::ExtendedLine => Glyph::DRAW_EXTENDED_LINE,
        D::TrendAngle => Glyph::DRAW_TREND_ANGLE,
        D::HorizontalLine => Glyph::DRAW_HORIZONTAL_LINE,
        D::HorizontalRay => Glyph::DRAW_HORIZONTAL_RAY,
        D::VerticalLine => Glyph::DRAW_VERTICAL_LINE,
        D::CrossLine => Glyph::DRAW_CROSS_LINE,
        D::ParallelChannel => Glyph::DRAW_PARALLEL_CHANNEL,
        D::Arrow => Glyph::DRAW_ARROW,
        D::FibRetracement => Glyph::DRAW_FIB_RETRACEMENT,
        D::FibExtension => Glyph::DRAW_FIB_EXTENSION,
        D::Rectangle => Glyph::DRAW_RECTANGLE,
        D::PriceRange => Glyph::DRAW_PRICE_RANGE,
        D::DateRange => Glyph::DRAW_DATE_RANGE,
        D::DateAndPriceRange => Glyph::DRAW_DATE_PRICE_RANGE,
        other => Glyph::from(other.icon()),
    }
}
