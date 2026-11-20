//! Symbol search dialog, TradingView-style: a search field, quote filter
//! chips, results with the match highlighted, recent symbols when empty.
//! ↑/↓ move the selection, Enter opens it (if a search is still running, the
//! top result once it arrives), Esc closes.

use crate::app::TradeTerm;
use crate::ui::badge_color;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, IconName, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::ops::Range;
use std::sync::Arc;
use tt_brokers::{BinanceSource, MarketDataSource};
use tt_chart_gpui::hsla;
use tt_core::{Symbol, SymbolInfo};
use tt_theme::Theme;

const QUOTES: [&str; 7] = ["All", "USDT", "USDC", "FDUSD", "BTC", "ETH", "BNB"];

pub struct SymbolSearch {
    app: WeakEntity<TradeTerm>,
    input: Entity<InputState>,
    source: Arc<BinanceSource>,
    handle: tokio::runtime::Handle,
    theme: Theme,
    recent: Vec<SymbolInfo>,
    query: String,
    /// Results of `results_for`; replaced when a newer search finishes.
    results: Vec<SymbolInfo>,
    results_for: String,
    searching: bool,
    error: Option<String>,
    quote: usize,
    selected: Option<usize>,
    /// Enter was pressed while a search was in flight.
    enter_pending: bool,
    scroll: ScrollHandle,
    _sub: Subscription,
}

impl SymbolSearch {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app: WeakEntity<TradeTerm>,
        source: Arc<BinanceSource>,
        handle: tokio::runtime::Handle,
        theme: Theme,
        recent: Vec<SymbolInfo>,
        initial: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Symbol, e.g. BTCUSDT"));
        let _sub = cx.subscribe_in(
            &input,
            window,
            |this, input, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => {
                    let raw = input.read(cx).value().to_string();
                    let norm: String = raw
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .collect::<String>()
                        .to_ascii_uppercase();
                    if norm != raw {
                        input.update(cx, |s, cx| s.set_value(norm.clone(), window, cx));
                    }
                    this.set_query(norm, cx);
                }
                InputEvent::PressEnter { .. } => this.enter(window, cx),
                _ => {}
            },
        );
        let mut this = Self {
            app,
            input,
            source,
            handle,
            theme,
            recent,
            query: String::new(),
            results: Vec::new(),
            results_for: String::new(),
            searching: false,
            error: None,
            quote: 0,
            selected: None,
            enter_pending: false,
            scroll: ScrollHandle::new(),
            _sub,
        };
        if !initial.is_empty() {
            let text = initial.to_ascii_uppercase();
            this.input
                .update(cx, |s, cx| s.set_value(text.clone(), window, cx));
            this.set_query(text, cx);
        }
        this
    }

    pub fn focus_input(this: &Entity<Self>, window: &mut Window, cx: &mut App) {
        let input = this.read(cx).input.clone();
        let handle = input.read(cx).focus_handle(cx);
        handle.focus(window, cx);
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        if query == self.query {
            return;
        }
        self.query = query.clone();
        self.selected = None;
        self.enter_pending = false;
        if query.is_empty() {
            self.searching = false;
            cx.notify();
            return;
        }
        self.searching = true;
        let (source, q) = (self.source.clone(), query.clone());
        let task = self
            .handle
            .spawn(async move { source.search_symbols(&q).await });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.query != query {
                    return; // a newer search is running
                }
                this.searching = false;
                match result {
                    Ok(Ok(rows)) => {
                        this.results = rows;
                        this.results_for = query;
                        this.error = None;
                    }
                    Ok(Err(e)) => this.error = Some(format!("{e:#}")),
                    Err(e) => this.error = Some(e.to_string()),
                }
                if std::mem::take(&mut this.enter_pending) {
                    this.choose_top(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Rows shown: recents when empty, else results filtered by quote chip.
    fn rows(&self) -> Vec<&SymbolInfo> {
        let base: Box<dyn Iterator<Item = &SymbolInfo>> = if self.query.is_empty() {
            Box::new(self.recent.iter())
        } else {
            Box::new(self.results.iter())
        };
        let quote = QUOTES[self.quote];
        base.filter(|s| quote == "All" || s.quote == quote)
            .collect()
    }

    fn move_selection(&mut self, down: bool, cx: &mut Context<Self>) {
        let n = self.rows().len();
        if n == 0 {
            return;
        }
        let last = n - 1;
        self.selected = Some(match (self.selected, down) {
            (None, true) => 0,
            (None, false) => last,
            (Some(i), true) => (i + 1).min(last),
            (Some(i), false) => i.saturating_sub(1),
        });
        if let Some(i) = self.selected {
            self.scroll.scroll_to_item(i);
        }
        cx.notify();
    }

    fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        if self.searching && self.selected.is_none() {
            // Like TradingView: open the top result once it arrives.
            self.enter_pending = true;
            return;
        }
        match self.selected {
            Some(i) => {
                if let Some(s) = self.rows().get(i).map(|s| (*s).clone()) {
                    self.choose(s, cx);
                }
            }
            None => self.choose_top(cx),
        }
    }

    fn choose_top(&mut self, cx: &mut Context<Self>) {
        let rows = self.rows();
        // An exact symbol match wins over the ranked top result.
        let pick = rows
            .iter()
            .find(|s| s.symbol.to_string() == self.query)
            .or(rows.first())
            .map(|s| (*s).clone());
        if let Some(s) = pick {
            self.choose(s, cx);
        }
    }

    fn choose(&mut self, info: SymbolInfo, cx: &mut Context<Self>) {
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |app, cx| {
                app.set_symbol(info.symbol.clone(), Some(info), cx)
            })
            .ok();
            if let Some(window) = cx.active_window() {
                window
                    .update(cx, |_, window, cx| window.close_dialog(cx))
                    .ok();
            }
        });
    }
}

/// Byte ranges of case-insensitive matches of `needle` in `text`.
pub fn match_ranges(text: &str, needle: &str) -> Vec<Range<usize>> {
    if needle.is_empty() {
        return Vec::new();
    }
    let (hay, needle) = (text.to_ascii_uppercase(), needle.to_ascii_uppercase());
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(off) = hay[pos..].find(&needle) {
        let start = pos + off;
        out.push(start..start + needle.len());
        pos = start + needle.len();
    }
    out
}

fn highlighted(text: String, needle: &str, accent: Hsla) -> StyledText {
    let ranges = match_ranges(&text, needle);
    StyledText::new(text).with_highlights(ranges.into_iter().map(|r| {
        (
            r,
            HighlightStyle {
                color: Some(accent),
                ..Default::default()
            },
        )
    }))
}

impl Render for SymbolSearch {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme.clone();
        let accent = hsla(t.series_palette[0]);
        let muted = hsla(t.text_muted);
        let rows: Vec<SymbolInfo> = self.rows().into_iter().cloned().collect();
        let needle = self.query.clone();

        let mut chips = h_flex().gap_1p5().py_1();
        for (i, q) in QUOTES.iter().enumerate() {
            let selected = i == self.quote;
            chips = chips.child(
                div()
                    .id(("quote-chip", i))
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .text_size(px(12.5))
                    .cursor_pointer()
                    .bg(hsla(if selected { t.text } else { t.selection }))
                    .text_color(hsla(if selected { t.bg } else { t.text }))
                    .hover(|s| s.opacity(0.85))
                    .child(*q)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.quote = i;
                        this.selected = None;
                        cx.notify();
                    })),
            );
        }

        let mut list = v_flex()
            .id("search-results")
            .h(px(420.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll);
        if let Some(e) = &self.error {
            list = list.child(
                div()
                    .p_4()
                    .text_color(muted)
                    .child(format!("Search failed: {e}")),
            );
        } else if rows.is_empty() {
            let msg = if self.query.is_empty() {
                "Type a symbol or asset, e.g. BTC, ETHUSDT, SOL"
            } else if self.searching || self.results_for != self.query {
                "Searching… (the first search loads the symbol list)"
            } else {
                "No symbols match your criteria"
            };
            list = list.child(div().p_6().text_color(muted).child(msg));
        } else if self.query.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_1()
                    .text_size(px(12.))
                    .text_color(muted)
                    .child("Recent"),
            );
        }
        for (i, s) in rows.into_iter().enumerate() {
            let selected = self.selected == Some(i);
            let key = if s.base.is_empty() {
                s.symbol.to_string()
            } else {
                s.base.clone()
            };
            let initial = key.chars().next().unwrap_or('?').to_string();
            let info = s.clone();
            list = list.child(
                h_flex()
                    .id(("result", i))
                    .h(px(48.))
                    .px_3()
                    .gap_3()
                    .flex_none()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(selected, |d| d.bg(hsla(t.selection)))
                    .hover(|d| d.bg(hsla(t.selection)))
                    .child(
                        div()
                            .size(px(30.))
                            .flex_none()
                            .rounded_full()
                            .bg(hsla(badge_color(&t, &key)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(13.))
                            .text_color(hsla(t.text_on_accent))
                            .child(initial),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .child(div().text_size(px(14.5)).child(highlighted(
                                s.symbol.to_string(),
                                &needle,
                                accent,
                            )))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .text_size(px(12.))
                                    .text_color(muted)
                                    .child(highlighted(
                                        format!("{} / {}", s.base, s.quote),
                                        &needle,
                                        accent,
                                    ))
                                    .child(
                                        div()
                                            .px_1()
                                            .rounded(px(3.))
                                            .bg(hsla(t.border))
                                            .text_size(px(9.5))
                                            .child("SPOT"),
                                    ),
                            ),
                    )
                    .child(div().text_size(px(12.5)).text_color(muted).child("Binance"))
                    .on_click(cx.listener(move |this, _, _, cx| this.choose(info.clone(), cx))),
            );
        }

        v_flex()
            .gap_2()
            .capture_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                match e.keystroke.key.as_str() {
                    "up" => this.move_selection(false, cx),
                    "down" => this.move_selection(true, cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                Input::new(&self.input)
                    .prefix(Icon::new(IconName::Search))
                    .cleanable(true),
            )
            .child(chips)
            .child(list)
    }
}

/// Recent symbols as search rows.
pub fn recent_rows(recent: &[tt_session::settings::RecentSymbol]) -> Vec<SymbolInfo> {
    recent
        .iter()
        .map(|r| SymbolInfo {
            base: r.base.clone(),
            quote: r.quote.clone(),
            ..SymbolInfo::fallback(Symbol::new(&r.symbol))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn match_ranges_are_case_insensitive_and_repeat() {
        assert_eq!(match_ranges("BTCUSDT", "usd"), vec![3..6usize]);
        assert_eq!(match_ranges("ETH / ETH", "eth"), [0..3, 6..9]);
        assert!(match_ranges("BTC", "").is_empty());
    }
}
