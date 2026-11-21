//! Symbol search dialog, TradingView's (measured, tokens `SS_*`): the search
//! field as the header with a "Clear" button, quote filter pills, 58 px
//! result rows (logo, symbol and pair name with the match in blue, type
//! badge, exchange with its logo), recent symbols when empty.
//! ↑/↓ and PageUp/PageDown move the selection (Home/End too once a row is
//! selected) and preview it in the field, Enter opens it (if a search is
//! still running, the top result once it arrives), Esc closes.
//!
//! Searches run against [`SymbolList`], read from its disk cache at
//! startup, so even the first one is instant.

use crate::app::TradeTerm;
use crate::ui::{badge_color, logo_or_badge};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::ops::Range;
use tt_core::{Symbol, SymbolInfo};
use tt_session::assets::AssetCache;
use tt_session::symbols::SymbolList;
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

const QUOTES: [&str; 7] = ["All", "USDT", "USDC", "FDUSD", "BTC", "ETH", "BNB"];

/// The exchange's name and the asset whose logo stands for it (Binance's
/// mark is BNB's).
const EXCHANGE: &str = "Binance";
const EXCHANGE_LOGO_ASSET: &str = "BNB";
/// Binance yellow, for the letter badge until the logo is on disk.
const EXCHANGE_COLOR: u32 = 0xF0B90B;

pub struct SymbolSearch {
    app: WeakEntity<TradeTerm>,
    input: Entity<InputState>,
    symbols: SymbolList,
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
        symbols: SymbolList,
        handle: tokio::runtime::Handle,
        assets: AssetCache,
        recent: Vec<SymbolInfo>,
        initial: &str,
        quote: usize,
        select_all: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Symbol or asset, e.g. BTCUSDT"));
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
            symbols,
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
        let (symbols, q) = (self.symbols.clone(), query.clone());
        let t0 = std::time::Instant::now();
        let task = self.handle.spawn(async move { symbols.search(&q).await });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.query != query {
                    return; // a newer search is running
                }
                tracing::debug!(
                    query = %query,
                    ms = t0.elapsed().as_secs_f64() * 1e3,
                    "symbol search"
                );
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

/// A round logo of `size`, or a letter badge in `bg` until it is on disk.
fn logo(assets: &AssetCache, code: &str, size: f32, bg: Hsla, fg: Hsla) -> AnyElement {
    let initial = SharedString::from(code.chars().next().unwrap_or('?').to_string());
    logo_or_badge(assets.logo(code), px(size), move || {
        div()
            .size(px(size))
            .flex_none()
            .rounded(px(R_FULL))
            .bg(bg)
            .flex()
            .items_center()
            .justify_center()
            .text_size(px((size * 0.44).round()))
            .font_weight(FontWeight::BOLD)
            .text_color(fg)
            .child(initial.clone())
            .into_any_element()
    })
}

impl SymbolSearch {
    /// One result, TradingView's: 36 px logo, the symbol (16 px) and the
    /// pair's full name ("Bitcoin / TetherUS", else "BASE / QUOTE", 13 px)
    /// with the query in blue, the "SPOT CRYPTO" badge, the exchange name
    /// and its 24 px logo at the right.
    fn row(&self, i: usize, s: &SymbolInfo, cx: &mut Context<Self>) -> ListRow {
        let t = cx.ui();
        let (active, muted) = (hsla(t.active), hsla(t.text_muted));
        let key = if s.base.is_empty() {
            s.symbol.to_string()
        } else {
            s.base.clone()
        };
        let on_badge = hsla(t.text_on_accent);
        let badge = logo(
            &self.assets,
            &key,
            LOGO_LG,
            hsla(badge_color(t, &key)),
            on_badge,
        );
        let exchange = logo(
            &self.assets,
            EXCHANGE_LOGO_ASSET,
            LOGO_EXCHANGE,
            rgb(EXCHANGE_COLOR).into(),
            on_badge,
        );
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
            .items_center()
            .whitespace_nowrap()
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(highlighted(pair, &self.query, active)),
            )
            .child(Tag::badge("spot crypto"));
        let info = s.clone();
        let this = cx.entity().downgrade();
        ListRow::new(("result", i))
            .large()
            .selected(self.nav.selected() == Some(i))
            .leading(badge)
            .primary_el(primary)
            .secondary_el(secondary)
            .trailing(
                h_flex()
                    .flex_none()
                    .pl(px(S6))
                    .gap(px(S2))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(TEXT_ROW_LG_SECONDARY))
                            .line_height(px(LINE_ROW_LG_SECONDARY))
                            .text_color(muted)
                            .child(EXCHANGE),
                    )
                    .child(exchange),
            )
            .on_click(move |_, _, cx| {
                this.update(cx, |this, cx| this.choose(info.clone(), cx))
                    .ok();
            })
    }
}

impl Render for SymbolSearch {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows: Vec<SymbolInfo> = self.rows().into_iter().cloned().collect();

        let this = cx.entity().downgrade();
        let pills = h_flex()
            .flex_none()
            .px(px(C_PAD_X))
            .pt(px(PILL_GAP))
            .pb(px(SS_PILLS_BOTTOM))
            .gap(px(PILL_GAP))
            .children(QUOTES.iter().enumerate().map(|(i, q)| {
                let (pick, clear) = (this.clone(), this.clone());
                FilterPill::new(("quote-pill", i), *q)
                    .selected(i == self.quote)
                    .on_click(move |_, _, cx| {
                        pick.update(cx, |this, cx| this.set_quote(i, cx)).ok();
                    })
                    // Like TradingView's "Crypto ×": back to all quotes.
                    .when(i > 0, |p| {
                        p.on_clear(move |_, _, cx| {
                            clear.update(cx, |this, cx| this.set_quote(0, cx)).ok();
                        })
                    })
            }));

        let empty = if let Some(e) = &self.error {
            Some(format!("Search failed: {e}"))
        } else if rows.is_empty() {
            Some(
                if self.query.is_empty() {
                    "Type a symbol or asset, e.g. BTC, ETHUSDT, SOL"
                } else if self.searching || self.results_for != self.query {
                    "Searching…"
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
                    .px(px(LIST_LG_INSET))
                    .pb(px(S1_5))
                    .overflow_y_scroll()
                    .track_scroll(&self.nav.scroll)
                    .children(list_rows)
                    .into_any_element()
            }
        };

        v_flex()
            .id("symbol-search")
            .w_full()
            .h(px(SS_H_DIALOG))
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
                div()
                    .flex_none()
                    .px(px(C_PAD_X))
                    .pt(px(SS_PAD_TOP))
                    .pb(px(SS_PAD_BOTTOM))
                    .child(SearchField::header(&self.input)),
            )
            .child(pills)
            .when(recent_header, |d| {
                d.child(
                    div()
                        .flex_none()
                        .px(px(LIST_LG_INSET))
                        .child(SectionHeader::new("Recent")),
                )
            })
            .child(body)
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
