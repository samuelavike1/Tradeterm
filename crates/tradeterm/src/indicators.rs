//! TradingView-style indicator dialogs:
//!
//! - [`IndicatorsDialog`] ("Indicators, metrics, and strategies"): search,
//!   Favorites / Technicals sidebar, alphabetical list with stars. Clicking a
//!   row adds the indicator; the dialog stays open.
//! - [`IndicatorSettings`]: one indicator's Inputs / Style, applied live;
//!   Cancel restores what it was, Defaults resets.

use crate::icons::{self, Icon};
use crate::search::highlighted;
use egui::text::{CCursor, CCursorRange};
use egui::{
    Align2, Color32, FontId, Frame, Id, Key, Margin, RichText, Sense, Stroke, TextEdit, pos2, vec2,
};
use tt_chart::{ChartView, Indicator, indicators::default_colors};
use tt_core::{Source, Study, StudyKind};
use tt_theme::{Theme, with_alpha};

const ROW_HEIGHT: f32 = 34.0;
const SIDEBAR_W: f32 = 210.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Favorites,
    Technicals,
}

/// Short names users type ("rsi", "ema", "bb"…) besides the full name.
fn aliases(kind: StudyKind) -> &'static [&'static str] {
    match kind {
        StudyKind::Volume => &["vol"],
        StudyKind::Sma => &["ma", "sma"],
        StudyKind::Ema => &["ema"],
        StudyKind::BollingerBands => &["bb"],
        StudyKind::Vwap => &["vwap"],
        StudyKind::Rsi => &["rsi"],
        StudyKind::Macd => &["macd"],
        StudyKind::Stochastic => &["stoch"],
        StudyKind::Atr => &["atr"],
        StudyKind::Obv => &["obv"],
    }
}

/// Catalog entries matching `query`, alphabetically (like TradingView).
fn matching(query: &str, only: Option<&[StudyKind]>) -> Vec<StudyKind> {
    let q = query.trim().to_ascii_lowercase();
    let mut kinds: Vec<StudyKind> = StudyKind::ALL
        .into_iter()
        .filter(|k| only.is_none_or(|f| f.contains(k)))
        .filter(|k| {
            q.is_empty()
                || k.name().to_ascii_lowercase().contains(&q)
                || aliases(*k).iter().any(|a| a.starts_with(&q))
        })
        .collect();
    kinds.sort_by_key(|k| k.name().to_ascii_lowercase());
    kinds
}

pub struct IndicatorsDialog {
    open: bool,
    /// Kept between openings, like TradingView.
    query: String,
    section: Section,
    selected: Option<usize>,
    focus_select_all: bool,
}

impl Default for IndicatorsDialog {
    fn default() -> Self {
        Self {
            open: false,
            query: String::new(),
            section: Section::Technicals,
            selected: None,
            focus_select_all: false,
        }
    }
}

impl IndicatorsDialog {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
        self.selected = None;
        self.focus_select_all = true;
    }

    /// Shows the dialog; returns the indicator the user picked to add.
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        theme: &Theme,
        favorites: &mut Vec<StudyKind>,
    ) -> Option<StudyKind> {
        if !self.open {
            return None;
        }
        let only = (self.section == Section::Favorites).then(|| favorites.clone());
        let rows = matching(&self.query, only.as_deref());

        let (up, down, enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, Key::Enter),
                i.consume_key(egui::Modifiers::NONE, Key::Escape),
            )
        });
        if esc {
            self.open = false;
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
        }
        let mut picked = None;
        if enter {
            picked = self
                .selected
                .or((rows.len() == 1).then_some(0))
                .and_then(|i| rows.get(i).copied());
        }

        let screen = ctx.content_rect();
        let width = (screen.width() - 80.0).clamp(420.0, 780.0);
        let height = (screen.height() - 80.0).clamp(320.0, 600.0);
        let modal = egui::Modal::new(Id::unique("indicators_modal"))
            .frame(dialog_frame(theme))
            .backdrop_color(backdrop(theme))
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.set_height(height);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                if header(ui, theme, "Indicators, metrics, and strategies") {
                    self.open = false;
                }

                // ---- search field ------------------------------------------
                let field_id = Id::unique("indicators_search");
                Frame::new()
                    .inner_margin(Margin::symmetric(18, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                            let (r, _) = ui.allocate_exact_size(vec2(22.0, 28.0), Sense::hover());
                            icons::paint(
                                ui.painter(),
                                r.center(),
                                Icon::Search,
                                17.0,
                                theme.text_muted,
                            );
                            let edit = ui.add(
                                TextEdit::singleline(&mut self.query)
                                    .id(field_id)
                                    .frame(Frame::NONE)
                                    .font(FontId::proportional(15.5))
                                    .text_color(theme.text)
                                    .hint_text("Search")
                                    .desired_width(ui.available_width() - 30.0),
                            );
                            if edit.changed() {
                                self.selected = None;
                            }
                            if std::mem::take(&mut self.focus_select_all) {
                                edit.request_focus();
                                if let Some(mut st) = TextEdit::load_state(ui.ctx(), field_id) {
                                    let n = self.query.chars().count();
                                    st.cursor.set_char_range(Some(CCursorRange::two(
                                        CCursor::new(0),
                                        CCursor::new(n),
                                    )));
                                    st.store(ui.ctx(), field_id);
                                }
                            }
                            if !self.query.is_empty() {
                                let (r, resp) =
                                    ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                                let c = if resp.hovered() {
                                    theme.text
                                } else {
                                    theme.text_muted
                                };
                                icons::paint(ui.painter(), r.center(), Icon::X, 16.0, c);
                                if resp.clicked() {
                                    self.query.clear();
                                    self.focus_select_all = true;
                                }
                            }
                        });
                    });
                separator(ui, theme);

                // ---- sidebar + list ----------------------------------------
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    let body_h = ui.available_height();
                    ui.allocate_ui(vec2(SIDEBAR_W, body_h), |ui| {
                        ui.set_min_size(vec2(SIDEBAR_W, body_h));
                        ui.vertical(|ui| {
                            ui.add_space(10.0);
                            sidebar_heading(ui, theme, "PERSONAL");
                            if sidebar_item(
                                ui,
                                theme,
                                Icon::Star,
                                "Favorites",
                                self.section == Section::Favorites,
                            ) {
                                self.section = Section::Favorites;
                                self.selected = None;
                            }
                            ui.add_space(8.0);
                            sidebar_heading(ui, theme, "BUILT-IN");
                            if sidebar_item(
                                ui,
                                theme,
                                Icon::ChartSpline,
                                "Technicals",
                                self.section == Section::Technicals,
                            ) {
                                self.section = Section::Technicals;
                                self.selected = None;
                            }
                        });
                    });
                    let r = ui.max_rect();
                    ui.painter().vline(
                        r.left() + SIDEBAR_W,
                        r.top()..=r.bottom(),
                        Stroke::new(1.0, theme.border),
                    );
                    ui.vertical(|ui| {
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.label(
                                RichText::new("SCRIPT NAME")
                                    .size(11.5)
                                    .color(theme.text_muted),
                            );
                        });
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if rows.is_empty() {
                                    ui.add_space(24.0);
                                    ui.vertical_centered(|ui| {
                                        let msg = if self.section == Section::Favorites
                                            && self.query.is_empty()
                                        {
                                            "Star indicators to add them to your favorites"
                                        } else {
                                            "No indicators match your criteria"
                                        };
                                        ui.label(RichText::new(msg).color(theme.text_muted));
                                    });
                                }
                                if let Some(k) = self.rows_ui(ui, theme, &rows, favorites) {
                                    picked = Some(k);
                                }
                            });
                    });
                });
            });
        if modal.should_close() {
            self.open = false;
        }
        picked
    }

    fn rows_ui(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        rows: &[StudyKind],
        favorites: &mut Vec<StudyKind>,
    ) -> Option<StudyKind> {
        let mut picked = None;
        let pointer_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        let ctx = ui.ctx().clone();
        for (i, &kind) in rows.iter().enumerate() {
            let (rect, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
            let star_rect = egui::Rect::from_center_size(
                pos2(rect.left() + 22.0, rect.center().y),
                vec2(24.0, 24.0),
            );
            let star = ui.interact(star_rect, Id::unique(("ind_star", kind)), Sense::click());
            if (resp.hovered() || star.hovered()) && pointer_moved {
                self.selected = Some(i);
            }
            let selected = self.selected == Some(i);
            let p = ui.painter();
            if selected {
                p.rect_filled(rect.shrink2(vec2(8.0, 1.0)), 6.0, theme.selection);
            }
            let fav = favorites.contains(&kind);
            if fav {
                crate::tfmenu::filled_star(
                    p,
                    star_rect.center(),
                    6.5,
                    Color32::from_rgb(0xf5, 0xb5, 0x2e),
                );
            } else if selected {
                let c = if star.hovered() {
                    theme.text
                } else {
                    theme.text_muted
                };
                icons::paint(p, star_rect.center(), Icon::Star, 15.0, c);
            }
            let job = highlighted(kind.name(), self.query.trim(), 14.0, theme.text, theme);
            let galley = ctx.fonts_mut(|f| f.layout_job(job));
            p.galley(
                pos2(rect.left() + 42.0, rect.center().y - galley.size().y / 2.0),
                galley,
                theme.text,
            );
            if star.clicked() {
                if fav {
                    favorites.retain(|k| *k != kind);
                } else {
                    favorites.push(kind);
                }
            } else if resp.clicked() {
                picked = Some(kind);
            }
        }
        picked
    }
}

fn dialog_frame(theme: &Theme) -> Frame {
    Frame::new()
        .fill(theme.panel)
        .stroke(Stroke::new(1.0, theme.border))
        .corner_radius(10)
        .inner_margin(Margin::same(0))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 0,
            color: with_alpha(Color32::BLACK, if theme.dark { 0.6 } else { 0.18 }),
        })
}

fn backdrop(theme: &Theme) -> Color32 {
    with_alpha(Color32::BLACK, if theme.dark { 0.55 } else { 0.25 })
}

/// Title row with a close button; returns `true` when closed.
fn header(ui: &mut egui::Ui, theme: &Theme, title: &str) -> bool {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());
    ui.painter().text(
        pos2(rect.left() + 20.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(19.0),
        theme.text,
    );
    let close_rect =
        egui::Rect::from_center_size(pos2(rect.right() - 26.0, rect.center().y), vec2(30.0, 30.0));
    let close = ui.interact(
        close_rect,
        ui.make_persistent_id(("dialog_close", title)),
        Sense::click(),
    );
    if close.hovered() {
        ui.painter().rect_filled(close_rect, 6.0, theme.selection);
    }
    icons::paint(ui.painter(), close_rect.center(), Icon::X, 18.0, theme.text);
    close.clicked()
}

fn separator(ui: &mut egui::Ui, theme: &Theme) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .hline(r.x_range(), r.center().y, Stroke::new(1.0, theme.border));
}

fn sidebar_heading(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        ui.label(RichText::new(text).size(11.0).color(theme.text_muted));
    });
    ui.add_space(4.0);
}

fn sidebar_item(ui: &mut egui::Ui, theme: &Theme, icon: Icon, label: &str, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(SIDEBAR_W, 36.0), Sense::click());
    let r = rect.shrink2(vec2(8.0, 2.0));
    let p = ui.painter();
    if selected {
        p.rect_filled(r, 6.0, theme.selection);
    } else if resp.hovered() {
        p.rect_filled(r, 6.0, with_alpha(theme.selection, 0.6));
    }
    icons::paint(
        p,
        pos2(r.left() + 18.0, r.center().y),
        icon,
        16.0,
        theme.text,
    );
    p.text(
        pos2(r.left() + 38.0, r.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        theme.text,
    );
    resp.clicked()
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Inputs,
    Style,
}

/// Snapshot restored by Cancel.
struct Original {
    study: Study,
    visible: bool,
    colors: Vec<Color32>,
}

/// Settings dialog of one indicator. Changes apply live.
pub struct IndicatorSettings {
    target: Option<u64>,
    original: Option<Original>,
    tab: SettingsTab,
}

impl Default for IndicatorSettings {
    fn default() -> Self {
        Self {
            target: None,
            original: None,
            tab: SettingsTab::Inputs,
        }
    }
}

impl IndicatorSettings {
    pub fn open(&mut self, chart: &ChartView, id: u64) {
        let Some(ind) = chart.indicators.iter().find(|i| i.id == id) else {
            return;
        };
        self.target = Some(id);
        self.original = Some(Original {
            study: ind.study.clone(),
            visible: ind.visible,
            colors: ind.colors.clone(),
        });
        self.tab = SettingsTab::Inputs;
    }

    pub fn is_open(&self) -> bool {
        self.target.is_some()
    }

    pub fn ui(&mut self, ctx: &egui::Context, theme: &Theme, chart: &mut ChartView) {
        let Some(id) = self.target else { return };
        let Some(ind) = chart.indicator_mut(id) else {
            self.target = None;
            return;
        };
        let (esc, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::Escape),
                i.consume_key(egui::Modifiers::NONE, Key::Enter),
            )
        });
        let mut outcome = if esc {
            Some(false)
        } else if enter {
            Some(true)
        } else {
            None
        };

        let modal = egui::Modal::new(Id::unique("indicator_settings_modal"))
            .frame(dialog_frame(theme))
            .backdrop_color(backdrop(theme))
            .show(ctx, |ui| {
                ui.set_width(440.0);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                if header(ui, theme, &short_title(&ind.study)) {
                    outcome = Some(false);
                }
                // Tabs.
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    for (tab, label) in [
                        (SettingsTab::Inputs, "Inputs"),
                        (SettingsTab::Style, "Style"),
                    ] {
                        if tab_button(ui, theme, label, self.tab == tab) {
                            self.tab = tab;
                        }
                    }
                });
                separator(ui, theme);
                Frame::new()
                    .inner_margin(Margin::symmetric(20, 14))
                    .show(ui, |ui| {
                        ui.set_min_height(220.0);
                        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                        egui::Grid::new("indicator_settings_grid")
                            .num_columns(2)
                            .spacing(vec2(24.0, 12.0))
                            .min_col_width(150.0)
                            .show(ui, |ui| match self.tab {
                                SettingsTab::Inputs => inputs_ui(ui, &mut ind.study),
                                SettingsTab::Style => style_ui(ui, ind),
                            });
                    });
                separator(ui, theme);
                // Footer: Defaults ▾ · Cancel · Ok.
                Frame::new()
                    .inner_margin(Margin::symmetric(18, 12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                            let defaults = ui.add(footer_button("Defaults      ", theme, false));
                            icons::paint(
                                ui.painter(),
                                pos2(defaults.rect.right() - 16.0, defaults.rect.center().y),
                                Icon::ChevronDown,
                                15.0,
                                theme.text,
                            );
                            egui::Popup::menu(&defaults).show(|ui| {
                                if ui.button("Reset settings").clicked() {
                                    ind.study = ind.study.kind().default_study();
                                    ind.colors = default_colors(&ind.study);
                                    ui.close();
                                }
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.add(footer_button("Ok", theme, true)).clicked() {
                                        outcome = Some(true);
                                    }
                                    if ui.add(footer_button("Cancel", theme, false)).clicked() {
                                        outcome = Some(false);
                                    }
                                },
                            );
                        });
                    });
            });
        if modal.should_close() && outcome.is_none() {
            outcome = Some(false);
        }
        if let Some(ok) = outcome {
            if !ok && let Some(o) = self.original.take() {
                ind.study = o.study;
                ind.visible = o.visible;
                ind.colors = o.colors;
            }
            self.target = None;
            self.original = None;
        }
    }
}

/// Dialog title, e.g. "RSI" / "Vol" (TradingView uses the short name).
fn short_title(study: &Study) -> String {
    study
        .legend_title()
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_owned()
}

fn tab_button(ui: &mut egui::Ui, theme: &Theme, label: &str, selected: bool) -> bool {
    let font = FontId::proportional(14.0);
    let w = ui
        .ctx()
        .fonts_mut(|f| f.layout_no_wrap(label.into(), font.clone(), theme.text))
        .size()
        .x;
    let (rect, resp) = ui.allocate_exact_size(vec2(w + 20.0, 40.0), Sense::click());
    let color = if selected || resp.hovered() {
        theme.text
    } else {
        theme.text_muted
    };
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, label, font, color);
    if selected {
        let r = egui::Rect::from_min_max(
            pos2(rect.left() + 6.0, rect.bottom() - 3.0),
            pos2(rect.right() - 6.0, rect.bottom()),
        );
        ui.painter().rect_filled(r, 1.5, theme.text);
    }
    resp.clicked()
}

fn footer_button<'a>(label: &'a str, theme: &Theme, primary: bool) -> egui::Button<'a> {
    let (fill, fg, stroke) = if primary {
        (theme.text, theme.bg, Stroke::NONE)
    } else {
        (
            Color32::TRANSPARENT,
            theme.text,
            Stroke::new(1.0, theme.border),
        )
    };
    egui::Button::new(RichText::new(label).size(14.0).color(fg))
        .fill(fill)
        .stroke(stroke)
        .corner_radius(6)
        .min_size(vec2(72.0, 34.0))
}

fn int_row(ui: &mut egui::Ui, label: &str, v: &mut u32, max: u32) {
    ui.label(label);
    ui.add(egui::DragValue::new(v).range(1..=max).speed(0.2));
    ui.end_row();
}

fn source_row(ui: &mut egui::Ui, v: &mut Source) {
    ui.label("Source");
    egui::ComboBox::from_id_salt("indicator_source")
        .selected_text(v.label())
        .show_ui(ui, |ui| {
            for s in Source::ALL {
                ui.selectable_value(v, s, s.label());
            }
        });
    ui.end_row();
}

fn inputs_ui(ui: &mut egui::Ui, study: &mut Study) {
    match study {
        Study::Volume {
            ma_length,
            color_by_prev_close,
            ..
        } => {
            int_row(ui, "MA Length", ma_length, 500);
            ui.label("");
            ui.checkbox(color_by_prev_close, "Color based on previous close");
            ui.end_row();
        }
        Study::Sma { length, source } | Study::Ema { length, source } => {
            int_row(ui, "Length", length, 2000);
            source_row(ui, source);
        }
        Study::BollingerBands {
            length,
            mult,
            source,
        } => {
            int_row(ui, "Length", length, 2000);
            source_row(ui, source);
            ui.label("StdDev");
            ui.add(
                egui::DragValue::new(mult)
                    .range(0.1..=10.0)
                    .speed(0.05)
                    .fixed_decimals(1),
            );
            ui.end_row();
        }
        Study::Vwap => {
            ui.label("Anchor period");
            ui.label("Session (UTC day)");
            ui.end_row();
            ui.label("Source");
            ui.label("hlc3");
            ui.end_row();
        }
        Study::Rsi { length, source } => {
            int_row(ui, "RSI Length", length, 2000);
            source_row(ui, source);
        }
        Study::Macd {
            fast,
            slow,
            signal,
            source,
        } => {
            int_row(ui, "Fast Length", fast, 2000);
            int_row(ui, "Slow Length", slow, 2000);
            source_row(ui, source);
            int_row(ui, "Signal Smoothing", signal, 500);
        }
        Study::Stochastic { k, k_smooth, d } => {
            int_row(ui, "%K Length", k, 2000);
            int_row(ui, "%K Smoothing", k_smooth, 500);
            int_row(ui, "%D Smoothing", d, 500);
        }
        Study::Atr { length } => int_row(ui, "Length", length, 2000),
        Study::Obv => {
            ui.label("No inputs");
            ui.end_row();
        }
    }
}

fn style_ui(ui: &mut egui::Ui, ind: &mut Indicator) {
    let names: Vec<&str> = ind.study.outputs().iter().map(|(n, _)| *n).collect();
    if ind.colors.len() < names.len() {
        let defaults = default_colors(&ind.study);
        ind.colors
            .extend(defaults.into_iter().skip(ind.colors.len()));
    }
    for (k, name) in names.iter().enumerate() {
        match (&mut ind.study, k) {
            // Volume columns follow the chart's up/down colors.
            (Study::Volume { .. }, 0) => {
                ui.label("Volume");
                ui.label(RichText::new("Up / down colors").weak());
            }
            (Study::Volume { show_ma, .. }, 1) => {
                ui.checkbox(show_ma, "Volume MA");
                ui.color_edit_button_srgba(&mut ind.colors[k]);
            }
            _ => {
                ui.label(*name);
                ui.color_edit_button_srgba(&mut ind.colors[k]);
            }
        }
        ui.end_row();
    }
    ui.label("");
    ui.checkbox(&mut ind.visible, "Visible");
    ui.end_row();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_matches_names_and_aliases_alphabetically() {
        let all = matching("", None);
        assert_eq!(all.len(), StudyKind::ALL.len());
        assert_eq!(
            all[0],
            StudyKind::Atr,
            "alphabetical: Average True Range first"
        );
        assert_eq!(matching("rsi", None), [StudyKind::Rsi]);
        assert_eq!(matching("Moving", None), [StudyKind::Ema, StudyKind::Sma]);
        assert!(matching("vol", None).contains(&StudyKind::Volume));
        assert!(
            matching("vol", None).contains(&StudyKind::Vwap),
            "Volume Weighted…"
        );
        assert_eq!(matching("", Some(&[StudyKind::Macd])), [StudyKind::Macd]);
        assert!(matching("zzz", None).is_empty());
    }

    #[test]
    fn short_titles() {
        assert_eq!(short_title(&StudyKind::Volume.default_study()), "Vol");
        assert_eq!(short_title(&StudyKind::Rsi.default_study()), "RSI");
    }
}
