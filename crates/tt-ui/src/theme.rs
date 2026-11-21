//! The UI palette: one source of truth.
//!
//! [`UiTheme`] is a gpui global holding the current `tt_theme::Theme`; every
//! kit component reads it through [`ActiveUiTheme::ui`]. [`sync`] sets it and
//! maps the tokens into gpui-component's `ThemeColor` (§2.5), so stock
//! components (inputs, checkboxes, dialogs, tooltips, popovers) wear the
//! same colors as the kit.

use gpui_kit::component::{Theme as ComponentTheme, ThemeMode as ComponentMode};
use gpui_kit::*;
use std::sync::OnceLock;
use tt_theme::{Color32, Theme, metrics as m, mix};

/// The current UI palette, readable from any render.
#[derive(Clone, Debug)]
pub struct UiTheme(pub Theme);

impl Global for UiTheme {}

/// `cx.ui()`: the current UI palette.
pub trait ActiveUiTheme {
    fn ui(&self) -> &Theme;
}

impl ActiveUiTheme for App {
    fn ui(&self) -> &Theme {
        if let Some(t) = self.try_global::<UiTheme>() {
            return &t.0;
        }
        // Before the first `sync` (tests, early renders): the dark palette.
        static FALLBACK: OnceLock<Theme> = OnceLock::new();
        FALLBACK.get_or_init(Theme::dark)
    }
}

/// tt color → gpui color.
pub fn hsla(c: Color32) -> Hsla {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Hsla::from(Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
    })
}

/// Sets the global [`UiTheme`], switches gpui-component's light/dark mode,
/// maps the tokens into its `ThemeColor` (§2.5 table), and sets its radii
/// and motion durations from the tokens. Refreshes every window.
///
/// Call it at startup and whenever the palette changes (theme switch, chart
/// style overrides).
pub fn sync(theme: &Theme, cx: &mut App) {
    cx.set_global(UiTheme(theme.clone()));
    let mode = if theme.dark {
        ComponentMode::Dark
    } else {
        ComponentMode::Light
    };
    let needs_mode = cx
        .try_global::<ComponentTheme>()
        .is_none_or(|g| g.mode != mode);
    if needs_mode {
        ComponentTheme::change(mode, None, cx);
    }
    let t = theme.clone();
    ComponentTheme::update(cx, move |g| {
        let c = &mut g.colors;
        let h = hsla;
        c.background = h(t.panel);
        c.foreground = h(t.text);
        c.muted = h(t.panel_alt);
        c.muted_foreground = h(t.text_muted);
        c.border = h(t.border);
        c.input = h(t.border);
        c.popover = h(t.overlay_bg);
        c.popover_foreground = h(t.text);
        // Ghost hover and menu highlight.
        c.accent = h(t.hover);
        c.accent_foreground = h(t.text);
        c.secondary = h(t.panel_alt);
        c.secondary_foreground = h(t.text);
        c.secondary_hover = h(t.hover);
        c.secondary_active = h(t.selection);
        c.list = h(t.panel);
        c.list_hover = h(t.hover);
        c.list_active = h(t.selection);
        c.list_active_border = h(t.selection);
        c.selection = h(tt_theme::with_alpha(t.active, 0.35));
        // TradingView-style neutral primary (the Ok button).
        c.primary = h(t.text);
        c.primary_foreground = h(t.bg);
        c.primary_hover = h(mix(t.text, t.bg, 0.12));
        c.primary_active = h(mix(t.text, t.bg, 0.22));
        c.ring = h(t.focus_ring);
        c.caret = h(t.text);
        c.link = h(t.active);
        c.link_hover = h(t.active);
        c.link_active = h(t.active);
        c.danger = h(t.danger);
        c.warning = h(t.warning);
        c.tab_bar = h(t.chrome);
        c.tab_active = h(t.tab_active);
        c.tab_active_foreground = h(t.text);
        c.tab_foreground = h(t.text_muted);
        c.title_bar = h(t.chrome);
        c.title_bar_border = h(t.chrome);
        c.status_bar = h(t.panel);
        c.status_bar_border = h(t.border);
        c.sidebar = h(t.panel);
        c.sidebar_foreground = h(t.text);
        c.sidebar_border = h(t.border);
        c.sidebar_accent = h(t.selection);
        c.sidebar_accent_foreground = h(t.text);
        c.window_border = h(t.border);

        g.radius = px(m::R_MD);
        g.radius_lg = px(m::R_XL);

        let motion = &mut g.motion;
        motion.duration_fast = crate::tokens::ms(m::MOTION_FAST);
        motion.duration_normal = crate::tokens::ms(m::MOTION_NORMAL);
        motion.duration_slow = crate::tokens::ms(m::MOTION_SLOW);
        motion.spring_control =
            gpui_kit::base::Spring::new(crate::tokens::ms(m::MOTION_SPRING_CONTROL));
        motion.spring_move = gpui_kit::base::Spring::new(crate::tokens::ms(m::MOTION_SPRING_MOVE))
            .with_damping(0.85)
            .with_epsilon(0.1);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn hsla_round_trips_opaque_colors() {
        let c = hsla(Color32::from_rgb(0x29, 0x62, 0xff));
        let rgba = c.to_rgb();
        assert!((rgba.r - 0x29 as f32 / 255.0).abs() < 1e-3);
        assert!((rgba.b - 1.0).abs() < 1e-3);
        assert_eq!(rgba.a, 1.0);
    }
}
