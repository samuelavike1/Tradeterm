//! The top toolbar: ticker, intervals, chart types, indicators.

use crate::app::{KEY_CONTEXT, OpenIndicators, TradeTerm};
use crate::ui::chart_type_icon;
use gpui_kit::component::h_flex;
use gpui_kit::*;
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

impl TradeTerm {
    pub(crate) fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(chart) = self.tab_chart() else {
            return div();
        };
        let state = chart.read(cx).state.read(cx);
        let tab = &state.source.tab;
        let (symbol, tf, chart_type) = (tab.display_symbol(), tab.timeframe, tab.chart.chart_type);

        // Favorite intervals (plus the current one) and the interval menu.
        let mut tfs = h_flex().gap(S0_5.px()).items_center();
        let mut favorites = self.favorite_timeframes.clone();
        if !favorites.contains(&tf) {
            favorites.push(tf);
        }
        favorites.sort_by_key(|t| t.duration_ms());
        for t in favorites {
            tfs = tfs.child(
                ToolbarButton::text(SharedString::from(format!("tf-{}", t.code())), t.label())
                    .selected(t == tf)
                    .tooltip(t.long_name())
                    .on_click(cx.listener(move |this, _, _, cx| this.set_timeframe(t, cx))),
            );
        }
        tfs = tfs.child(crate::menus::timeframe_menu(cx));

        // Favorite chart types as quick buttons, then the current type's menu.
        let mut chart_types = h_flex().gap(S0_5.px()).items_center();
        for fav in self.favorite_chart_types.clone() {
            if fav == chart_type {
                continue;
            }
            chart_types = chart_types.child(
                IconButton::new(
                    SharedString::from(format!("ct-fav-{}", fav.label())),
                    chart_type_icon(fav),
                )
                .tooltip(fav.label())
                .on_click(cx.listener(move |this, _, _, cx| this.set_chart_type(fav, cx))),
            );
        }
        chart_types = chart_types.child(crate::menus::chart_type_menu(chart_type, cx));

        let t = cx.ui();
        h_flex()
            .flex_none()
            .h(H_TOOLBAR.px())
            .px(S2.px())
            .gap(S1.px())
            .items_center()
            .bg(hsla(t.panel))
            .border_b_1()
            .border_color(hsla(t.border))
            .child(
                ToolbarButton::field("ticker", symbol)
                    .tooltip("Symbol search, or start typing on the chart")
                    .on_click(cx.listener(|this, _, window, cx| this.open_search("", window, cx))),
            )
            .child(Divider::vertical())
            .child(tfs)
            .child(Divider::vertical())
            .child(chart_types)
            .child(Divider::vertical())
            .child(
                ToolbarButton::text("indicators", "Indicators")
                    .leading(Glyph::SQUARE_FUNCTION)
                    .tooltip_action(
                        "Indicators, metrics and strategies",
                        &OpenIndicators,
                        Some(KEY_CONTEXT),
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.open_indicators(window, cx))),
            )
    }
}
