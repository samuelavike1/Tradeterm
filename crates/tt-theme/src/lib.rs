//! Semantic color tokens shared by the chart and the application UI.
//!
//! Every color drawn by tradeterm comes from a [`Theme`]: the chart reads the
//! tokens directly and the app's chrome uses them alongside gpui-component's
//! theme. Colors are [`ecolor::Color32`] (sRGB, premultiplied alpha).
//!
//! [`metrics`] holds the numeric design tokens (spacing, radii, text, icon
//! and control sizes, motion) shared by the canvas and the gpui kit
//! (`tt-ui`), which also projects these colors into gpui-component's theme.

pub mod metrics;

pub use ecolor::Color32;
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
    /// Title bar / tab strip.
    pub chrome: Color32,
    /// The selected tab in the tab strip.
    pub tab_active: Color32,
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
    /// Neutral emphasis (chart crosshair labels).
    ///
    /// **Deprecated for UI code**: use [`Theme::active`] for "on" states and
    /// focus, [`Theme::selection`] / [`Theme::hover`] for backgrounds.
    pub accent: Color32,
    /// Text drawn on top of `accent`, `up`, `down` or `crosshair_label_bg`.
    pub text_on_accent: Color32,
    pub up: Color32,
    pub down: Color32,
    pub up_wick: Color32,
    pub down_wick: Color32,
    pub crosshair: Color32,
    pub crosshair_label_bg: Color32,
    /// Background of selected controls (active timeframe, chosen list row),
    /// toggled buttons, pressed state and text selection.
    pub selection: Color32,
    /// Hover background of any control or row (one step from `selection`).
    pub hover: Color32,
    /// "On" state: active drawing tool icon, A/L/% when on, search match
    /// highlight, links (TradingView's blue).
    pub active: Color32,
    /// Tinted background where a blue fill is wanted (`active` at low alpha).
    pub active_bg: Color32,
    /// Keyboard focus outline.
    pub focus_ring: Color32,
    /// Disabled labels.
    pub text_disabled: Color32,
    /// Favorite stars.
    pub favorite: Color32,
    /// Connecting status, warnings.
    pub warning: Color32,
    /// Destructive menu rows, validation errors (separate from `down`).
    pub danger: Color32,
    /// Floating toolbars, perf overlay, menus and popovers.
    pub overlay_bg: Color32,
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

/// TradingView's blue: the `active` token in both palettes.
const ACTIVE: Color32 = rgb(0x2962ff);

/// Black or white, whichever reads better on `bg` (badges on palette colors).
pub fn badge_fg(bg: Color32) -> Color32 {
    let [r, g, b, _] = bg.to_srgba_unmultiplied();
    // Relative luminance (sRGB to linear, Rec. 709 weights).
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let l = 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
    // WCAG contrast against white vs. against near-black; pick the larger.
    if 1.05 / (l + 0.05) >= (l + 0.05) / 0.05 {
        Color32::WHITE
    } else {
        rgb(0x0a0a0a)
    }
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
    /// Neutral dark palette (pure grays, shadcn "neutral" scale) with
    /// TradingView-style up/down colors.
    pub fn dark() -> Self {
        let up = rgb(0x089981);
        let down = rgb(0xf23645);
        Self {
            dark: true,
            chrome: rgb(0x141414),
            tab_active: rgb(0x262626),
            bg: rgb(0x0a0a0a),
            panel: rgb(0x0a0a0a),
            panel_alt: rgb(0x171717),
            border: rgb(0x262626),
            grid: rgb(0x303030),
            text: rgb(0xfafafa),
            text_muted: rgb(0xb4b4b4),
            accent: rgb(0xd4d4d4),
            text_on_accent: rgb(0xffffff),
            up,
            down,
            up_wick: up,
            down_wick: down,
            crosshair: rgb(0x9c9c9c),
            crosshair_label_bg: rgb(0x303030),
            selection: rgb(0x2e2e2e),
            hover: rgb(0x232323),
            active: ACTIVE,
            active_bg: with_alpha(ACTIVE, 0.18),
            focus_ring: with_alpha(ACTIVE, 0.60),
            text_disabled: rgb(0x5c5c5c),
            favorite: rgb(0xf5b52e),
            warning: rgb(0xf5b52e),
            danger: rgb(0xf23645),
            overlay_bg: rgb(0x141414),
            volume_up: with_alpha(up, 0.55),
            volume_down: with_alpha(down, 0.55),
            series_palette: SERIES_PALETTE,
        }
    }

    /// Neutral light palette.
    pub fn light() -> Self {
        let up = rgb(0x089981);
        let down = rgb(0xf23645);
        Self {
            dark: false,
            chrome: rgb(0xebebeb),
            tab_active: rgb(0xffffff),
            bg: rgb(0xffffff),
            panel: rgb(0xffffff),
            panel_alt: rgb(0xf5f5f5),
            border: rgb(0xe5e5e5),
            grid: rgb(0xd4d4d4),
            text: rgb(0x0a0a0a),
            text_muted: rgb(0x666666),
            accent: rgb(0x171717),
            text_on_accent: rgb(0xffffff),
            up,
            down,
            up_wick: up,
            down_wick: down,
            crosshair: rgb(0x9c9c9c),
            crosshair_label_bg: rgb(0x171717),
            selection: rgb(0xe5e5e5),
            hover: rgb(0xf0f0f0),
            active: ACTIVE,
            active_bg: with_alpha(ACTIVE, 0.12),
            focus_ring: with_alpha(ACTIVE, 0.50),
            text_disabled: rgb(0xa3a3a3),
            favorite: rgb(0xf5a623),
            warning: rgb(0xd48806),
            danger: rgb(0xe0293a),
            overlay_bg: rgb(0xffffff),
            volume_up: with_alpha(up, 0.50),
            volume_down: with_alpha(down, 0.50),
            series_palette: SERIES_PALETTE,
        }
    }

    pub fn for_dark(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }

    /// Applies up/down overrides; wick and volume tokens follow the new colors.
    pub fn with_overrides(mut self, o: ThemeOverrides) -> Self {
        let vol_alpha = if self.dark { 0.55 } else { 0.50 };
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
    fn badge_fg_picks_contrast() {
        assert_eq!(badge_fg(rgb(0xfdd835)), rgb(0x0a0a0a));
        assert_eq!(badge_fg(rgb(0x2962ff)), Color32::WHITE);
        assert_eq!(badge_fg(rgb(0x000000)), Color32::WHITE);
    }

    #[test]
    fn hover_sits_between_panel_and_selection() {
        for t in [Theme::dark(), Theme::light()] {
            let (p, h, s) = (
                t.panel.intensity(),
                t.hover.intensity(),
                t.selection.intensity(),
            );
            assert!((p < h && h < s) || (p > h && h > s), "dark={}", t.dark);
        }
    }

    #[test]
    fn mix_endpoints() {
        let a = rgb(0x000000);
        let b = rgb(0xffffff);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
    }
}
