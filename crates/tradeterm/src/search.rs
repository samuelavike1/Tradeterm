//! Symbol box with live search results.
//!
//! Typing triggers a background search (the caller spawns it); results come
//! back over a channel and appear in a popup under the box. ↑/↓ choose,
//! Enter or a click applies, Esc or clicking elsewhere cancels.

use crate::toolbar::{pill_fill, search_icon};
use crossbeam_channel::{Receiver, Sender};
use egui::{FontId, Frame, Key, Margin, Rect, RichText, Stroke, StrokeKind, TextEdit, vec2};
use tt_core::SymbolInfo;
use tt_theme::Theme;

/// Result of a search request: (query, results or error message).
pub type SearchReply = (String, Result<Vec<SymbolInfo>, String>);

pub enum SearchOutcome {
    /// The user picked or typed a symbol.
    Submit(String),
    /// Editing ended without a choice; the caller should restore the box.
    Cancel,
}

pub struct SymbolSearch {
    pub input: String,
    open: bool,
    results: Vec<SymbolInfo>,
    /// Query the current `results` belong to.
    results_query: String,
    loading: bool,
    error: Option<String>,
    selected: usize,
    popup_rect: Option<Rect>,
    tx: Sender<SearchReply>,
    rx: Receiver<SearchReply>,
}

impl SymbolSearch {
    pub fn new(initial: &str) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            input: initial.to_owned(),
            open: false,
            results: Vec::new(),
            results_query: String::new(),
            loading: false,
            error: None,
            selected: 0,
            popup_rect: None,
            tx,
            rx,
        }
    }

    /// Applies finished searches; replies for outdated queries are dropped.
    pub fn poll(&mut self) {
        for (query, reply) in self.rx.try_iter() {
            if query != self.input {
                continue;
            }
            self.loading = false;
            match reply {
                Ok(results) => {
                    self.results = results;
                    self.results_query = query;
                    self.error = None;
                    self.selected = 0;
                }
                Err(e) => self.error = Some(e),
            }
        }
    }

    /// Draws the box and popup. `request(query, reply_tx)` must start a
    /// background search that sends its [`SearchReply`] to `reply_tx`.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        request: impl FnOnce(String, Sender<SearchReply>),
    ) -> Option<SearchOutcome> {
        // A rounded "pill" with a magnifier, like TradingView's symbol box.
        let pill = Frame::new()
            .fill(pill_fill(theme))
            .corner_radius(14)
            .inner_margin(Margin {
                left: 28,
                right: 12,
                top: 5,
                bottom: 5,
            })
            .show(ui, |ui| {
                // Size the field to the text so it sits centered in the pill.
                ui.spacing_mut().interact_size.y = 18.0;
                ui.add(
                    TextEdit::singleline(&mut self.input)
                        .frame(Frame::NONE)
                        .desired_width(92.0)
                        .hint_text("Symbol")
                        .char_limit(20)
                        .font(FontId::proportional(14.0))
                        .text_color(theme.text),
                )
            });
        let edit = pill.inner;
        let pill_rect = pill.response.rect;
        let p = ui.painter();
        search_icon(
            p,
            pill_rect.left_center() + vec2(15.0, 0.0),
            theme.text_muted,
        );
        if edit.has_focus() {
            p.rect_stroke(
                pill_rect,
                14.0,
                Stroke::new(1.0, theme.text_muted),
                StrokeKind::Inside,
            );
        }
        if edit.changed() {
            self.input = self
                .input
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .map(|c| c.to_ascii_uppercase())
                .collect();
            if self.input.is_empty() {
                self.open = false;
            } else {
                self.open = true;
                self.loading = true;
                request(self.input.clone(), self.tx.clone());
            }
        }

        // Keep ↑/↓ for the result list instead of egui's focus navigation,
        // which would otherwise move focus out of the box and cancel the search.
        if edit.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    edit.id,
                    egui::EventFilter {
                        vertical_arrows: true,
                        horizontal_arrows: true,
                        tab: false,
                        escape: false,
                    },
                )
            });
        }

        let fresh = self.results_query == self.input;
        if edit.has_focus() && self.open && fresh && !self.results.is_empty() {
            let (down, up) =
                ui.input(|i| (i.key_pressed(Key::ArrowDown), i.key_pressed(Key::ArrowUp)));
            if down {
                self.selected = (self.selected + 1).min(self.results.len() - 1);
            }
            if up {
                self.selected = self.selected.saturating_sub(1);
            }
        }

        let mut outcome = None;
        if edit.lost_focus() {
            let enter = ui.input(|i| i.key_pressed(Key::Enter));
            let over_popup = self
                .popup_rect
                .is_some_and(|r| ui.ctx().pointer_hover_pos().is_some_and(|p| r.contains(p)));
            if enter {
                let pick = if self.open && fresh {
                    self.results
                        .get(self.selected)
                        .map(|s| s.symbol.to_string())
                } else {
                    None
                };
                outcome = Some(SearchOutcome::Submit(
                    pick.unwrap_or_else(|| self.input.clone()),
                ));
                self.open = false;
            } else if !over_popup {
                // Esc or click elsewhere.
                self.open = false;
                outcome = Some(SearchOutcome::Cancel);
            }
        }

        self.popup_rect = None;
        if self.open {
            let area = egui::Area::new(ui.make_persistent_id("symbol_search_popup"))
                .order(egui::Order::Foreground)
                .fixed_pos(pill_rect.left_bottom() + egui::vec2(0.0, 4.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .show(ui, |ui| {
                            ui.set_min_width(240.0);
                            self.popup_contents(ui, fresh)
                        })
                        .inner
                });
            self.popup_rect = Some(area.response.rect);
            if let Some(sym) = area.inner {
                outcome = Some(SearchOutcome::Submit(sym));
                self.open = false;
            }
        }
        outcome
    }

    /// Rows of results; returns the clicked symbol.
    fn popup_contents(&mut self, ui: &mut egui::Ui, fresh: bool) -> Option<String> {
        if let Some(e) = &self.error {
            ui.label(RichText::new(format!("Search failed: {e}")).small().weak());
            return None;
        }
        if !fresh || (self.loading && self.results.is_empty()) {
            ui.label(
                RichText::new("Searching… (first search loads the symbol list)")
                    .small()
                    .weak(),
            );
            return None;
        }
        if self.results.is_empty() {
            ui.label(RichText::new("No matches").small().weak());
            return None;
        }
        let mut picked = None;
        for (i, s) in self.results.iter().take(12).enumerate() {
            let text = RichText::new(format!("{:<12} {}/{}", s.symbol.as_str(), s.base, s.quote))
                .monospace();
            let row = ui.selectable_label(i == self.selected, text);
            if row.hovered() {
                self.selected = i;
            }
            if row.clicked() {
                picked = Some(s.symbol.to_string());
            }
        }
        picked
    }
}
