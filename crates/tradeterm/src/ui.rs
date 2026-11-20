//! Small UI building blocks and the app's motion.
//!
//! Motion is deliberately snappy: state changes ease over ~100–150 ms, hover
//! feedback is instant, and nothing that shows data (prices, charts,
//! crosshair) is ever animated. gpui-kit honours the OS "reduce motion"
//! setting (Windows: Animation effects off), snapping to the end state.

use crate::app::TradeTerm;
use egui::Color32;
use gpui_kit::base::{
    Easing, IterationCount, Keyframe, Keyframes, Timing, Transition, animate_keyframes, transition,
};
use gpui_kit::component::{ActiveTheme, Theme as UiTheme, ThemeMode as UiThemeMode, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;
use tt_chart::ChartType;
use tt_chart_gpui::hsla;
use tt_theme::{Theme, mix, with_alpha};

pub const UI_FONT: &str = if cfg!(target_os = "windows") {
    "Segoe UI"
} else {
    ".SystemUIFont"
};

/// Standard snappy transition for state changes.
pub fn snappy(ms: u64) -> Transition {
    Transition::new(Duration::from_millis(ms)).easing(Easing::EaseOut)
}

/// Switches gpui-component's palette with our light/dark mode.
pub fn apply_theme(dark: bool, window: &mut Window, cx: &mut App) {
    let mode = if dark {
        UiThemeMode::Dark
    } else {
        UiThemeMode::Light
    };
    UiTheme::change(mode, Some(window), cx);
    // Snappy motion for every component (menus, popovers, dialogs): about
    // two thirds of gpui-component's defaults.
    let m = &mut UiTheme::global_mut(cx).motion;
    m.duration_fast = Duration::from_millis(90);
    m.duration_normal = Duration::from_millis(130);
    m.duration_slow = Duration::from_millis(200);
    m.spring_control = gpui_kit::base::Spring::new(Duration::from_millis(140));
    m.spring_move = gpui_kit::base::Spring::new(Duration::from_millis(200))
        .with_damping(0.85)
        .with_epsilon(0.1);
}

/// A Lucide glyph (from the icon font registered by `tt_chart_gpui::init`).
pub fn lucide(icon: lucide_icons::Icon, size: f32) -> Div {
    div()
        .font_family(tt_chart_gpui::ICON_FONT)
        .text_size(px(size))
        .line_height(px(size))
        .child(char::from(icon).to_string())
}

/// A thin vertical toolbar separator.
pub fn separator(theme: &Theme) -> Div {
    div().w(px(1.)).h(px(20.)).mx_1().bg(hsla(theme.border))
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
        div()
            .size(px(8.))
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

/// One chart tab in the title bar: badge, symbol, change %, close button on
/// hover. The active tab's background eases in.
#[allow(clippy::too_many_arguments)]
pub fn tab_pill(
    index: usize,
    active: bool,
    label: String,
    badge: String,
    change: Option<f64>,
    closable: bool,
    theme: &Theme,
    window: &mut Window,
    cx: &mut Context<TradeTerm>,
) -> impl IntoElement {
    let fill = transition(
        SharedString::from(format!("tab-fill-{index}")),
        if active { 1.0f32 } else { 0.0 },
        snappy(120),
        window,
        cx,
    );
    let bg = with_alpha(theme.tab_active, fill);
    let hover_bg = with_alpha(mix(theme.chrome, theme.tab_active, 0.5), 1.0);
    let group = SharedString::from(format!("tab-{index}"));
    h_flex()
        .id(("tab", index))
        .group(group.clone())
        .h(px(30.))
        .pl_2()
        .pr_1()
        .gap_2()
        .items_center()
        .rounded(px(7.))
        .bg(hsla(bg))
        .when(!active, |d| d.hover(|s| s.bg(hsla(hover_bg))))
        .cursor_pointer()
        .text_size(px(13.))
        .child(
            div()
                .size(px(18.))
                .rounded_full()
                .bg(hsla(badge_color(theme, &badge)))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.))
                .text_color(hsla(theme.text_on_accent))
                .child(badge.chars().next().unwrap_or('?').to_string()),
        )
        .child(div().text_color(hsla(theme.text)).child(label))
        .children(change.map(|pct| {
            let color = if pct >= 0.0 { theme.up } else { theme.down };
            div()
                .text_size(px(12.))
                .text_color(hsla(color))
                .child(format!(
                    "{}{:.2}%",
                    if pct >= 0.0 { "▲ " } else { "▼ " },
                    pct.abs()
                ))
        }))
        .child(
            div()
                .id(("tab-close", index))
                .size(px(18.))
                .rounded(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .text_color(hsla(theme.text_muted))
                .invisible()
                .when(closable, |d| {
                    d.group_hover(group, |s| s.visible())
                        .hover(|s| s.bg(hsla(theme.selection)).text_color(hsla(theme.text)))
                })
                .child(lucide(lucide_icons::Icon::X, 13.))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.on_tab_click(index, true, cx)
                })),
        )
        .on_click(cx.listener(move |this, _, _, cx| this.on_tab_click(index, false, cx)))
        .on_mouse_down(
            MouseButton::Middle,
            cx.listener(move |this, _, _, cx| this.on_tab_click(index, true, cx)),
        )
        .text_color(cx.theme().foreground)
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
