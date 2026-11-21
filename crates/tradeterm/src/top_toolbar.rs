//! The top toolbar: ticker, intervals, chart types, indicators.

use crate::app::TradeTerm;
use crate::ui::{self, lucide};
use gpui_kit::component::Selectable as _;
use gpui_kit::component::button::*;
use gpui_kit::component::{IconName, Sizable, h_flex};
use gpui_kit::*;
use tt_chart_gpui::hsla;

impl TradeTerm {
    pub(crate) fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(chart) = self.tab_chart() else {
            return div();
        };
        let state = chart.read(cx).state.read(cx);
        let tab = &state.source.tab;
        let (symbol, tf, chart_type) = (tab.display_symbol(), tab.timeframe, tab.chart.chart_type);

        let mut tfs = h_flex().gap_0p5();
        let mut favorites = self.favorite_timeframes.clone();
        if !favorites.contains(&tf) {
            favorites.push(tf);
        }
        favorites.sort_by_key(|t| t.duration_ms());
        for t in favorites {
            tfs = tfs.child(
                Button::new(SharedString::from(format!("tf-{}", t.code())))
                    .ghost()
                    .small()
                    .label(t.label())
                    .selected(t == tf)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_timeframe(t, cx))),
            );
        }
        let tf_menu = crate::menus::timeframe_menu(cx);
        // Favorite chart types as quick buttons, then the current type's menu.
        let mut chart_types = h_flex().gap_0p5();
        for fav in self.favorite_chart_types.clone() {
            if fav == chart_type {
                continue;
            }
            chart_types = chart_types.child(
                Button::new(SharedString::from(format!("ct-fav-{}", fav.label())))
                    .ghost()
                    .small()
                    .child(lucide(ui::chart_type_icon(fav), 17.))
                    .tooltip(fav.label())
                    .on_click(cx.listener(move |this, _, _, cx| this.set_chart_type(fav, cx))),
            );
        }
        let ct_menu = chart_types.child(crate::menus::chart_type_menu(chart_type, cx));

        h_flex()
            .h(px(40.))
            .px_2()
            .gap_1()
            .items_center()
            .border_b_1()
            .border_color(hsla(self.theme.border))
            .child(
                Button::new("ticker")
                    .ghost()
                    .small()
                    .icon(IconName::Search)
                    .label(symbol)
                    .tooltip("Symbol search (or just start typing)")
                    .on_click(cx.listener(|this, _, window, cx| this.open_search("", window, cx))),
            )
            .child(ui::separator(&self.theme))
            .child(tfs)
            .child(tf_menu)
            .child(ui::separator(&self.theme))
            .child(ct_menu)
            .child(ui::separator(&self.theme))
            .child(
                Button::new("indicators")
                    .ghost()
                    .small()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .child(lucide(lucide_icons::Icon::SquareFunction, 17.))
                            .child("Indicators"),
                    )
                    .tooltip("Indicators, metrics, and strategies (/)")
                    .on_click(cx.listener(|this, _, window, cx| this.open_indicators(window, cx))),
            )
            .child(div().flex_1())
    }
}
