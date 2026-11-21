//! The status bar and the F12 performance overlay.

use crate::app::TradeTerm;
use crate::source::now_ms;
use crate::ui;
use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::*;
use std::time::Duration;
use tt_chart::style::TimeZoneSetting;
use tt_chart_gpui::ChartSource as _;
use tt_core::ConnectionStatus;
use tt_ui::format_keys;
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

/// Status bar group separator: a short vertical hairline.
fn group_divider() -> Divider {
    Divider::vertical().length(ICON_XS).inset(S1_5)
}

impl TradeTerm {
    pub(crate) fn status_bar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let app = cx.weak_entity();
        let clock = window.use_keyed_state("tz-clock", cx, move |_, cx| TzClock::new(app, cx));
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
            div().child(tab.series.len().to_string()).into_any_element(),
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
                    div().child(i.tick_size.to_string()).into_any_element(),
                ))
                .child(group_divider());
        }
        bar.child(div().text_color(muted).child(tab.source_name(&self.shared)))
            .child(group_divider())
            .child(clock)
    }

    /// Status-bar zone menu: the active tab's Symbol › Time zone (its own
    /// style, or the default when the result equals it).
    pub(crate) fn set_time_zone(&mut self, zone: TimeZoneSetting, cx: &mut Context<Self>) {
        let Some((tab, _, _, mut style)) = self.active_tab_style(cx) else {
            return;
        };
        if style.symbol.data.time_zone != zone {
            style.symbol.data.time_zone = zone;
            self.commit_tab_style(tab, style, cx);
        }
    }

    /// Clock text in the active tab's zone and that tab's zone setting.
    fn clock(&self, cx: &App) -> Option<(String, TimeZoneSetting, Option<String>)> {
        let tab = &self.tab_chart()?.read(cx).state.read(cx).source.tab;
        let text = tt_session::tz::clock_text(tab.chart.time_zone(), now_ms());
        let exchange = tab.symbol_info.as_ref().map(|i| i.timezone.clone());
        Some((
            text,
            tab.chart.style.symbol.data.time_zone.clone(),
            exchange,
        ))
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
                .child(
                    div()
                        .w(W_FIELD_NUMBER.px())
                        .text_color(muted)
                        .child(k.to_owned()),
                )
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

/// The status bar's time zone button: a live clock in the active chart's
/// zone (`14:05:33 UTC+1`, like TradingView's) that opens the zone menu.
/// Its own view, so the once-a-second tick repaints only this button.
pub(crate) struct TzClock {
    app: WeakEntity<TradeTerm>,
}

impl TzClock {
    fn new(app: WeakEntity<TradeTerm>, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                // Wake just after each second boundary so the seconds tick evenly.
                let wait = 1000 - now_ms().rem_euclid(1000) as u64 + 5;
                cx.background_executor()
                    .timer(Duration::from_millis(wait))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self { app }
    }
}

/// Section of a zone id in the menu (its region).
fn region(id: &str) -> &'static str {
    match id.split('/').next().unwrap_or("") {
        "Africa" => "AFRICA",
        "America" | "US" => "AMERICAS",
        "Asia" => "ASIA",
        "Atlantic" => "ATLANTIC",
        "Australia" => "AUSTRALIA",
        "Europe" => "EUROPE",
        "Pacific" => "PACIFIC",
        _ => "OTHER",
    }
}

impl Render for TzClock {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = self.app.upgrade() else {
            return div().into_any_element();
        };
        let Some((text, _, _)) = app.read(cx).clock(cx) else {
            return div().into_any_element();
        };
        let weak = self.app.clone();
        let max_h = (window.viewport_size().height
            - (H_TITLE_BAR + H_TOOLBAR + H_STATUS_BAR + S4).px())
        .max((H_MENU_ROW * 6.).px());
        ToolbarButton::text("tz-clock", text)
            .size(ControlSize::Sm)
            .tooltip("Time zone")
            .menu_anchor(Anchor::BottomRight)
            .menu(move |menu, _, cx| {
                let Some(app) = weak.upgrade() else {
                    return menu;
                };
                let Some((_, current, exchange)) = app.read(cx).clock(cx) else {
                    return menu;
                };
                let mut menu = menu.id("tz-menu").width(MenuWidth::Md).max_h(max_h);
                let mut section = "";
                for (i, (zone, label)) in tt_session::tz::choices(exchange.as_deref(), now_ms())
                    .into_iter()
                    .enumerate()
                {
                    if let TimeZoneSetting::Iana(id) = &zone {
                        let r = region(id);
                        if r != section {
                            section = r;
                            menu = menu.section(r);
                        }
                    } else if i == 0 {
                        menu = menu.section("TIME ZONE");
                    }
                    let checked = zone == current;
                    let app = weak.clone();
                    menu = menu.row(
                        MenuRow::new(SharedString::from(format!("tz-{i}")), label)
                            .checked(checked)
                            .on_select(move |_, cx| {
                                app.update(cx, |a, cx| a.set_time_zone(zone.clone(), cx))
                                    .ok();
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }
}
