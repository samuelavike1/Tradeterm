//! The title bar: chart tab pills, new tab, theme toggle, settings menu.

use crate::app::{ToggleTheme, TradeTerm};
use crate::ui::{badge_color, lucide, snappy};
use gpui_kit::base::transition;
use gpui_kit::component::button::*;
use gpui_kit::component::menu::*;
use gpui_kit::component::{ActiveTheme, IconName, Sizable, TitleBar, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use tt_chart_gpui::hsla;
use tt_theme::{Theme, ThemeMode, mix, with_alpha};

impl TradeTerm {
    pub(crate) fn title_bar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut tabs = h_flex().gap_1().h_full().items_center();
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
                &self.theme,
                window,
                cx,
            ));
        }
        let this = cx.entity().downgrade();
        let dark = self.theme.dark;
        let mode = self.theme_mode;
        TitleBar::new().child(
            h_flex()
                .w_full()
                .h_full()
                .pr_2()
                .justify_between()
                .child(
                    tabs.child(
                        Button::new("new-tab")
                            .ghost()
                            .small()
                            .icon(IconName::Plus)
                            .tooltip("New tab (Ctrl+T)")
                            .on_click(cx.listener(|this, _, window, cx| this.new_tab(window, cx))),
                    ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            // Kit button (slice 0 proof): font glyph, token
                            // colors, managed tooltip with the bound key.
                            tt_ui::IconButton::new(
                                "theme",
                                if dark {
                                    tt_ui::Glyph::SUN
                                } else {
                                    tt_ui::Glyph::MOON
                                },
                            )
                            .size(tt_ui::ControlSize::Sm)
                            .tooltip_action("Toggle theme", &ToggleTheme, None)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.toggle_theme(window, cx)),
                            ),
                        )
                        .child(
                            Button::new("settings")
                                .ghost()
                                .small()
                                .icon(IconName::Settings)
                                .dropdown_menu(move |menu, _, _| {
                                    let item = |label: &'static str, m: ThemeMode| {
                                        let this = this.clone();
                                        PopupMenuItem::new(label).checked(mode == m).on_click(
                                            move |_, window, cx| {
                                                this.update(cx, |app, cx| {
                                                    app.set_theme(m, window, cx)
                                                })
                                                .ok();
                                            },
                                        )
                                    };
                                    let synth = this.clone();
                                    let colors = this.clone();
                                    menu.min_w(px(240.))
                                        .label("Theme")
                                        .item(item("Dark", ThemeMode::Dark))
                                        .item(item("Light", ThemeMode::Light))
                                        .item(item("System", ThemeMode::System))
                                        .separator()
                                        .item(PopupMenuItem::new("Chart settings…").on_click(
                                            move |_, window, cx| {
                                                colors
                                                    .update(cx, |app, cx| {
                                                        app.open_chart_settings(window, cx)
                                                    })
                                                    .ok();
                                            },
                                        ))
                                        .item(
                                            PopupMenuItem::new("Load 1M synthetic bars").on_click(
                                                move |_, _, cx| {
                                                    synth
                                                        .update(cx, |app, cx| {
                                                            app.load_synthetic(cx)
                                                        })
                                                        .ok();
                                                },
                                            ),
                                        )
                                }),
                        ),
                ),
        )
    }
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
