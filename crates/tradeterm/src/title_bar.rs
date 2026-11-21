//! The title bar: chart tab pills, new tab, theme toggle, settings menu.

use crate::app::{LoadSynthetic, NewTab, TogglePerf, ToggleTheme, TradeTerm};
use crate::ui::{badge_color, snappy};
use gpui_kit::base::transition;
use gpui_kit::component::{TitleBar, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use tt_theme::{ThemeMode, badge_fg, with_alpha};
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{Menu, hsla};

impl TradeTerm {
    pub(crate) fn title_bar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut tabs = h_flex().gap(S1.px()).h_full().items_center();
        for (i, chart) in self.tabs.iter().enumerate() {
            let s = &chart.read(cx).state.read(cx).source.tab;
            let (label, badge, change) = (s.display_symbol(), s.base_asset(), s.change_pct());
            tabs = tabs.child(tab_pill(
                i,
                i == self.active,
                label,
                badge,
                change,
                self.tabs.len() > 1,
                window,
                cx,
            ));
        }
        let this = cx.entity().downgrade();
        let dark = self.theme.dark;
        TitleBar::new().child(
            h_flex()
                .w_full()
                .h_full()
                .pr(S2.px())
                .justify_between()
                .child(
                    tabs.child(
                        IconButton::new("new-tab", Glyph::PLUS)
                            .size(ControlSize::Sm)
                            .tooltip_action("New tab", &NewTab, None)
                            .on_click(cx.listener(|this, _, window, cx| this.new_tab(window, cx))),
                    ),
                )
                .child(
                    h_flex()
                        .gap(S1.px())
                        .items_center()
                        .child(
                            IconButton::new("theme", if dark { Glyph::SUN } else { Glyph::MOON })
                                .size(ControlSize::Sm)
                                .tooltip_action("Toggle theme", &ToggleTheme, None)
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.toggle_theme(window, cx)
                                    }),
                                ),
                        )
                        .child(
                            ToolbarButton::icon("settings", Glyph::SETTINGS)
                                .size(ControlSize::Sm)
                                .tooltip("Settings")
                                .menu(move |menu, window, cx| {
                                    settings_menu(menu, &this, window, cx)
                                }),
                        ),
                ),
        )
    }
}

/// The gear menu: theme (checked), chart settings, developer tools.
fn settings_menu(menu: Menu, app: &WeakEntity<TradeTerm>, window: &Window, cx: &App) -> Menu {
    let Some(a) = app.upgrade() else {
        return menu;
    };
    let (mode, perf) = {
        let a = a.read(cx);
        (a.theme_mode, a.perf)
    };
    let theme_row = |id: &'static str, label: &'static str, m: ThemeMode| {
        let app = app.clone();
        MenuRow::new(id, label)
            .checked(mode == m)
            .on_select(move |window, cx| {
                app.update(cx, |a, cx| a.set_theme(m, window, cx)).ok();
            })
    };
    let (settings, synth, perf_app) = (app.clone(), app.clone(), app.clone());
    menu.width(MenuWidth::Lg)
        .section("Theme")
        .row(theme_row("theme-dark", "Dark", ThemeMode::Dark))
        .row(theme_row("theme-light", "Light", ThemeMode::Light))
        .row(theme_row("theme-system", "System", ThemeMode::System))
        .separator()
        .row(
            MenuRow::new("chart-settings", "Chart settings…").on_select(move |window, cx| {
                settings
                    .update(cx, |a, cx| a.open_chart_settings(window, cx))
                    .ok();
            }),
        )
        .separator()
        .section("Developer")
        .row(
            MenuRow::new("perf", "Performance overlay")
                .checked(perf)
                .shortcut_action(&TogglePerf, None, window)
                .on_select(move |_, cx| {
                    perf_app
                        .update(cx, |a, cx| {
                            a.perf = !a.perf;
                            cx.notify();
                        })
                        .ok();
                }),
        )
        .row(
            MenuRow::new("synthetic", "Load 1M synthetic bars")
                .shortcut_action(&LoadSynthetic, None, window)
                .on_select(move |_, cx| {
                    synth.update(cx, |a, cx| a.load_synthetic(cx)).ok();
                }),
        )
}

/// Badge diameter: the small-icon box, so initials line up with icons.
const BADGE: f32 = ICON_SM;

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
    window: &mut Window,
    cx: &mut Context<TradeTerm>,
) -> impl IntoElement {
    let fill = transition(
        SharedString::from(format!("tab-fill-{index}")),
        if active { 1.0f32 } else { 0.0 },
        snappy(MOTION_NORMAL),
        window,
        cx,
    );
    let t = cx.ui().clone();
    let badge_bg = badge_color(&t, &badge);
    let group = SharedString::from(format!("tab-{index}"));
    let app = cx.entity().downgrade();
    h_flex()
        .id(("tab", index))
        .group(group.clone())
        .h(H_TAB.px())
        .pl(S2.px())
        .pr(S1.px())
        .gap(S1_5.px())
        .items_center()
        .rounded(R_MD.px())
        .bg(hsla(with_alpha(t.tab_active, fill)))
        .when(!active, |d| d.hover(|s| s.bg(hsla(t.hover))))
        .cursor_pointer()
        .text_size(TEXT_DENSE.px())
        .text_color(hsla(t.text))
        .child(
            div()
                .flex_none()
                .size(BADGE.px())
                .rounded_full()
                .bg(hsla(badge_bg))
                .flex()
                .items_center()
                .justify_center()
                .text_size(TEXT_CAPTION.px())
                .line_height(TEXT_CAPTION.px())
                .font_weight(FontWeight::MEDIUM)
                .text_color(hsla(badge_fg(badge_bg)))
                .child(badge.chars().next().unwrap_or('?').to_string()),
        )
        .child(div().whitespace_nowrap().child(label))
        .children(change.map(|pct| {
            let up = pct >= 0.0;
            h_flex()
                .items_center()
                .gap(S0_5.px())
                .text_size(TEXT_SMALL.px())
                .text_color(hsla(if up { t.up } else { t.down }))
                .child(
                    // The triangle in a caption-sized box, so its baseline
                    // matches the figure next to it.
                    div()
                        .flex_none()
                        .size(TEXT_CAPTION.px())
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(TEXT_CAPTION.px())
                        .line_height(TEXT_CAPTION.px())
                        .child(if up { "▲" } else { "▼" }),
                )
                .child(format!("{:.2}%", pct.abs()))
        }))
        .child(
            div()
                .flex_none()
                .invisible()
                .when(closable, |d| d.group_hover(group, |s| s.visible()))
                .child(
                    IconButton::new(("tab-close", index), Glyph::X)
                        .size(ControlSize::Xs)
                        .tooltip("Close tab")
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            app.update(cx, |a, cx| a.on_tab_click(index, true, cx)).ok();
                        }),
                ),
        )
        .on_click(cx.listener(move |this, _, _, cx| this.on_tab_click(index, false, cx)))
        .on_mouse_down(
            MouseButton::Middle,
            cx.listener(move |this, _, _, cx| this.on_tab_click(index, true, cx)),
        )
}
