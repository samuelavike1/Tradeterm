//! Dark theme modelled on TradingView's palette.

use egui::{CornerRadius, FontFamily, FontId, Margin, Stroke, Style, TextStyle, Visuals, vec2};
use tt_chart::ChartStyle;

pub mod palette {
    use egui::Color32;

    /// Chart background.
    pub const BG: Color32 = Color32::from_rgb(0x13, 0x17, 0x22);
    /// Toolbars and status bar.
    pub const PANEL: Color32 = Color32::from_rgb(0x18, 0x1c, 0x27);
    /// Windows, menus, popups.
    pub const SURFACE: Color32 = Color32::from_rgb(0x1e, 0x22, 0x2d);
    pub const BORDER: Color32 = Color32::from_rgb(0x2a, 0x2e, 0x39);
    pub const CONTROL: Color32 = Color32::from_rgb(0x2a, 0x2e, 0x39);
    pub const CONTROL_HOVER: Color32 = Color32::from_rgb(0x36, 0x3a, 0x45);
    pub const CONTROL_ACTIVE: Color32 = Color32::from_rgb(0x43, 0x46, 0x51);
    pub const TEXT: Color32 = Color32::from_rgb(0xd1, 0xd4, 0xdc);
    pub const TEXT_WEAK: Color32 = Color32::from_rgb(0x78, 0x7b, 0x86);
    pub const TEXT_STRONG: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
    pub const ACCENT: Color32 = Color32::from_rgb(0x29, 0x62, 0xff);
    pub const UP: Color32 = Color32::from_rgb(0x26, 0xa6, 0x9a);
    pub const DOWN: Color32 = Color32::from_rgb(0xef, 0x53, 0x50);
    pub const WARN: Color32 = Color32::from_rgb(0xf2, 0xa9, 0x00);
}

use palette::*;

/// Installs the theme for both egui light and dark modes so the OS setting can't override it.
pub fn apply(ctx: &egui::Context) {
    ctx.all_styles_mut(configure);
}

fn configure(style: &mut Style) {
    style.visuals = visuals();

    let sp = &mut style.spacing;
    sp.item_spacing = vec2(8.0, 6.0);
    sp.button_padding = vec2(10.0, 4.0);
    sp.interact_size.y = 24.0;
    sp.window_margin = Margin::same(10);
    sp.menu_margin = Margin::same(6);

    use FontFamily::{Monospace, Proportional};
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0, Proportional)),
        (TextStyle::Body, FontId::new(13.0, Proportional)),
        (TextStyle::Button, FontId::new(13.0, Proportional)),
        (TextStyle::Heading, FontId::new(16.0, Proportional)),
        (TextStyle::Monospace, FontId::new(12.5, Monospace)),
    ]
    .into();
}

fn visuals() -> Visuals {
    let mut v = Visuals::dark();
    let r4 = CornerRadius::same(4);
    let r6 = CornerRadius::same(6);

    v.panel_fill = PANEL;
    v.window_fill = SURFACE;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = r6;
    v.menu_corner_radius = r6;
    v.faint_bg_color = SURFACE;
    v.extreme_bg_color = BG;
    v.text_edit_bg_color = Some(BG);
    v.code_bg_color = SURFACE;
    v.hyperlink_color = ACCENT;
    v.warn_fg_color = WARN;
    v.error_fg_color = DOWN;

    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, TEXT_STRONG);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.weak_bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.corner_radius = r4;

    for (wv, fill) in [
        (&mut w.inactive, CONTROL),
        (&mut w.hovered, CONTROL_HOVER),
        (&mut w.active, CONTROL_ACTIVE),
        (&mut w.open, CONTROL_HOVER),
    ] {
        wv.bg_fill = fill;
        wv.weak_bg_fill = fill;
        wv.bg_stroke = Stroke::NONE;
        wv.corner_radius = r4;
        wv.expansion = 0.0;
    }
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.hovered.fg_stroke = Stroke::new(1.0, TEXT_STRONG);
    w.active.fg_stroke = Stroke::new(1.0, TEXT_STRONG);
    w.open.fg_stroke = Stroke::new(1.0, TEXT_STRONG);
    // Focused text edits get an accent outline.
    w.hovered.bg_stroke = Stroke::new(1.0, CONTROL_ACTIVE);
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);

    v
}

/// Frame for the top bar and status bar.
pub fn bar_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .inner_margin(Margin::symmetric(10, 6))
        .stroke(Stroke::new(1.0, BORDER))
}

pub fn chart_style() -> ChartStyle {
    ChartStyle {
        background: BG,
        up: UP,
        down: DOWN,
        text: TEXT,
        ..ChartStyle::default()
    }
}
