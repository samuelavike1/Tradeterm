//! Symbol search dialog, TradingView-style: a search field, quote filter
//! chips, results with the match highlighted, recent symbols when empty.
//! ↑/↓ and PageUp/PageDown move the selection (Home/End too once a row is
//! selected) and preview it in the field, Enter opens it (if a search is
//! still running, the top result once it arrives), Esc closes.

use crate::app::TradeTerm;
use crate::ui::{badge_color, logo_or_badge};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::ops::Range;
use std::sync::Arc;
use tt_brokers::{BinanceSource, MarketDataSource};
use tt_core::{Symbol, SymbolInfo};
use tt_session::assets::AssetCache;
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

/// Logo / badge diameter in result rows.
const BADGE: f32 = 28.;

const QUOTES: [&str; 7] = ["All", "USDT", "USDC", "FDUSD", "BTC", "ETH", "BNB"];

pub struct SymbolSearch {
    app: WeakEntity<TradeTerm>,
    input: Entity<InputState>,
    source: Arc<BinanceSource>,
    handle: tokio::runtime::Handle,
    /// Full names and logos of assets.
    assets: AssetCache,
    recent: Vec<SymbolInfo>,
    query: String,
    /// Results of `results_for`; replaced when a newer search finishes.
    results: Vec<SymbolInfo>,
    results_for: String,
    searching: bool,
    error: Option<String>,
    quote: usize,
    nav: ListNav,
    /// Enter was pressed while a search was in flight.
    enter_pending: bool,
    /// The field shows a highlighted row's symbol (↑/↓ preview); the next
    /// change event is ours, not typing.
    previewing: bool,
    _sub: Subscription,
}

impl SymbolSearch {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app: WeakEntity<TradeTerm>,
        source: Arc<BinanceSource>,
        handle: tokio::runtime::Handle,
        assets: AssetCache,
        recent: Vec<SymbolInfo>,
        initial: &str,
        quote: usize,
        select_all: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Symbol, e.g. BTCUSDT"));
        let _sub = cx.subscribe_in(
            &input,
            window,
            |this, input, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => {
                    if std::mem::take(&mut this.previewing) {
                        return;
                    }
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
            assets,
            recent,
            query: String::new(),
            results: Vec::new(),
            results_for: String::new(),
            searching: false,
            error: None,
            quote: quote.min(QUOTES.len() - 1),
            nav: ListNav::new(),
            enter_pending: false,
            previewing: false,
            _sub,
        };
        if !initial.is_empty() {
            let text = initial.to_ascii_uppercase();
            this.input.update(cx, |s, cx| {
                s.set_value(text.clone(), window, cx);
                if select_all {
                    // Like TradingView: the current symbol, selected, so
                    // typing replaces it.
                    s.select_all(window, cx);
                }
            });
            if !select_all {
                this.set_query(text, cx);
            }
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
        self.nav.reset();
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

    /// A navigation key; returns whether it was one.
    fn nav_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // Enter stays with the field (`InputEvent::PressEnter`).
        let k = self.nav.key(key, self.rows().len());
        if !k.is_nav() {
            return false;
        }
        if !k.changed() {
            return true;
        }
        if let Some(i) = self.nav.selected() {
            // Preview the highlighted symbol in the field (no new search).
            if let Some(sym) = self.rows().get(i).map(|s| s.symbol.to_string()) {
                self.previewing = true;
                self.input.update(cx, |s, cx| s.set_value(sym, window, cx));
            }
        }
        cx.notify();
        true
    }

    fn set_quote(&mut self, i: usize, cx: &mut Context<Self>) {
        self.quote = i;
        self.nav.reset();
        // Remembered for the next opening.
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |a, _| a.search_quote = i).ok();
        });
        cx.notify();
    }

    fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        if self.searching && self.nav.selected().is_none() {
            // Like TradingView: open the top result once it arrives.
            self.enter_pending = true;
            return;
        }
        match self.nav.selected() {
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

impl SymbolSearch {
    /// One result: logo (or initial badge), symbol and the pair's full name
    /// ("Bitcoin / TetherUS", else "BASE / QUOTE") with the query
    /// highlighted, SPOT tag, exchange.
    fn row(&self, i: usize, s: &SymbolInfo, cx: &mut Context<Self>) -> ListRow {
        let t = cx.ui();
        let (active, muted) = (hsla(t.active), hsla(t.text_muted));
        let key = if s.base.is_empty() {
            s.symbol.to_string()
        } else {
            s.base.clone()
        };
        let initial = SharedString::from(key.chars().next().unwrap_or('?').to_string());
        let (bg, fg) = (hsla(badge_color(t, &key)), hsla(t.text_on_accent));
        let badge = logo_or_badge(self.assets.logo(&key), px(BADGE), move || {
            div()
                .size(px(BADGE))
                .flex_none()
                .rounded(px(R_FULL))
                .bg(bg)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(TEXT_SMALL))
                .font_weight(FontWeight::MEDIUM)
                .text_color(fg)
                .child(initial.clone())
                .into_any_element()
        });
        let pair = self
            .assets
            .pair_name(&s.base, &s.quote)
            .unwrap_or_else(|| format!("{} / {}", s.base, s.quote));
        let primary =
            div()
                .whitespace_nowrap()
                .child(highlighted(s.symbol.to_string(), &self.query, active));
        let secondary = h_flex()
            .gap(px(S1_5))
            .whitespace_nowrap()
            .child(highlighted(pair, &self.query, active))
            .child(Tag::new("SPOT"));
        let info = s.clone();
        let this = cx.entity().downgrade();
        ListRow::new(("result", i))
            .tall()
            .selected(self.nav.selected() == Some(i))
            .leading(badge)
            .primary_el(primary)
            .secondary_el(secondary)
            .trailing(
                div()
                    .flex_none()
                    .text_size(px(TEXT_SMALL))
                    .text_color(muted)
                    .child("Binance"),
            )
            .on_click(move |_, _, cx| {
                this.update(cx, |this, cx| this.choose(info.clone(), cx))
                    .ok();
            })
    }
}

impl Render for SymbolSearch {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = hsla(cx.ui().text_muted);
        let rows: Vec<SymbolInfo> = self.rows().into_iter().cloned().collect();

        let this = cx.entity().downgrade();
        let chips = h_flex()
            .flex_none()
            .gap(px(S1))
            .children(QUOTES.iter().enumerate().map(|(i, q)| {
                let this = this.clone();
                Chip::new(("quote-chip", i), *q)
                    .selected(i == self.quote)
                    .on_click(move |_, _, cx| {
                        this.update(cx, |this, cx| this.set_quote(i, cx)).ok();
                    })
            }));

        let empty = if let Some(e) = &self.error {
            Some(format!("Search failed: {e}"))
        } else if rows.is_empty() {
            Some(
                if self.query.is_empty() {
                    "Type a symbol or asset, e.g. BTC, ETHUSDT, SOL"
                } else if self.searching || self.results_for != self.query {
                    "Searching… (the first search loads the symbol list)"
                } else {
                    "No symbols match your criteria"
                }
                .to_owned(),
            )
        } else {
            None
        };
        let recent_header = empty.is_none() && self.query.is_empty();
        let body = match empty {
            Some(msg) => EmptyState::new(msg).into_any_element(),
            None => {
                let list_rows: Vec<ListRow> = rows
                    .iter()
                    .enumerate()
                    .map(|(i, s)| self.row(i, s, cx))
                    .collect();
                // Rows are the list's only children, so row i is item i for
                // scroll-into-view.
                v_flex()
                    .id("search-results")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.nav.scroll)
                    .children(list_rows)
                    .into_any_element()
            }
        };

        DialogFrame::new(DialogSize::Md).id("symbol-search").body(
            v_flex()
                .size_full()
                .gap(px(S2))
                .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                    let k = &e.keystroke;
                    if k.modifiers.control || k.modifiers.alt || k.modifiers.shift {
                        return;
                    }
                    if this.nav_key(k.key.as_str(), window, cx) {
                        cx.stop_propagation();
                    }
                }))
                .child(
                    Input::new(&self.input)
                        .prefix(glyph(Glyph::SEARCH, ICON_SM).color(muted))
                        .cleanable(true),
                )
                .child(chips)
                .when(recent_header, |d| d.child(SectionHeader::new("Recent")))
                .child(body),
        )
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
