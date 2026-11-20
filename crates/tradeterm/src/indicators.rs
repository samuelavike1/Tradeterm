//! TradingView-style indicator dialogs:
//!
//! - [`IndicatorsPanel`] ("Indicators, metrics, and strategies"): search,
//!   Favorites / Technicals sidebar, alphabetical list with stars. Clicking a
//!   row adds the indicator; the dialog stays open.
//! - [`IndicatorSettingsPanel`]: one indicator's Inputs / Style, applied
//!   live; Cancel (or closing) restores what it was, Defaults resets.

use crate::app::{TabChart, TradeTerm};
use crate::search::match_ranges;
use ecolor::Color32;
use gpui_kit::component::button::*;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{Input, InputEvent, InputState, NumberInput};
use gpui_kit::component::menu::*;
use gpui_kit::component::{Icon, IconName, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use tt_chart::Indicator;
use tt_chart::indicators::default_colors;
use tt_chart_gpui::hsla;
use tt_core::{Source, Study, StudyKind};
use tt_theme::Theme;

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
pub fn matching(query: &str, only: Option<&[StudyKind]>) -> Vec<StudyKind> {
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

/// "Indicators, metrics, and strategies".
pub struct IndicatorsPanel {
    app: WeakEntity<TradeTerm>,
    theme: Theme,
    input: Entity<InputState>,
    query: String,
    section: Section,
    favorites: Vec<StudyKind>,
    selected: Option<usize>,
    _sub: Subscription,
}

impl IndicatorsPanel {
    pub fn new(
        app: WeakEntity<TradeTerm>,
        theme: Theme,
        favorites: Vec<StudyKind>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let _sub = cx.subscribe(&input, |this, input, ev: &InputEvent, cx| match ev {
            InputEvent::Change => {
                this.query = input.read(cx).value().to_string();
                this.selected = None;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                let rows = this.rows();
                let pick = this
                    .selected
                    .or((rows.len() == 1).then_some(0))
                    .and_then(|i| rows.get(i).copied());
                if let Some(k) = pick {
                    this.add(k, cx);
                }
            }
            _ => {}
        });
        Self {
            app,
            theme,
            input,
            query: String::new(),
            section: Section::Technicals,
            favorites,
            selected: None,
            _sub,
        }
    }

    pub fn focus_input(this: &Entity<Self>, window: &mut Window, cx: &mut App) {
        let input = this.read(cx).input.clone();
        let handle = input.read(cx).focus_handle(cx);
        handle.focus(window, cx);
    }

    fn rows(&self) -> Vec<StudyKind> {
        let only = (self.section == Section::Favorites).then_some(self.favorites.as_slice());
        matching(&self.query, only)
    }

    fn add(&mut self, kind: StudyKind, cx: &mut Context<Self>) {
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |app, cx| app.add_indicator(kind, cx)).ok();
        });
    }

    fn toggle_favorite(&mut self, kind: StudyKind, cx: &mut Context<Self>) {
        if let Some(i) = self.favorites.iter().position(|k| *k == kind) {
            self.favorites.remove(i);
        } else {
            self.favorites.push(kind);
        }
        let favorites = self.favorites.clone();
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |app, cx| app.set_favorite_indicators(favorites, cx))
                .ok();
        });
        cx.notify();
    }

    fn sidebar_item(
        &self,
        icon: IconName,
        label: &'static str,
        section: Section,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &self.theme;
        let selected = self.section == section;
        h_flex()
            .id(label)
            .h(px(36.))
            .mx_2()
            .px_3()
            .gap_2()
            .rounded(px(6.))
            .cursor_pointer()
            .when(selected, |d| d.bg(hsla(t.selection)))
            .hover(|d| d.bg(hsla(t.selection)))
            .child(Icon::new(icon).size_4())
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.section = section;
                this.selected = None;
                cx.notify();
            }))
    }
}

impl Render for IndicatorsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme.clone();
        let muted = hsla(t.text_muted);
        let accent = hsla(t.series_palette[0]);
        let gold = hsla(Color32::from_rgb(0xf5, 0xb5, 0x2e));
        let rows = self.rows();

        let mut list = v_flex().id("indicator-list").flex_1().overflow_y_scroll();
        if rows.is_empty() {
            let msg = if self.section == Section::Favorites && self.query.is_empty() {
                "Star indicators to add them to your favorites"
            } else {
                "No indicators match your criteria"
            };
            list = list.child(div().p_6().text_color(muted).child(msg));
        }
        for (i, kind) in rows.into_iter().enumerate() {
            let fav = self.favorites.contains(&kind);
            let selected = self.selected == Some(i);
            let name = kind.name().to_owned();
            let ranges = match_ranges(&name, self.query.trim());
            let group = SharedString::from(format!("ind-row-{i}"));
            list = list.child(
                h_flex()
                    .id(("indicator", i))
                    .group(group.clone())
                    .h(px(36.))
                    .flex_none()
                    .mx_2()
                    .px_2()
                    .gap_2()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(selected, |d| d.bg(hsla(t.selection)))
                    .hover(|d| d.bg(hsla(t.selection)))
                    .child(
                        div()
                            .id(("star", i))
                            .size(px(22.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .when(fav, |d| d.text_color(gold))
                            .when(!fav, |d| {
                                d.text_color(muted)
                                    .invisible()
                                    .group_hover(group.clone(), |s| s.visible())
                            })
                            .child(
                                Icon::new(if fav {
                                    IconName::StarFill
                                } else {
                                    IconName::Star
                                })
                                .size_4(),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.toggle_favorite(kind, cx)
                            })),
                    )
                    .child(
                        StyledText::new(name).with_highlights(ranges.into_iter().map(|r| {
                            (
                                r,
                                HighlightStyle {
                                    color: Some(accent),
                                    ..Default::default()
                                },
                            )
                        })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.add(kind, cx))),
            );
        }

        let heading = |text: &'static str| {
            div()
                .px_5()
                .pt_3()
                .pb_1()
                .text_size(px(11.))
                .text_color(muted)
                .child(text)
        };
        v_flex()
            .h(px(480.))
            .gap_2()
            .capture_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                let n = this.rows().len();
                if n == 0 {
                    return;
                }
                let last = n - 1;
                this.selected = Some(match (e.keystroke.key.as_str(), this.selected) {
                    ("down", None) => 0,
                    ("up", None) => last,
                    ("down", Some(i)) => (i + 1).min(last),
                    ("up", Some(i)) => i.saturating_sub(1),
                    _ => return,
                });
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                Input::new(&self.input)
                    .prefix(Icon::new(IconName::Search))
                    .cleanable(true),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .border_t_1()
                    .border_color(hsla(t.border))
                    .child(
                        v_flex()
                            .w(px(200.))
                            .h_full()
                            .border_r_1()
                            .border_color(hsla(t.border))
                            .child(heading("PERSONAL"))
                            .child(self.sidebar_item(
                                IconName::Star,
                                "Favorites",
                                Section::Favorites,
                                cx,
                            ))
                            .child(heading("BUILT-IN"))
                            .child(self.sidebar_item(
                                IconName::ChartPie,
                                "Technicals",
                                Section::Technicals,
                                cx,
                            )),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .child(heading("SCRIPT NAME"))
                            .child(list),
                    ),
            )
    }
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Inputs,
    Style,
}

type Setter = fn(&mut Study, f64);

/// A numeric input of the indicator's study.
struct NumberField {
    label: &'static str,
    input: Entity<InputState>,
}

/// Snapshot restored by Cancel / closing.
#[derive(Clone)]
struct Original {
    study: Study,
    visible: bool,
    colors: Vec<Color32>,
}

pub struct IndicatorSettingsPanel {
    chart: TabChart,
    id: u64,
    theme: Theme,
    original: Original,
    tab: SettingsTab,
    numbers: Vec<NumberField>,
    colors: Vec<(&'static str, Entity<ColorPickerState>)>,
    committed: bool,
    _subs: Vec<Subscription>,
}

/// Numeric fields of a study: (label, value, min, max, step, setter).
fn number_fields(study: &Study) -> Vec<(&'static str, f64, f64, f64, f64, Setter)> {
    fn set_u32(v: f64) -> u32 {
        v.round().clamp(1.0, 5000.0) as u32
    }
    match study {
        Study::Volume { ma_length, .. } => {
            vec![("MA Length", *ma_length as f64, 1.0, 500.0, 1.0, |s, v| {
                if let Study::Volume { ma_length, .. } = s {
                    *ma_length = set_u32(v)
                }
            })]
        }
        Study::Sma { length, .. } | Study::Ema { length, .. } => {
            vec![("Length", *length as f64, 1.0, 2000.0, 1.0, |s, v| {
                if let Study::Sma { length, .. } | Study::Ema { length, .. } = s {
                    *length = set_u32(v)
                }
            })]
        }
        Study::BollingerBands { length, mult, .. } => vec![
            ("Length", *length as f64, 1.0, 2000.0, 1.0, |s, v| {
                if let Study::BollingerBands { length, .. } = s {
                    *length = set_u32(v)
                }
            }),
            ("StdDev", *mult, 0.1, 10.0, 0.1, |s, v| {
                if let Study::BollingerBands { mult, .. } = s {
                    *mult = (v * 10.0).round().clamp(1.0, 100.0) / 10.0
                }
            }),
        ],
        Study::Rsi { length, .. } => {
            vec![("RSI Length", *length as f64, 1.0, 2000.0, 1.0, |s, v| {
                if let Study::Rsi { length, .. } = s {
                    *length = set_u32(v)
                }
            })]
        }
        Study::Macd {
            fast, slow, signal, ..
        } => vec![
            ("Fast Length", *fast as f64, 1.0, 2000.0, 1.0, |s, v| {
                if let Study::Macd { fast, .. } = s {
                    *fast = set_u32(v)
                }
            }),
            ("Slow Length", *slow as f64, 1.0, 2000.0, 1.0, |s, v| {
                if let Study::Macd { slow, .. } = s {
                    *slow = set_u32(v)
                }
            }),
            (
                "Signal Smoothing",
                *signal as f64,
                1.0,
                500.0,
                1.0,
                |s, v| {
                    if let Study::Macd { signal, .. } = s {
                        *signal = set_u32(v)
                    }
                },
            ),
        ],
        Study::Stochastic { k, k_smooth, d } => vec![
            ("%K Length", *k as f64, 1.0, 2000.0, 1.0, |s, v| {
                if let Study::Stochastic { k, .. } = s {
                    *k = set_u32(v)
                }
            }),
            ("%K Smoothing", *k_smooth as f64, 1.0, 500.0, 1.0, |s, v| {
                if let Study::Stochastic { k_smooth, .. } = s {
                    *k_smooth = set_u32(v)
                }
            }),
            ("%D Smoothing", *d as f64, 1.0, 500.0, 1.0, |s, v| {
                if let Study::Stochastic { d, .. } = s {
                    *d = set_u32(v)
                }
            }),
        ],
        Study::Atr { length } => vec![("Length", *length as f64, 1.0, 2000.0, 1.0, |s, v| {
            if let Study::Atr { length } = s {
                *length = set_u32(v)
            }
        })],
        Study::Vwap | Study::Obv => Vec::new(),
    }
}

fn source_of(study: &Study) -> Option<Source> {
    match study {
        Study::Sma { source, .. }
        | Study::Ema { source, .. }
        | Study::BollingerBands { source, .. }
        | Study::Rsi { source, .. }
        | Study::Macd { source, .. } => Some(*source),
        _ => None,
    }
}

fn set_source(study: &mut Study, s: Source) {
    if let Study::Sma { source, .. }
    | Study::Ema { source, .. }
    | Study::BollingerBands { source, .. }
    | Study::Rsi { source, .. }
    | Study::Macd { source, .. } = study
    {
        *source = s;
    }
}

fn color32(c: Hsla) -> Color32 {
    let r = Rgba::from(c);
    Color32::from_rgba_unmultiplied(
        (r.r * 255.0).round() as u8,
        (r.g * 255.0).round() as u8,
        (r.b * 255.0).round() as u8,
        (r.a * 255.0).round() as u8,
    )
}

impl IndicatorSettingsPanel {
    /// `None` if the indicator no longer exists.
    pub fn new(
        chart: TabChart,
        id: u64,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Self> {
        let ind = chart
            .read(cx)
            .state
            .read(cx)
            .source
            .tab
            .chart
            .indicators
            .iter()
            .find(|i| i.id == id)?
            .clone();
        let original = Original {
            study: ind.study.clone(),
            visible: ind.visible,
            colors: ind.colors.clone(),
        };
        let mut subs = Vec::new();
        let mut numbers = Vec::new();
        for (label, value, min, max, step, setter) in number_fields(&ind.study) {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .step(step)
                    .min(min)
                    .max(max)
                    .default_value(trim_number(value))
            });
            subs.push(
                cx.subscribe(&input, move |this, input, ev: &InputEvent, cx| {
                    if let InputEvent::Change = ev
                        && let Ok(v) = input.read(cx).value().trim().parse::<f64>()
                    {
                        this.edit(cx, |ind| setter(&mut ind.study, v));
                    }
                }),
            );
            numbers.push(NumberField { label, input });
        }
        let names: Vec<&'static str> = ind.study.outputs().iter().map(|(n, _)| *n).collect();
        let mut colors = Vec::new();
        for (k, name) in names.into_iter().enumerate() {
            if matches!(ind.study, Study::Volume { .. }) && k == 0 {
                continue; // columns follow the chart's up/down colors
            }
            let picker =
                cx.new(|cx| ColorPickerState::new(window, cx).default_value(hsla(ind.color(k))));
            subs.push(
                cx.subscribe(&picker, move |this, _, ev: &ColorPickerEvent, cx| {
                    let ColorPickerEvent::Change(Some(c)) = ev else {
                        return;
                    };
                    let c = color32(*c);
                    this.edit(cx, |ind| {
                        if ind.colors.len() <= k {
                            ind.colors.resize(k + 1, c);
                        }
                        ind.colors[k] = c;
                    });
                }),
            );
            colors.push((name, picker));
        }
        // The dialog dropped this panel: closing any way but Ok (×, Esc,
        // Cancel, clicking outside) restores the indicator.
        subs.push(cx.on_release(|this, cx| {
            if !this.committed {
                this.restore(cx);
            }
        }));
        Some(Self {
            chart,
            id,
            theme,
            original,
            tab: SettingsTab::Inputs,
            numbers,
            colors,
            committed: false,
            _subs: subs,
        })
    }

    pub fn title(&self) -> String {
        self.original
            .study
            .legend_title()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_owned()
    }

    /// Changes the indicator live and repaints the chart.
    fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Indicator)) {
        let id = self.id;
        self.chart.update(cx, |chart, cx| {
            chart.update_state(cx, |s| {
                if let Some(ind) = s.source.tab.chart.indicator_mut(id) {
                    f(ind);
                }
            })
        });
        cx.notify();
    }

    fn current(&self, cx: &App) -> Option<Indicator> {
        self.chart
            .read(cx)
            .state
            .read(cx)
            .source
            .tab
            .chart
            .indicators
            .iter()
            .find(|i| i.id == self.id)
            .cloned()
    }

    /// Ok: keep the changes.
    pub fn commit(&mut self) {
        self.committed = true;
    }

    /// Restores the indicator as it was before the dialog opened.
    fn restore(&self, cx: &mut App) {
        let (id, o) = (self.id, self.original.clone());
        self.chart.update(cx, |chart, cx| {
            chart.update_state(cx, |s| {
                if let Some(ind) = s.source.tab.chart.indicator_mut(id) {
                    ind.study = o.study;
                    ind.visible = o.visible;
                    ind.colors = o.colors;
                }
            })
        });
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let study = self.original.study.kind().default_study();
        let colors = default_colors(&study);
        let values: Vec<f64> = number_fields(&study).iter().map(|f| f.1).collect();
        for (field, v) in self.numbers.iter().zip(values) {
            field
                .input
                .update(cx, |s, cx| s.set_value(trim_number(v), window, cx));
        }
        self.edit(cx, |ind| {
            ind.study = study;
            ind.colors = colors;
        });
    }

    fn tab_button(
        &self,
        tab: SettingsTab,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &self.theme;
        let selected = self.tab == tab;
        div()
            .id(label)
            .px_3()
            .pb_2()
            .pt_1()
            .cursor_pointer()
            .text_color(hsla(if selected { t.text } else { t.text_muted }))
            .hover(|s| s.text_color(hsla(t.text)))
            .when(selected, |d| d.border_b_2().border_color(hsla(t.text)))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.tab = tab;
                cx.notify();
            }))
    }
}

/// "14", "2.5" (no trailing zeros).
fn trim_number(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v}")
    }
}

impl Render for IndicatorSettingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(ind) = self.current(cx) else {
            return div().child("This indicator was removed.");
        };
        let t = self.theme.clone();
        let row = |label: &'static str| {
            h_flex().h(px(36.)).gap_4().child(
                div()
                    .w(px(150.))
                    .text_color(hsla(t.text_muted))
                    .child(label),
            )
        };
        let mut body = v_flex().gap_1().min_h(px(220.)).pt_2();
        match self.tab {
            SettingsTab::Inputs => {
                for field in &self.numbers {
                    body = body.child(
                        row(field.label).child(
                            div()
                                .w(px(140.))
                                .child(NumberInput::new(&field.input).small()),
                        ),
                    );
                }
                if let Some(src) = source_of(&ind.study) {
                    let this = cx.entity().downgrade();
                    body = body.child(
                        row("Source").child(
                            Button::new("source")
                                .outline()
                                .small()
                                .label(src.label())
                                .dropdown_caret(true)
                                .dropdown_menu(move |mut menu, _, _| {
                                    for s in Source::ALL {
                                        let this = this.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(s.label())
                                                .checked(s == src)
                                                .on_click(move |_, _, cx| {
                                                    this.update(cx, |p, cx| {
                                                        p.edit(cx, |ind| {
                                                            set_source(&mut ind.study, s)
                                                        })
                                                    })
                                                    .ok();
                                                }),
                                        );
                                    }
                                    menu
                                }),
                        ),
                    );
                }
                if let Study::Volume {
                    color_by_prev_close,
                    ..
                } = ind.study
                {
                    body = body.child(
                        Checkbox::new("prev-close")
                            .label("Color based on previous close")
                            .checked(color_by_prev_close)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                let v = *checked;
                                this.edit(cx, |ind| {
                                    if let Study::Volume {
                                        color_by_prev_close,
                                        ..
                                    } = &mut ind.study
                                    {
                                        *color_by_prev_close = v
                                    }
                                })
                            })),
                    );
                }
                if matches!(ind.study, Study::Vwap) {
                    body = body.child(row("Anchor period").child("Session (UTC day)"));
                }
                if matches!(ind.study, Study::Obv) {
                    body = body.child(div().text_color(hsla(t.text_muted)).child("No inputs"));
                }
            }
            SettingsTab::Style => {
                if let Study::Volume { show_ma, .. } = ind.study {
                    body = body.child(
                        Checkbox::new("show-ma")
                            .label("Volume MA")
                            .checked(show_ma)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                let v = *checked;
                                this.edit(cx, |ind| {
                                    if let Study::Volume { show_ma, .. } = &mut ind.study {
                                        *show_ma = v
                                    }
                                })
                            })),
                    );
                }
                for (name, picker) in &self.colors {
                    body = body.child(row(name).child(ColorPicker::new(picker).small()));
                }
                body = body.child(
                    Checkbox::new("visible")
                        .label("Visible")
                        .checked(ind.visible)
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            let v = *checked;
                            this.edit(cx, |ind| ind.visible = v)
                        })),
                );
            }
        }
        let this = cx.entity().downgrade();
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .border_b_1()
                    .border_color(hsla(t.border))
                    .child(self.tab_button(SettingsTab::Inputs, "Inputs", cx))
                    .child(self.tab_button(SettingsTab::Style, "Style", cx)),
            )
            .child(body)
            .child(
                h_flex()
                    .pt_3()
                    .gap_2()
                    .border_t_1()
                    .border_color(hsla(t.border))
                    .child(
                        Button::new("defaults")
                            .outline()
                            .label("Defaults")
                            .dropdown_caret(true)
                            .dropdown_menu(move |menu, _, _| {
                                let this = this.clone();
                                menu.item(PopupMenuItem::new("Reset settings").on_click(
                                    move |_, window, cx| {
                                        this.update(cx, |p, cx| p.reset(window, cx)).ok();
                                    },
                                ))
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("cancel")
                            .outline()
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("ok")
                            .primary()
                            .label("Ok")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.commit();
                                window.close_dialog(cx);
                            })),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn search_matches_names_and_aliases_alphabetically() {
        let all = matching("", None);
        assert_eq!(all.len(), StudyKind::ALL.len());
        assert_eq!(all[0], StudyKind::Atr);
        assert_eq!(matching("rsi", None), [StudyKind::Rsi]);
        assert_eq!(matching("Moving", None), [StudyKind::Ema, StudyKind::Sma]);
        assert_eq!(matching("", Some(&[StudyKind::Macd])), [StudyKind::Macd]);
    }

    #[::core::prelude::v1::test]
    fn numbers_trim() {
        assert_eq!(trim_number(14.0), "14");
        assert_eq!(trim_number(2.5), "2.5");
    }
}
