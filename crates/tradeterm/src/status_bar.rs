//! The bottom toolbar under the chart (TradingView's, docs/specs/
//! bottom-panel.md) and the F12 performance overlay.
//!
//! left: date ranges `1D 5D 1M 3M 6M YTD 1Y 5Y All` | Go to (Alt+G);
//! right: connection dot (data source; words only while not connected) |
//! the time zone clock and its zone menu.

use crate::app::{KEY_CONTEXT, TradeTerm};
use crate::source::now_ms;
use crate::ui;
use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;
use tt_chart::range::{DateRange, ViewTarget};
use tt_chart::style::TimeZoneSetting;
use tt_chart_gpui::ChartSource as _;
use tt_core::ConnectionStatus;
use tt_ui::format_keys;
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

actions!(
    tradeterm,
    [
        /// The Go to dialog (Alt+G, TradingView's).
        GoToDate,
    ]
);

/// Bottom toolbar shortcuts.
pub fn key_bindings() -> Vec<KeyBinding> {
    vec![KeyBinding::new("alt-g", GoToDate, Some(KEY_CONTEXT))]
}

/// Registers the bottom toolbar's action handlers on the app root.
pub fn on_bottom_bar_actions(root: Stateful<Div>, cx: &mut Context<TradeTerm>) -> Stateful<Div> {
    root.on_action(cx.listener(|this, _: &GoToDate, window, cx| {
        if !this.chart_has_keyboard(window, cx) {
            return cx.propagate();
        }
        this.open_go_to(window, cx);
    }))
}

/// Group separator: a short vertical hairline.
fn group_divider() -> Divider {
    Divider::vertical().length(ICON_MD).inset(S1)
}

impl TradeTerm {
    /// The bottom toolbar (was the status bar).
    pub(crate) fn status_bar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let app = cx.weak_entity();
        let clock = window.use_keyed_state("tz-clock", cx, move |_, cx| TzClock::new(app, cx));
        let Some(chart) = self.tab_chart() else {
            return div().into_any_element();
        };
        let t = cx.ui().clone();
        let state = chart.read(cx).state.read(cx);
        let tab = &state.source.tab;
        let active = tab.chart.active_range();

        let mut ranges = h_flex().gap(S0_5.px()).items_center();
        for r in DateRange::ALL {
            ranges = ranges.child(
                ToolbarButton::text(
                    SharedString::from(format!("range-{}", r.label())),
                    r.label(),
                )
                .selected(active == Some(r))
                .tooltip(r.tooltip())
                .on_click(cx.listener(move |this, _, _, cx| this.set_date_range(r, cx))),
            );
        }

        // Connection: a dot (tooltip: state and source); words only while
        // not connected.
        let connected = tab.status == ConnectionStatus::Connected;
        let (dot, text) = match &tab.status {
            ConnectionStatus::Connected => (t.up, "Connected".to_owned()),
            ConnectionStatus::Connecting => (t.warning, "Connecting…".to_owned()),
            other => (t.down, format!("{other:?}")),
        };
        let tip = SharedString::from(format!("{text} · {}", tab.source_name(&self.shared)));
        let connection = h_flex()
            .id("connection")
            .h(CONTROL_MD.px())
            .px(S2.px())
            .gap(S1_5.px())
            .items_center()
            .text_size(TEXT_BODY.px())
            .text_color(hsla(t.text_muted))
            .child(ui::status_dot(
                dot,
                tab.status == ConnectionStatus::Connecting,
            ))
            .when(!connected, |d| d.child(text))
            .tooltip(tt_ui::tip(tip, None));

        h_flex()
            .id("bottom-bar")
            .flex_none()
            .h(H_BOTTOM_BAR.px())
            .px(S1.px())
            .gap(S0_5.px())
            .items_center()
            .text_color(hsla(t.text))
            .bg(hsla(t.panel))
            .border_t_1()
            .border_color(hsla(t.border))
            .child(ranges)
            .child(group_divider())
            .child(
                IconButton::new("go-to", Glyph::GO_TO_DATE)
                    .tooltip_action("Go to", &GoToDate, Some(KEY_CONTEXT))
                    .on_click(cx.listener(|this, _, window, cx| this.open_go_to(window, cx))),
            )
            .child(div().flex_1())
            .child(connection)
            .child(group_divider())
            .child(clock)
            .into_any_element()
    }

    /// A range button: TradingView's interval for it, then its span.
    pub(crate) fn set_date_range(&mut self, r: DateRange, cx: &mut Context<Self>) {
        self.with_tab(cx, |s, shared| {
            s.tab.set_timeframe(r.timeframe(), shared);
            s.tab.chart.show_target(ViewTarget::Range(r), Some(r));
        });
    }

    /// Go to dialog result: a date or a custom range.
    pub(crate) fn go_to(&mut self, target: ViewTarget, cx: &mut Context<Self>) {
        self.with_tab(cx, |s, _| s.tab.chart.show_target(target, None));
    }

    pub(crate) fn open_go_to(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chart) = self.tab_chart() else {
            return;
        };
        let tab = &chart.read(cx).state.read(cx).source.tab;
        let tz = tab.chart.time_zone().clone();
        let visible = tab.chart.visible_time_range(&tab.series);
        let app = cx.weak_entity();
        let panel = cx.new(|cx| crate::go_to::GoTo::new(app, tz, visible, window, cx));
        crate::go_to::GoTo::open(panel, window, cx);
    }

    /// Bottom toolbar zone menu: the active tab's Symbol › Time zone (its own
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

/// The bottom toolbar's time zone button: a live clock in the active chart's
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
            - (H_TITLE_BAR + H_TOOLBAR + H_BOTTOM_BAR + S4).px())
        .max((H_MENU_ROW * 6.).px());
        ToolbarButton::text("tz-clock", text)
            .tooltip("Timezone")
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
