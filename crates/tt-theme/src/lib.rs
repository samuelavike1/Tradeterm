//! Semantic color tokens shared by the chart and the application UI.
//!
//! Every color drawn by tradeterm comes from a [`Theme`]. The chart reads the
//! tokens directly; the rest of the UI gets them through egui [`egui::Style`]s
//! installed by [`install`]. Changing palette therefore restyles everything.

use egui::{
    Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, Style, TextStyle, Visuals, vec2,
};
use serde::{Deserialize, Serialize};

/// Which palette to use. `System` follows the OS light/dark setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeMode {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Dark, ThemeMode::Light];

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::System => "System",
            ThemeMode::Dark => "Dark",
            ThemeMode::Light => "Light",
        }
    }

    fn preference(self) -> egui::ThemePreference {
        match self {
            ThemeMode::System => egui::ThemePreference::System,
            ThemeMode::Dark => egui::ThemePreference::Dark,
            ThemeMode::Light => egui::ThemePreference::Light,
        }
    }
}

/// User overrides layered on top of a built-in palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThemeOverrides {
    pub up: Option<Color32>,
    pub down: Option<Color32>,
}

/// Semantic color tokens.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub dark: bool,
    /// Chart and text-field background.
    pub bg: Color32,
    /// Toolbars and status bar.
    pub panel: Color32,
    /// Raised surfaces: menus, popups, inactive controls.
    pub panel_alt: Color32,
    pub border: Color32,
    pub grid: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    /// Text drawn on top of `accent`, `up`, `down` or `crosshair_label_bg`.
    pub text_on_accent: Color32,
    pub up: Color32,
    pub down: Color32,
    pub up_wick: Color32,
    pub down_wick: Color32,
    pub crosshair: Color32,
    pub crosshair_label_bg: Color32,
    pub selection: Color32,
    pub volume_up: Color32,
    pub volume_down: Color32,
    pub series_palette: [Color32; 8],
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Same color with `alpha` (0..=1) opacity.
pub fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Linear blend in sRGB space: `t = 0` → `a`, `t = 1` → `b`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let [ar, ag, ab, aa] = a.to_srgba_unmultiplied();
    let [br, bg, bb, ba] = b.to_srgba_unmultiplied();
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(ar, br), l(ag, bg), l(ab, bb), l(aa, ba))
}

const SERIES_PALETTE: [Color32; 8] = [
    rgb(0x2962ff),
    rgb(0xff9800),
    rgb(0xe91e63),
    rgb(0x00bcd4),
    rgb(0x9c27b0),
    rgb(0x4caf50),
    rgb(0xfdd835),
    rgb(0x8d6e63),
];

impl Theme {
    /// TradingView-like dark palette.
    pub fn dark() -> Self {
        let up = rgb(0x26a69a);
        let down = rgb(0xef5350);
        Self {
            dark: true,
            bg: rgb(0x131722),
            panel: rgb(0x181c27),
            panel_alt: rgb(0x1e222d),
            border: rgb(0x2a2e39),
            grid: rgb(0x1f2330),
            text: rgb(0xd1d4dc),
            text_muted: rgb(0x787b86),
            accent: rgb(0x2962ff),
            text_on_accent: rgb(0xffffff),
            up,
            down,
            up_wick: up,
            down_wick: down,
            crosshair: rgb(0x758696),
            crosshair_label_bg: rgb(0x4c525e),
            selection: with_alpha(rgb(0x2962ff), 0.35),
            volume_up: with_alpha(up, 0.45),
            volume_down: with_alpha(down, 0.45),
            series_palette: SERIES_PALETTE,
        }
    }

    /// Light palette.
    pub fn light() -> Self {
        let up = rgb(0x089981);
        let down = rgb(0xf23645);
        Self {
            dark: false,
            bg: rgb(0xffffff),
            panel: rgb(0xf8f9fd),
            panel_alt: rgb(0xf0f3fa),
            border: rgb(0xe0e3eb),
            grid: rgb(0xf0f3fa),
            text: rgb(0x131722),
            text_muted: rgb(0x787b86),
            accent: rgb(0x2962ff),
            text_on_accent: rgb(0xffffff),
            up,
            down,
            up_wick: up,
            down_wick: down,
            crosshair: rgb(0x9598a1),
            crosshair_label_bg: rgb(0x131722),
            selection: with_alpha(rgb(0x2962ff), 0.25),
            volume_up: with_alpha(up, 0.40),
            volume_down: with_alpha(down, 0.40),
            series_palette: SERIES_PALETTE,
        }
    }

    pub fn for_dark(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }

    /// Applies up/down overrides; wick and volume tokens follow the new colors.
    pub fn with_overrides(mut self, o: ThemeOverrides) -> Self {
        let vol_alpha = if self.dark { 0.45 } else { 0.40 };
        if let Some(up) = o.up {
            self.up = up;
            self.up_wick = up;
            self.volume_up = with_alpha(up, vol_alpha);
        }
        if let Some(down) = o.down {
            self.down = down;
            self.down_wick = down;
            self.volume_down = with_alpha(down, vol_alpha);
        }
        self
    }

    /// Writes the tokens into an egui style (visuals, rounding, spacing, fonts).
    pub fn apply_to_style(&self, style: &mut Style) {
        style.visuals = self.visuals();

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

    fn visuals(&self) -> Visuals {
        let mut v = if self.dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        let r4 = CornerRadius::same(4);
        let r6 = CornerRadius::same(6);

        // Control fills are derived from the surface tokens.
        let control = self.panel_alt;
        let control_hover = self.border;
        let control_active = mix(self.border, self.text_muted, 0.35);

        v.override_text_color = None;
        v.panel_fill = self.panel;
        v.window_fill = self.panel_alt;
        v.window_stroke = Stroke::new(1.0, self.border);
        v.window_corner_radius = r6;
        v.menu_corner_radius = r6;
        v.faint_bg_color = self.panel_alt;
        v.extreme_bg_color = self.bg;
        v.text_edit_bg_color = Some(self.bg);
        v.code_bg_color = self.panel_alt;
        v.hyperlink_color = self.accent;
        v.warn_fg_color = self.series_palette[1];
        v.error_fg_color = self.down;
        v.weak_text_color = Some(self.text_muted);

        v.selection.bg_fill = self.accent;
        v.selection.stroke = Stroke::new(1.0, self.text_on_accent);

        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.panel;
        w.noninteractive.weak_bg_fill = self.panel;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.text);
        w.noninteractive.corner_radius = r4;

        for (wv, fill) in [
            (&mut w.inactive, control),
            (&mut w.hovered, control_hover),
            (&mut w.active, control_active),
            (&mut w.open, control_hover),
        ] {
            // weak_bg_fill: buttons; bg_fill: checkbox/radio/slider wells, which
            // must stand out from menus (panel_alt) too, so use the border tone.
            wv.weak_bg_fill = fill;
            wv.bg_fill = mix(fill, self.border, 0.5);
            wv.bg_stroke = Stroke::NONE;
            wv.fg_stroke = Stroke::new(1.0, self.text);
            wv.corner_radius = r4;
            wv.expansion = 0.0;
        }
        w.inactive.bg_fill = self.border;
        w.inactive.bg_stroke = Stroke::new(1.0, mix(self.border, self.text_muted, 0.4));
        w.hovered.bg_stroke = Stroke::new(1.0, control_active);
        w.active.bg_stroke = Stroke::new(1.0, self.accent);
        v
    }

    /// Frame for toolbars / status bars.
    pub fn bar_frame(&self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.panel)
            .inner_margin(Margin::symmetric(10, 6))
            .stroke(Stroke::new(1.0, self.border))
    }
}

/// Installs both palettes into egui and selects one according to `mode`.
/// Call again whenever the mode or overrides change.
pub fn install(ctx: &egui::Context, mode: ThemeMode, overrides: ThemeOverrides) {
    for (egui_theme, dark) in [(egui::Theme::Dark, true), (egui::Theme::Light, false)] {
        let theme = Theme::for_dark(dark).with_overrides(overrides);
        ctx.style_mut_of(egui_theme, |s| theme.apply_to_style(s));
    }
    ctx.set_theme(mode.preference());
}

/// The palette currently in effect (resolves `System` via egui).
pub fn current(ctx: &egui::Context, overrides: ThemeOverrides) -> Theme {
    Theme::for_dark(ctx.theme() == egui::Theme::Dark).with_overrides(overrides)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_propagate_to_derived_tokens() {
        let red = Color32::from_rgb(255, 0, 0);
        let t = Theme::dark().with_overrides(ThemeOverrides {
            up: Some(red),
            down: None,
        });
        assert_eq!(t.up, red);
        assert_eq!(t.up_wick, red);
        assert_eq!(t.volume_up.to_srgba_unmultiplied()[..3], [255, 0, 0]);
        assert_eq!(t.down, Theme::dark().down);
    }

    #[test]
    fn palettes_differ_in_brightness() {
        assert!(Theme::dark().bg.intensity() < 0.2);
        assert!(Theme::light().bg.intensity() > 0.8);
        assert!(Theme::dark().dark && !Theme::light().dark);
    }

    #[test]
    fn mix_endpoints() {
        let a = rgb(0x000000);
        let b = rgb(0xffffff);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
    }
}
