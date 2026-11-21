//! The status bar and the F12 performance overlay.

use crate::app::TradeTerm;
use crate::ui;
use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::*;
use tt_chart_gpui::ChartSource as _;
use tt_core::ConnectionStatus;
use tt_ui::format_keys;
use tt_ui::prelude::*;
use tt_ui::hsla;
use tt_ui::tokens::*;

/// Status bar group separator: a short vertical hairline.
fn group_divider() -> Divider {
    Divider::vertical().length(ICON_XS).inset(S1_5)
}

impl TradeTerm {
    pub(crate) fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(chart) = self.tab_chart() else {
            return div();
        };
        let t = cx.ui().clone();
        let state = chart.read(cx).state.read(cx);
        let tab = &state.source.tab;
        let (dot, text) = match &tab.status {
            ConnectionStatus::Connected => (t.up, "Connected".to_owned()),
            ConnectionStatus::Connecting => (t.warning, "Connecting…".to_owned()),
            other => (t.down, format!("{other:?}")),
        };
        let muted = hsla(t.text_muted);
        let pair = |k: &'static str, v: AnyElement| {
            h_flex()
                .gap(S1.px())
                .child(div().text_color(muted).child(k))
                .child(v)
        };
        let mut bar = h_flex()
            .h(H_STATUS_BAR.px())
            .px(S2.px())
            .gap(S1.px())
            .items_center()
            .text_size(TEXT_SMALL.px())
            .text_color(hsla(t.text))
            .bg(hsla(t.panel))
            .border_t_1()
            .border_color(hsla(t.border))
            .child(
                h_flex()
                    .gap(S1_5.px())
                    .items_center()
                    .child(ui::status_dot(
                        dot,
                        tab.status == ConnectionStatus::Connecting,
                    ))
                    .child(text),
            );
        if let Some(c) = tab.series.last() {
            let color = if c.is_bullish() { t.up } else { t.down };
            let price = tt_chart::scale::format_price(c.close, tab.price_precision());
            bar = bar.child(group_divider()).child(pair(
                "Last",
                div()
                    .text_color(hsla(color))
                    .child(price)
                    .into_any_element(),
            ));
        }
        bar = bar.child(group_divider()).child(pair(
            "Bars",
            div()
                .child(tab.series.len().to_string())
                .into_any_element(),
        ));
        bar = bar.child(div().flex_1());
        if let Some(i) = tab.symbol_info.as_ref() {
            bar = bar
                .child(
                    div()
                        .text_color(muted)
                        .child(format!("{} / {}", i.base, i.quote)),
                )
                .child(group_divider())
                .child(pair(
                    "Tick",
                    div()
                        .child(i.tick_size.to_string())
                        .into_any_element(),
                ))
                .child(group_divider());
        }
        bar.child(
            div()
                .text_color(muted)
                .child(format!("{} · UTC", tab.source_name(&self.shared))),
        )
    }

    pub(crate) fn perf_overlay(&self, cx: &App) -> Option<impl IntoElement + use<>> {
        if !self.perf {
            return None;
        }
        let chart = self.tab_chart()?;
        let s = chart.read(cx).state.read(cx);
        let (bn, bms, on, oms, events) = self.perf_stats.last;
        let muted = hsla(cx.ui().text_muted);
        let line = |k: &str, v: String| {
            h_flex()
                .gap(S3.px())
                .child(div().w(W_FIELD_NUMBER.px()).text_color(muted).child(k.to_owned()))
                .child(v)
        };
        // Top right of the plot, clear of the price axis and below the
        // pane-controls row (§2.7).
        let axis_w = s.source.view().last_layout().price_axis.width();
        Some(
            Overlay::new()
                .absolute()
                .top((S2 + CONTROL_SM + S2).px())
                .right((axis_w + S2).px())
                .font_family(cx.theme().mono_font_family.clone())
                .child(
                    h_flex()
                        .pb(S1.px())
                        .justify_between()
                        .child(div().font_weight(FontWeight::MEDIUM).child("Performance"))
                        .child(div().text_color(muted).child(format_keys("f12"))),
                )
                .child(line(
                    "Base layer",
                    format!("{bn:>3}/s  {bms:5.2} ms  {} prims", s.base_stats.primitives),
                ))
                .child(line(
                    "Overlay",
                    format!(
                        "{on:>3}/s  {oms:5.2} ms  {} prims",
                        s.overlay_stats.primitives
                    ),
                ))
                .child(line("Data events", format!("{events}/s")))
                .child(line("Bars", s.source.tab.series.len().to_string())),
        )
    }
}
