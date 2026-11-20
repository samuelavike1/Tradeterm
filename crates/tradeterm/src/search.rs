//! Symbol search dialog, modelled on TradingView's.
//!
//! - Opens from the toolbar's ticker pill, or by typing a letter/digit while
//!   no text field has focus (the dialog starts with that character).
//! - The field starts with the current symbol, fully selected.
//! - Quote-currency chips filter results (TradingView's category chips); the
//!   choice is remembered between openings.
//! - ↑/↓ highlight a row and show its symbol in the field; Enter opens the
//!   highlighted row (or the top one); click opens; Esc / outside click closes.
//! - Matches are highlighted in the symbol and description.
//! - An empty query lists recently opened symbols.

use crate::icons::{self, Icon};
use crossbeam_channel::{Receiver, Sender};
use egui::text::{CCursor, CCursorRange, LayoutJob, TextFormat};
use egui::{
    Align, Align2, Color32, FontId, Frame, Id, Key, Margin, Rect, RichText, Sense, Stroke,
    TextEdit, pos2, vec2,
};
use tt_core::{Symbol, SymbolInfo};
use tt_theme::{Theme, with_alpha};

/// Result of a search request: (query, results or error message).
pub type SearchReply = (String, Result<Vec<SymbolInfo>, String>);

/// Quote currencies offered as filter chips (`None` = All).
pub const QUOTE_FILTERS: [Option<&str>; 9] = [
    None,
    Some("USDT"),
    Some("USDC"),
    Some("FDUSD"),
    Some("BTC"),
    Some("ETH"),
    Some("BNB"),
    Some("EUR"),
    Some("TRY"),
];

const ROW_HEIGHT: f32 = 50.0;
const MAX_ROWS: usize = 100;

pub struct SymbolSearch {
    open: bool,
    /// Text in the field (may show a previewed symbol while arrowing).
    input: String,
    /// What the user typed; results belong to this.
    query: String,
    results: Vec<SymbolInfo>,
    results_query: String,
    loading: bool,
    error: Option<String>,
    selected: Option<usize>,
    /// Selected quote chip; remembered between openings.
    pub quote_filter: Option<String>,
    /// Apply selection/focus to the text field on the next frame.
    pending_focus: Option<FocusMode>,
    scroll_to_selected: bool,
    /// Enter was pressed while results were still loading: open the top
    /// result as soon as they arrive.
    enter_pending: bool,
    tx: Sender<SearchReply>,
    rx: Receiver<SearchReply>,
}

#[derive(Clone, Copy)]
enum FocusMode {
    SelectAll,
    CursorAtEnd,
}

impl Default for SymbolSearch {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            open: false,
            input: String::new(),
            query: String::new(),
            results: Vec::new(),
            results_query: String::new(),
            loading: false,
            error: None,
            selected: None,
            quote_filter: None,
            pending_focus: None,
            scroll_to_selected: false,
            enter_pending: false,
            tx,
            rx,
        }
    }
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .take(20)
        .collect()
}

impl SymbolSearch {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens with the current symbol selected (click on the ticker).
    pub fn open_with_current(&mut self, current: &str) {
        self.open = true;
        self.input = current.to_owned();
        self.query = String::new();
        self.selected = None;
        self.pending_focus = Some(FocusMode::SelectAll);
    }

    /// Opens with `text` already typed (type-to-search on the chart).
    /// Returns the query to search for.
    pub fn open_with_text(&mut self, text: &str) -> String {
        self.open = true;
        self.input = normalize(text);
        self.query = self.input.clone();
        self.selected = None;
        self.loading = !self.query.is_empty();
        self.pending_focus = Some(FocusMode::CursorAtEnd);
        self.query.clone()
    }

    pub fn close(&mut self) {
        self.open = false;
        self.selected = None;
        self.enter_pending = false;
    }

    /// Results for the current query have not arrived yet.
    fn awaiting_results(&self) -> bool {
        !self.query.is_empty() && self.error.is_none() && self.results_query != self.query
    }

    /// What Enter opens: the highlighted row, else the top row. Without rows,
    /// the typed text is used only if the search itself failed (offline).
    fn enter_choice(&self, rows: &[SymbolInfo]) -> Option<Symbol> {
        match self
            .selected
            .or((!rows.is_empty()).then_some(0))
            .and_then(|i| rows.get(i))
        {
            Some(s) => Some(s.symbol.clone()),
            None if self.error.is_some() => Some(Symbol::new(&self.input)).filter(|s| s.is_valid()),
            None => None,
        }
    }

    /// Applies finished searches; replies for outdated queries are dropped.
    pub fn poll(&mut self) {
        for (query, reply) in self.rx.try_iter() {
            if query != self.query {
                continue;
            }
            self.loading = false;
            match reply {
                Ok(results) => {
                    self.results = results;
                    self.results_query = query;
                    self.error = None;
                    self.selected = None;
                }
                Err(e) => self.error = Some(e),
            }
        }
    }

    /// Latest search results (for metadata of a chosen symbol).
    pub fn last_results(&self) -> &[SymbolInfo] {
        &self.results
    }

    pub fn reply_sender(&self) -> Sender<SearchReply> {
        self.tx.clone()
    }

    /// Rows currently listed: search results filtered by the quote chip, or
    /// recent symbols when the query is empty.
    fn rows(&self, recent: &[SymbolInfo]) -> Vec<SymbolInfo> {
        let source: Vec<SymbolInfo> = if self.query.is_empty() {
            recent.to_vec()
        } else if self.results_query == self.query {
            self.results.clone()
        } else {
            Vec::new()
        };
        source
            .into_iter()
            .filter(|s| self.quote_filter.as_deref().is_none_or(|q| s.quote == q))
            .take(MAX_ROWS)
            .collect()
    }

    /// Draws the dialog if open. `request(query, reply_tx)` must start a
    /// background search. Returns the chosen symbol.
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        theme: &Theme,
        recent: &[SymbolInfo],
        // Called with the base asset (or the symbol when unknown).
        badge_color: impl Fn(&str) -> Color32,
        request: impl FnOnce(String, Sender<SearchReply>),
    ) -> Option<Symbol> {
        if !self.open {
            return None;
        }
        let rows = self.rows(recent);

        // Keys handled by the dialog (consumed before the text field sees them).
        let (up, down, enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, Key::Enter),
                i.consume_key(egui::Modifiers::NONE, Key::Escape),
            )
        });
        if esc {
            self.close();
            return None;
        }
        if (up || down) && !rows.is_empty() {
            let last = rows.len() - 1;
            self.selected = Some(match (self.selected, down) {
                (None, true) => 0,
                (None, false) => last,
                (Some(i), true) => (i + 1).min(last),
                (Some(i), false) => i.saturating_sub(1),
            });
            // Like TradingView: the field previews the highlighted symbol.
            if let Some(s) = self.selected.and_then(|i| rows.get(i)) {
                self.input = s.symbol.to_string();
                self.pending_focus = Some(FocusMode::CursorAtEnd);
            }
            self.scroll_to_selected = true;
        }
        let mut chosen: Option<Symbol> = None;
        if enter && self.awaiting_results() {
            // Like TradingView: Enter during a search opens the top result
            // once it arrives, instead of guessing from the raw text.
            self.enter_pending = true;
        } else if enter || (self.enter_pending && !self.awaiting_results()) {
            self.enter_pending = false;
            chosen = self.enter_choice(&rows);
        }

        let screen = ctx.content_rect();
        let width = (screen.width() - 80.0).clamp(360.0, 720.0);
        let height = (screen.height() - 80.0).clamp(300.0, 640.0);
        let frame = Frame::new()
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.border))
            .corner_radius(10)
            .inner_margin(Margin::same(0))
            .shadow(egui::Shadow {
                offset: [0, 8],
                blur: 28,
                spread: 0,
                color: with_alpha(Color32::BLACK, if theme.dark { 0.6 } else { 0.18 }),
            });

        let mut request_query: Option<String> = None;
        let modal = egui::Modal::new(Id::unique("symbol_search_modal"))
            .frame(frame)
            .backdrop_color(with_alpha(
                Color32::BLACK,
                if theme.dark { 0.55 } else { 0.25 },
            ))
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.set_height(height);
                ui.spacing_mut().item_spacing = vec2(6.0, 0.0);

                // ---- search field ------------------------------------------
                let field_id = Id::unique("symbol_search_field");
                Frame::new()
                    .inner_margin(Margin {
                        left: 18,
                        right: 14,
                        top: 12,
                        bottom: 10,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (icon_rect, _) =
                                ui.allocate_exact_size(vec2(24.0, 28.0), Sense::hover());
                            icons::paint(
                                ui.painter(),
                                icon_rect.center(),
                                Icon::Search,
                                18.0,
                                theme.text_muted,
                            );
                            let clear_w = 50.0;
                            let edit = ui.add(
                                TextEdit::singleline(&mut self.input)
                                    .id(field_id)
                                    .frame(Frame::NONE)
                                    .font(FontId::proportional(17.0))
                                    .text_color(theme.text)
                                    .hint_text("Symbol, e.g. BTCUSDT")
                                    .desired_width(ui.available_width() - clear_w),
                            );
                            if edit.changed() {
                                self.enter_pending = false;
                                self.input = normalize(&self.input);
                                self.query = self.input.clone();
                                self.selected = None;
                                self.loading = !self.query.is_empty();
                                if !self.query.is_empty() {
                                    request_query = Some(self.query.clone());
                                }
                            }
                            if let Some(mode) = self.pending_focus.take() {
                                edit.request_focus();
                                if let Some(mut state) = TextEdit::load_state(ui.ctx(), field_id) {
                                    let n = self.input.chars().count();
                                    let range = match mode {
                                        FocusMode::SelectAll => {
                                            CCursorRange::two(CCursor::new(0), CCursor::new(n))
                                        }
                                        FocusMode::CursorAtEnd => {
                                            CCursorRange::one(CCursor::new(n))
                                        }
                                    };
                                    state.cursor.set_char_range(Some(range));
                                    state.store(ui.ctx(), field_id);
                                }
                            }
                            let clear = ui.add_sized(
                                vec2(clear_w - 6.0, 26.0),
                                egui::Button::new(RichText::new("Clear").color(theme.text_muted))
                                    .frame(false),
                            );
                            if clear.clicked() {
                                self.input.clear();
                                self.query.clear();
                                self.selected = None;
                                self.pending_focus = Some(FocusMode::CursorAtEnd);
                            }
                        });
                    });

                // ---- quote chips -------------------------------------------
                Frame::new()
                    .inner_margin(Margin {
                        left: 16,
                        right: 16,
                        top: 0,
                        bottom: 10,
                    })
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                            for q in QUOTE_FILTERS {
                                let label = q.unwrap_or("All");
                                let selected = self.quote_filter.as_deref() == q;
                                if chip(ui, label, selected, theme).clicked() {
                                    self.quote_filter = q.map(str::to_owned);
                                    self.selected = None;
                                }
                            }
                        });
                    });
                ui.painter().hline(
                    ui.max_rect().x_range(),
                    ui.cursor().top(),
                    Stroke::new(1.0, theme.border),
                );

                // ---- results -----------------------------------------------
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Frame::new()
                            .inner_margin(Margin::symmetric(8, 6))
                            .show(ui, |ui| {
                                if let Some(pick) = self.results_ui(ui, theme, &rows, &badge_color)
                                {
                                    chosen = Some(pick);
                                }
                            });
                    });
            });
        if modal.should_close() {
            self.close();
        }
        if let Some(q) = request_query {
            request(q, self.tx.clone());
        }
        if chosen.is_some() {
            self.close();
        }
        chosen
    }

    fn results_ui(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        rows: &[SymbolInfo],
        badge_color: &impl Fn(&str) -> Color32,
    ) -> Option<Symbol> {
        let muted = |t: &str| RichText::new(t).color(theme.text_muted);
        if let Some(e) = &self.error {
            ui.add_space(12.0);
            ui.label(muted(&format!("Search failed: {e}")));
            return None;
        }
        if rows.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                let msg = if self.query.is_empty() {
                    "Type a symbol or asset, e.g. BTC, ETHUSDT, SOL"
                } else if self.loading || self.results_query != self.query {
                    "Searching… (the first search loads the symbol list)"
                } else {
                    "No symbols match your criteria"
                };
                ui.label(muted(msg));
            });
            return None;
        }
        if self.query.is_empty() {
            ui.label(muted("Recent").size(12.0));
            ui.add_space(4.0);
        }

        let mut picked = None;
        let needle = self.query.clone();
        let ctx = ui.ctx().clone();
        let pointer_moved = ctx.input(|inp| inp.pointer.delta() != egui::Vec2::ZERO);
        for (i, s) in rows.iter().enumerate() {
            let (rect, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
            // Mouse hover selects, but only when the mouse actually moves, so
            // a resting pointer doesn't fight the arrow keys.
            if resp.hovered() && pointer_moved {
                self.selected = Some(i);
            }
            let is_sel = self.selected == Some(i);
            if is_sel && self.scroll_to_selected {
                ui.scroll_to_rect(rect, Some(Align::Center));
                self.scroll_to_selected = false;
            }
            let p = ui.painter();
            if is_sel {
                p.rect_filled(rect, 6.0, theme.selection);
            }

            // Badge with the base asset's initial.
            let c = pos2(rect.left() + 26.0, rect.center().y);
            let key = if s.base.is_empty() {
                s.symbol.as_str()
            } else {
                s.base.as_str()
            };
            p.circle_filled(c, 15.0, badge_color(key));
            p.text(
                c,
                Align2::CENTER_CENTER,
                s.base.chars().next().unwrap_or('?').to_string(),
                FontId::proportional(13.0),
                theme.text_on_accent,
            );

            // Symbol (match highlighted) over "BASE / QUOTE" + type badge.
            let text_x = rect.left() + 54.0;
            let sym = highlighted(s.symbol.as_str(), &needle, 14.5, theme.text, theme);
            let sym_galley = ctx.fonts_mut(|f| f.layout_job(sym));
            p.galley(pos2(text_x, rect.top() + 7.0), sym_galley, theme.text);

            let desc = format!("{} / {}", s.base, s.quote);
            let desc_job = highlighted(&desc, &needle, 12.0, theme.text_muted, theme);
            let desc_galley = ctx.fonts_mut(|f| f.layout_job(desc_job));
            let desc_w = desc_galley.size().x;
            let desc_pos = pos2(text_x, rect.top() + 27.0);
            p.galley(desc_pos, desc_galley, theme.text_muted);

            let badge_galley = ctx.fonts_mut(|f| {
                f.layout_no_wrap("SPOT".into(), FontId::proportional(9.5), theme.text_muted)
            });
            let badge = Rect::from_min_size(
                pos2(desc_pos.x + desc_w + 8.0, desc_pos.y + 1.0),
                badge_galley.size() + vec2(8.0, 3.0),
            );
            p.rect_filled(badge, 3.0, theme.border);
            p.galley(badge.min + vec2(4.0, 1.5), badge_galley, theme.text_muted);

            p.text(
                pos2(rect.right() - 14.0, rect.center().y),
                Align2::RIGHT_CENTER,
                "Binance",
                FontId::proportional(12.5),
                theme.text_muted,
            );

            if resp.clicked() {
                picked = Some(s.symbol.clone());
            }
        }
        picked
    }
}

/// A rounded filter chip; the selected one is inverted like TradingView's.
fn chip(ui: &mut egui::Ui, label: &str, selected: bool, theme: &Theme) -> egui::Response {
    let galley = ui
        .ctx()
        .fonts_mut(|f| f.layout_no_wrap(label.to_owned(), FontId::proportional(13.0), theme.text));
    let size = galley.size() + vec2(24.0, 12.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let (bg, fg) = if selected {
        (theme.text, theme.bg)
    } else if resp.hovered() {
        (theme.border, theme.text)
    } else {
        (theme.panel_alt, theme.text)
    };
    let p = ui.painter();
    p.rect_filled(rect, rect.height() / 2.0, bg);
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(13.0),
        fg,
    );
    resp
}

/// `text` with every case-insensitive occurrence of `needle` in the accent blue.
fn highlighted(text: &str, needle: &str, size: f32, color: Color32, theme: &Theme) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font = FontId::proportional(size);
    let plain = TextFormat::simple(font.clone(), color);
    let hit = TextFormat::simple(font, theme.series_palette[0]);
    for (s, is_hit) in match_spans(text, needle) {
        job.append(s, 0.0, if is_hit { hit.clone() } else { plain.clone() });
    }
    job
}

/// Splits `text` into (segment, matched) runs for case-insensitive `needle`.
fn match_spans<'a>(text: &'a str, needle: &str) -> Vec<(&'a str, bool)> {
    if needle.is_empty() {
        return vec![(text, false)];
    }
    let hay = text.to_ascii_uppercase();
    let needle = needle.to_ascii_uppercase();
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(off) = hay[pos..].find(&needle) {
        let start = pos + off;
        if start > pos {
            out.push((&text[pos..start], false));
        }
        out.push((&text[start..start + needle.len()], true));
        pos = start + needle.len();
    }
    if pos < text.len() {
        out.push((&text[pos..], false));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_mark_every_match() {
        assert_eq!(
            match_spans("BTC / USDT", "btc"),
            [("BTC", true), (" / USDT", false)]
        );
        assert_eq!(
            match_spans("WBTCBTC", "BTC"),
            [("W", false), ("BTC", true), ("BTC", true)]
        );
        assert_eq!(match_spans("ETH", ""), [("ETH", false)]);
        assert_eq!(match_spans("ETH", "SOL"), [("ETH", false)]);
    }

    #[test]
    fn open_modes() {
        let mut s = SymbolSearch::default();
        s.open_with_current("BTCUSDT");
        assert!(s.is_open());
        assert_eq!(s.input, "BTCUSDT");
        assert!(s.query.is_empty(), "current symbol is not a query");
        let q = s.open_with_text("e");
        assert_eq!((q.as_str(), s.input.as_str()), ("E", "E"));
        s.close();
        assert!(!s.is_open());
    }

    #[test]
    fn rows_filter_by_quote_and_show_recent_when_empty() {
        let mut s = SymbolSearch::default();
        let mk = |sym: &str, base: &str, quote: &str| SymbolInfo {
            base: base.into(),
            quote: quote.into(),
            ..SymbolInfo::fallback(Symbol::new(sym))
        };
        let recent = vec![mk("ETHUSDT", "ETH", "USDT"), mk("ETHBTC", "ETH", "BTC")];
        assert_eq!(s.rows(&recent).len(), 2);
        s.quote_filter = Some("BTC".into());
        assert_eq!(s.rows(&recent)[0].symbol.as_str(), "ETHBTC");

        s.quote_filter = None;
        s.query = "BTC".into();
        s.results_query = "BTC".into();
        s.results = vec![mk("BTCUSDT", "BTC", "USDT"), mk("BTCEUR", "BTC", "EUR")];
        assert_eq!(s.rows(&recent).len(), 2);
        s.quote_filter = Some("EUR".into());
        assert_eq!(s.rows(&recent)[0].symbol.as_str(), "BTCEUR");
        // Stale results (for another query) are not shown.
        s.query = "ETH".into();
        assert!(s.rows(&recent).is_empty());
    }

    #[test]
    fn enter_never_opens_unknown_text() {
        let mut s = SymbolSearch::default();
        s.open_with_text("sol");
        // Still searching: nothing to open yet.
        assert!(s.awaiting_results());
        assert_eq!(s.enter_choice(&[]), None);
        // Search finished with no matches: still nothing.
        s.results_query = s.query.clone();
        assert!(!s.awaiting_results());
        assert_eq!(s.enter_choice(&[]), None);
        // Top row wins when there are results.
        let row = SymbolInfo::fallback(Symbol::new("SOLUSDT"));
        assert_eq!(s.enter_choice(&[row]).unwrap().as_str(), "SOLUSDT");
        // Offline (search failed): fall back to the typed text.
        s.error = Some("network".into());
        assert_eq!(s.enter_choice(&[]).unwrap().as_str(), "SOL");
    }
}
