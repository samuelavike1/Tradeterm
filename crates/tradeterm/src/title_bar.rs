//! The title bar: chart tab pills, new tab, the "⋯" window menu.

use crate::app::{NewTab, TradeTerm};
use crate::ui::{badge_color, logo_or_badge, snappy};
use gpui_kit::base::transition;
use gpui_kit::component::{TitleBar, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use tt_theme::{badge_fg, with_alpha};
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

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
            let logo = self.shared.assets.logo(&badge);
            tabs = tabs.child(tab_pill(
                i,
                i == self.active,
                label,
                badge,
                logo,
                change,
                self.tabs.len() > 1,
                window,
                cx,
            ));
        }
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
                .child(crate::window_menu::window_menu_button(cx)),
        )
    }
}

/// Badge diameter: the small-icon box, so initials line up with icons.
const BADGE: f32 = ICON_SM;

/// One chart tab in the title bar: logo (or letter badge), symbol, change %, close button on
/// hover. The active tab's background eases in.
#[allow(clippy::too_many_arguments)]
pub fn tab_pill(
    index: usize,
    active: bool,
    label: String,
    badge: String,
    logo: Option<std::sync::Arc<std::path::Path>>,
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
    let letter = SharedString::from(badge.chars().next().unwrap_or('?').to_string());
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
        .child(logo_or_badge(logo, BADGE.px(), move || {
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
                .child(letter.clone())
                .into_any_element()
        }))
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
