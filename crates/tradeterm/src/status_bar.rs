//! The status bar and the F12 performance overlay.

use crate::app::TradeTerm;
use crate::ui;
use ecolor::Color32;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use tt_chart_gpui::hsla;
use tt_core::ConnectionStatus;

impl TradeTerm {
    pub(crate) fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &self.theme;
        let Some(chart) = self.tab_chart() else {
            return div();
        };
        let state = chart.read(cx).state.read(cx);
        let tab = &state.source.tab;
        let (dot, text) = match &tab.status {
            ConnectionStatus::Connected => (t.up, "Connected".to_owned()),
            ConnectionStatus::Connecting => (
                Color32::from_rgb(0xf5, 0xb5, 0x2e),
                "Connecting…".to_owned(),
            ),
            other => (t.down, format!("{other:?}")),
        };
        let last = tab.series.last();
        let precision = tab.price_precision();
        let muted = hsla(t.text_muted);
        let mut bar = h_flex()
            .h(px(28.))
            .px_3()
            .gap_4()
            .items_center()
            .text_size(px(12.5))
            .border_t_1()
            .border_color(hsla(t.border))
            .child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .child(ui::status_dot(
                        dot,
                        tab.status == ConnectionStatus::Connecting,
                    ))
                    .child(text),
            );
        if let Some(c) = last {
            let color = if c.is_bullish() { t.up } else { t.down };
            bar = bar.child(
                h_flex()
                    .gap_1()
                    .child(div().text_color(muted).child("Last"))
                    .child(
                        div()
                            .text_color(hsla(color))
                            .child(tt_chart::scale::format_price(c.close, precision)),
                    ),
            );
        }
        bar = bar.child(
            h_flex()
                .gap_1()
                .child(div().text_color(muted).child("Bars"))
                .child(tab.series.len().to_string()),
        );
        let info = tab
            .symbol_info
            .as_ref()
            .map(|i| format!("{} / {}  tick {}", i.base, i.quote, i.tick_size))
            .unwrap_or_default();
        bar.child(div().flex_1())
            .child(div().text_color(muted).child(info))
            .child(
                div()
                    .text_color(muted)
                    .child(format!("{} · UTC", tab.source_name(&self.shared))),
            )
    }

    pub(crate) fn perf_overlay(&self, cx: &App) -> Option<Div> {
        if !self.perf {
            return None;
        }
        let chart = self.tab_chart()?;
        let s = chart.read(cx).state.read(cx);
        let (bn, bms, on, oms, events) = self.perf_stats.last;
        let t = &self.theme;
        let line = |k: &str, v: String| {
            h_flex()
                .gap_3()
                .child(
                    div()
                        .w(px(110.))
                        .text_color(hsla(t.text_muted))
                        .child(k.to_owned()),
                )
                .child(v)
        };
        Some(
            v_flex()
                .absolute()
                .top(px(36.))
                .right(px(90.))
                .p_3()
                .gap_0p5()
                .rounded(px(8.))
                .bg(hsla(tt_theme::with_alpha(t.panel, 0.94)))
                .border_1()
                .border_color(hsla(t.border))
                .text_size(px(12.))
                .font_family("Consolas")
                .child(div().pb_1().child("Performance (F12)"))
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
