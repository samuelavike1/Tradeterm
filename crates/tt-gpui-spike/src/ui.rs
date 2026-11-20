//! `--ui`: the app chrome rebuilt with gpui-component, to judge how much
//! work TradingView-style UI takes compared with our hand-painted egui UI:
//! title bar with a tab, toolbar (ticker → symbol search dialog, timeframe
//! toggles, chart type dropdown, Indicators), and the chart.

use crate::{ChartPane, synthetic};
use gpui_kit::component::button::*;
use gpui_kit::component::input::*;
use gpui_kit::component::menu::*;
use gpui_kit::component::*;
use gpui_kit::*;

const TIMEFRAMES: [&str; 6] = ["1m", "5m", "15m", "1h", "4h", "D"];
const CHART_TYPES: [&str; 6] = [
    "Bars",
    "Candles",
    "Hollow candles",
    "Line",
    "Area",
    "Heikin Ashi",
];
const SYMBOLS: [&str; 16] = [
    "BTCUSDT",
    "ETHUSDT",
    "SOLUSDT",
    "BNBUSDT",
    "XRPUSDT",
    "ADAUSDT",
    "DOGEUSDT",
    "AVAXUSDT",
    "LINKUSDT",
    "DOTUSDT",
    "TRXUSDT",
    "MATICUSDT",
    "LTCUSDT",
    "BTCUSDC",
    "ETHBTC",
    "SOLBTC",
];

/// Search field + filtered results; its own view so typing re-renders it.
pub struct SearchPanel {
    input: Entity<InputState>,
    owner: WeakEntity<UiDemo>,
    _sub: Subscription,
}

impl SearchPanel {
    fn new(owner: WeakEntity<UiDemo>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Symbol, e.g. BTCUSDT"));
        let _sub = cx.subscribe(
            &input,
            |_, _, _: &gpui_kit::component::input::InputEvent, cx| cx.notify(),
        );
        Self { input, owner, _sub }
    }
}

impl Render for SearchPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let q = self.input.read(cx).value().to_uppercase();
        let theme = cx.theme().clone();
        let rows = SYMBOLS
            .iter()
            .filter(|s| q.is_empty() || s.contains(q.as_str()))
            .map(|s| {
                let owner = self.owner.clone();
                let sym: SharedString = (*s).into();
                // Highlight the matched part, like TradingView.
                let (pre, hit, post) = match s.find(q.as_str()).filter(|_| !q.is_empty()) {
                    Some(i) => (&s[..i], &s[i..i + q.len()], &s[i + q.len()..]),
                    None => (*s, "", ""),
                };
                h_flex()
                    .id(sym.clone())
                    .h(px(44.))
                    .px_3()
                    .gap_3()
                    .rounded(px(6.))
                    .hover(|st| st.bg(theme.accent))
                    .cursor_pointer()
                    .child(
                        div()
                            .size(px(28.))
                            .rounded_full()
                            .bg(rgb(0xf7931a))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(gpui_kit::white())
                            .child(sym.chars().next().unwrap_or('?').to_string()),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .child(pre.to_owned())
                            .child(div().text_color(rgb(0x2962ff)).child(hit.to_owned()))
                            .child(post.to_owned()),
                    )
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .text_sm()
                            .child("Binance"),
                    )
                    .on_click(move |_, window, cx| {
                        let sym = sym.clone();
                        owner
                            .update(cx, |me, cx| {
                                me.symbol = sym;
                                cx.notify();
                            })
                            .ok();
                        window.close_dialog(cx);
                    })
            });
        v_flex()
            .gap_2()
            .child(
                Input::new(&self.input)
                    .prefix(Icon::new(IconName::Search))
                    .cleanable(true),
            )
            .child(v_flex().max_h(px(420.)).overflow_hidden().children(rows))
    }
}

pub struct UiDemo {
    chart: Entity<ChartPane>,
    symbol: SharedString,
    timeframe: &'static str,
    chart_type: &'static str,
    search: Option<Entity<SearchPanel>>,
}

impl UiDemo {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let series = synthetic::generate_minutes(20_000, 3);
        let chart = cx.new(|_| ChartPane::new(series, "BTCUSDT · 15m · Binance", "ui", Some(200)));
        Self {
            chart,
            symbol: "BTCUSDT".into(),
            timeframe: "15m",
            chart_type: "Candles",
            search: None,
        }
    }

    fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let panel = cx.new(|cx| SearchPanel::new(owner, window, cx));
        self.search = Some(panel.clone());
        let focus = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if focus.replace(false) {
                let input = panel.read(cx).input.clone();
                input.focus_handle(cx).focus(window, cx);
            }
            dialog
                .title("Symbol Search")
                .w(px(640.))
                .child(panel.clone())
        });
    }
}

fn lucide(glyph: lucide_icons::Icon) -> Div {
    div()
        .font_family("lucide")
        .text_size(px(17.))
        .child(char::from(glyph).to_string())
}

impl Render for UiDemo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let this = cx.entity().downgrade();

        let title_bar = TitleBar::new().child(
            h_flex()
                .gap_2()
                .child(
                    h_flex()
                        .h(px(28.))
                        .px_3()
                        .gap_2()
                        .rounded(px(6.))
                        .bg(theme.secondary)
                        .child(div().size(px(16.)).rounded_full().bg(rgb(0xf7931a)))
                        .child(self.symbol.clone())
                        .child(div().text_color(rgb(0xf23645)).child("▼ -4.05%")),
                )
                .child(Button::new("new-tab").ghost().small().icon(IconName::Plus)),
        );

        let mut tf_buttons = h_flex().gap_1();
        for tf in TIMEFRAMES {
            let this = this.clone();
            tf_buttons = tf_buttons.child(
                Button::new(tf)
                    .ghost()
                    .small()
                    .label(tf)
                    .selected(self.timeframe == tf)
                    .on_click(move |_, _, cx| {
                        this.update(cx, |me, cx| {
                            me.timeframe = tf;
                            cx.notify();
                        })
                        .ok();
                    }),
            );
        }

        let ct_owner = this.clone();
        let current_ct = self.chart_type;
        let chart_type = Button::new("chart-type")
            .ghost()
            .small()
            .child(lucide(lucide_icons::Icon::ChartCandlestick))
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu.min_w(px(220.));
                for ct in CHART_TYPES {
                    let owner = ct_owner.clone();
                    menu = menu.item(PopupMenuItem::new(ct).checked(ct == current_ct).on_click(
                        move |_, _, cx| {
                            owner
                                .update(cx, |me, cx| {
                                    me.chart_type = ct;
                                    cx.notify();
                                })
                                .ok();
                        },
                    ));
                }
                menu
            });

        let search_owner = this.clone();
        let toolbar = h_flex()
            .h(px(40.))
            .px_2()
            .gap_1()
            .border_b_1()
            .border_color(theme.border)
            .child(
                Button::new("ticker")
                    .ghost()
                    .small()
                    .icon(IconName::Search)
                    .label(self.symbol.clone())
                    .on_click(move |_, window, cx| {
                        search_owner
                            .update(cx, |me, cx| me.open_search(window, cx))
                            .ok();
                    }),
            )
            .child(gpui_kit::component::separator::Separator::vertical().h(px(20.)))
            .child(tf_buttons)
            .child(gpui_kit::component::separator::Separator::vertical().h(px(20.)))
            .child(chart_type)
            .child(gpui_kit::component::separator::Separator::vertical().h(px(20.)))
            .child(
                Button::new("indicators").ghost().small().child(
                    h_flex()
                        .gap_2()
                        .child(lucide(lucide_icons::Icon::SquareFunction))
                        .child("Indicators"),
                ),
            )
            .child(div().flex_1())
            .child(Button::new("theme").ghost().small().icon(IconName::Sun))
            .child(
                Button::new("settings")
                    .ghost()
                    .small()
                    .icon(IconName::Settings),
            );

        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(title_bar)
            .child(toolbar)
            .child(div().flex_1().child(self.chart.clone()))
    }
}
