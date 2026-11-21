//! The "⋯" window menu at the right end of the title bar, after TradingView
//! desktop's (docs/specs/img/tv-ref/tv-window-menu.png), without the profile
//! header and "New window":
//!
//! Settings (Ctrl+,), New tab (Ctrl+T), Reopen closed tab (Ctrl+Shift+T) |
//! theme tiles System / Dark / Light | Developer › performance overlay,
//! synthetic bars | Exit (Ctrl+Shift+Q).
//!
//! TradingView's Zoom row is not here: an app-wide UI zoom needs every kit
//! size to scale (BACKLOG.md). "Open link from clipboard" has nothing to
//! open in tradeterm.

use crate::app::{LoadSynthetic, NewTab, Quit, ReopenClosedTab, TogglePerf, TradeTerm};
use crate::top_toolbar::OpenChartSettings;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use tt_session::settings::TabSettings;
use tt_theme::{Theme, ThemeMode};
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{Menu, hsla};

/// Closed tabs remembered for "Reopen closed tab".
const MAX_CLOSED_TABS: usize = 10;

/// Theme preview tile size (three fit in a large menu).
const TILE_W: f32 = 80.0;
const TILE_H: f32 = 48.0;

impl TradeTerm {
    pub(crate) fn remember_closed_tab(&mut self, tab: TabSettings) {
        self.closed_tabs.push(tab);
        if self.closed_tabs.len() > MAX_CLOSED_TABS {
            self.closed_tabs.remove(0);
        }
    }

    /// Reopens the last closed tab after the active one.
    pub(crate) fn reopen_closed_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(settings) = self.closed_tabs.pop() else {
            return;
        };
        let chart = self.make_chart(&settings, None, window, cx);
        self.active = (self.active + 1).min(self.tabs.len());
        self.tabs.insert(self.active, chart);
        self.refresh_ui_theme(cx);
        self.persist(cx);
        cx.notify();
    }
}

/// The "⋯" button and its menu.
pub fn window_menu_button(cx: &mut Context<TradeTerm>) -> impl IntoElement + use<> {
    let app = cx.entity().downgrade();
    ToolbarButton::icon("window-menu", Glyph::ELLIPSIS)
        .size(ControlSize::Sm)
        .tooltip("Menu")
        .menu_right()
        .menu(move |menu, window, cx| build(menu, &app, window, cx))
}

fn build(menu: Menu, app: &WeakEntity<TradeTerm>, window: &mut Window, cx: &App) -> Menu {
    let Some(a) = app.upgrade() else {
        return menu;
    };
    let (mode, perf, can_reopen) = {
        let a = a.read(cx);
        (a.theme_mode, a.perf, !a.closed_tabs.is_empty())
    };
    let act = |f: fn(&mut TradeTerm, &mut Window, &mut Context<TradeTerm>)| {
        let app = app.clone();
        move |window: &mut Window, cx: &mut App| {
            app.update(cx, |a, cx| f(a, window, cx)).ok();
        }
    };
    // The hexagon SVG is drawn at its natural menu size (24) but takes the
    // 16 px icon slot of the font glyphs, so every label lines up.
    let muted = hsla(cx.ui().text_muted);
    let settings_icon = move || {
        div()
            .flex_none()
            .size(ICON_SM.px())
            .flex()
            .items_center()
            .justify_center()
            .child(
                tt_ui::glyph(Glyph::APP_SETTINGS, Glyph::APP_SETTINGS.svg_size(ICON_SM))
                    .color(muted),
            )
            .into_any_element()
    };
    menu.id("window-menu-list")
        .width(MenuWidth::Lg)
        .row(
            MenuRow::new("wm-settings", "Settings")
                .leading_element(settings_icon)
                .shortcut_action(&OpenChartSettings, None, window)
                .on_select(act(TradeTerm::open_chart_settings)),
        )
        .row(
            MenuRow::new("wm-new-tab", "New tab")
                .icon(Glyph::PLUS)
                .shortcut_action(&NewTab, None, window)
                .on_select(act(TradeTerm::new_tab)),
        )
        .row(
            MenuRow::new("wm-reopen", "Reopen closed tab")
                .icon(Glyph::REOPEN)
                .disabled(!can_reopen)
                .shortcut_action(&ReopenClosedTab, None, window)
                .on_select(act(TradeTerm::reopen_closed_tab)),
        )
        .separator()
        .child(theme_tiles(mode, app.clone(), cx))
        .separator()
        .section("Developer")
        .row(
            MenuRow::new("wm-perf", "Performance overlay")
                .icon(Glyph::ACTIVITY)
                .toggle(perf)
                .shortcut_action(&TogglePerf, None, window)
                .on_select(act(|a, _, cx| {
                    a.perf = !a.perf;
                    cx.notify();
                })),
        )
        .row(
            MenuRow::new("wm-synthetic", "1M synthetic bars")
                .icon(Glyph::CHART_CANDLESTICK)
                .shortcut_action(&LoadSynthetic, None, window)
                .on_select(act(|a, _, cx| a.load_synthetic(cx))),
        )
        .separator()
        .row(
            MenuRow::new("wm-exit", "Exit tradeterm")
                .icon(Glyph::EXIT)
                .shortcut_action(&Quit, None, window)
                .on_select(|_, cx| cx.quit()),
        )
}

/// System / Dark / Light preview tiles; the current mode is outlined.
fn theme_tiles(mode: ThemeMode, app: WeakEntity<TradeTerm>, cx: &App) -> impl IntoElement {
    let t = cx.ui().clone();
    let (dark, light) = (Theme::for_dark(true), Theme::for_dark(false));
    let tile = |id: &'static str, label: &'static str, m: ThemeMode| {
        let app = app.clone();
        let selected = m == mode;
        // Half / half for System, else one palette.
        let halves: Vec<&Theme> = match m {
            ThemeMode::System => vec![&dark, &light],
            ThemeMode::Dark => vec![&dark],
            ThemeMode::Light => vec![&light],
        };
        let preview = h_flex()
            .w(TILE_W.px())
            .h(TILE_H.px())
            .rounded(R_MD.px())
            .overflow_hidden()
            .border_1()
            .border_color(hsla(t.border))
            .children(halves.into_iter().map(|p| {
                // A window: three panels on the page background.
                h_flex()
                    .flex_1()
                    .h_full()
                    .p(S1.px())
                    .gap(S0_5.px())
                    .bg(hsla(p.bg))
                    .children((0..3).map(|_| {
                        div()
                            .flex_1()
                            .h_full()
                            .rounded(R_XS.px())
                            .bg(hsla(p.panel_alt))
                    }))
            }));
        v_flex()
            .id(id)
            .items_center()
            .gap(S1.px())
            .cursor_pointer()
            .child(
                div()
                    .p(S0_5.px())
                    .rounded(R_LG.px())
                    .border_1()
                    .border_color(if selected {
                        hsla(t.text)
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .hover(|s| s.bg(hsla(t.hover)))
                    .child(preview),
            )
            .child(
                div()
                    .text_size(TEXT_DENSE.px())
                    .text_color(hsla(if selected { t.text } else { t.text_muted }))
                    .child(label),
            )
            .on_click(move |_, window, cx| {
                app.update(cx, |a, cx| a.set_theme(m, window, cx)).ok();
            })
    };
    h_flex()
        .w_full()
        .px(S2.px())
        .py(S2.px())
        .justify_between()
        .child(tile("wm-theme-system", "System", ThemeMode::System))
        .child(tile("wm-theme-dark", "Dark", ThemeMode::Dark))
        .child(tile("wm-theme-light", "Light", ThemeMode::Light))
}
