//! One indicator's settings: TradingView's indicator settings dialog
//! (docs/specs/indicators-tv-ref.md), on the comfortable density of
//! ui-guidelines §2.10, generated from the study metadata
//! ([`tt_core::StudyMeta`]) and the per-plot style ([`IndicatorStyle`]):
//!
//! - **Inputs**: every `InputDef` in order: groups as uppercase captions,
//!   labels on the left, controls (34 px, 100 wide) in a right-hand
//!   column, an ⓘ after inputs with a tooltip, inline runs on one row,
//!   `enabled_if` dimming. A Bool input titled like a plot ("Volume MA") is
//!   that plot's checkbox on the Style tab instead.
//! - **Style**: per plot `[x] title [color + line] [plot type ▾]` (the plot
//!   type menu carries "Price line"); multi-color plots get a checkbox row
//!   and one indented row per color; levels `[x] title [line] [value]`;
//!   fills `[x] title [swatch]`; then OUTPUT VALUES (Precision, Labels on
//!   price scale, Values in status line) and INPUT VALUES (Inputs in
//!   status line).
//! - **Visibility**: Ticks, per unit `[x] [from] slider [to]`, Ranges (the
//!   drawing dialogs' rows, [`crate::visibility_tv`]).
//!
//! Every change applies live. Cancel (×, Esc, clicking outside) restores the
//! indicator exactly as it was; Ok keeps it. The footer's "Defaults ▾" menu
//! resets to factory settings, saves / applies / removes the user default of
//! the kind (new instances start from it) and manages named presets.
//!
//! Number fields are the only widget states (created once per dialog); after
//! a bulk change (reset, default, preset) [`Self::sync`] writes every one of
//! them from the indicator again. Colors, lines and sliders are stateless
//! kit controls drawn from the indicator each frame.

use crate::app::{TabChart, TradeTerm};
use crate::visibility_tv::{self, W_VIS_DIALOG};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use lucide_icons::Icon as Lucide;
use std::cell::Cell;
use std::rc::Rc;
use tt_chart::indicator_style::{IntervalVisibility, MAX_PRECISION};
use tt_chart::{Indicator, IndicatorStyle, Rgba};
use tt_core::{
    InputDef, InputKind, InputValue, LineStyle, PlotDef, PlotKind, PlotType, Source, Study,
    StudyKind, StudyMeta, TimeUnit, Timeframe,
};
use tt_session::settings::{IndicatorPreset, NamedPreset};
use tt_theme::Theme;
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{DialogOpts, Menu};

/// The legacy size, for the small "Save preset" dialog.
pub const SIZE: DialogSize = DialogSize::Sm;

/// Multi-color plots: one row per color (TradingView: 42 tall).
const H_COLOR_ROW: f32 = 42.0;
/// Lone checkbox rows (TradingView: 34 tall).
const H_CHECK_ROW: f32 = C_CONTROL;
/// Space under a multi-color plot's rows.
const GAP_GROUP: f32 = 8.0;
/// Section captions ("OUTPUT VALUES"): 16 above, 32 tall.
const H_CAPTION: f32 = 32.0;
const PAD_CAPTION: f32 = 16.0;
/// Indented labels line up with checkbox labels.
const INDENT: f32 = C_CHECKBOX + C_GAP;
/// The ⓘ after an input with a tooltip (gap + icon).
const W_INFO: f32 = C_GAP + ICON_MD;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Inputs,
    Style,
    Visibility,
}

impl SettingsTab {
    fn id(self) -> &'static str {
        match self {
            SettingsTab::Inputs => "inputs",
            SettingsTab::Style => "style",
            SettingsTab::Visibility => "visibility",
        }
    }
}

/// What a number field edits.
#[derive(Debug, Clone, Copy, PartialEq)]
enum NumberSlot {
    Input(&'static str),
    HLine(&'static str),
    VisFrom(TimeUnit),
    VisTo(TimeUnit),
}

/// What a color control edits.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ColorSlot {
    Input(&'static str),
    /// Palette entry of a plot.
    Plot(&'static str, usize),
    HLine(&'static str),
    /// A fill's first (`false`) or second (gradient) color.
    Fill(&'static str, bool),
}

/// Snapshot restored by Cancel / closing.
#[derive(Debug, Clone, PartialEq)]
pub struct Original {
    pub study: Study,
    pub visible: bool,
    pub style: IndicatorStyle,
}

impl Original {
    pub fn of(ind: &Indicator) -> Self {
        Self {
            study: ind.study.clone(),
            visible: ind.visible,
            style: ind.style.clone(),
        }
    }

    pub fn restore(&self, ind: &mut Indicator) {
        ind.study = self.study.clone();
        ind.visible = self.visible;
        ind.style = self.style.clone();
    }
}

// ---- pure helpers (tested) --------------------------------------------------

/// A number field's text as the value of input `def` (`None` while it is
/// not a number, e.g. empty while typing). Ranges are applied by
/// [`Study::set`].
pub fn parse_input(def: &InputDef, text: &str) -> Option<InputValue> {
    let t = text.trim();
    match def.kind {
        InputKind::Int { .. } => t
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(|v| InputValue::Int(v.round() as i64)),
        InputKind::Float { .. } => t
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(InputValue::Float),
        _ => None,
    }
}

/// Applies a number field of input `id` to `study`. Returns whether it was
/// set (clamped to the input's range).
pub fn apply_number_input(study: &mut Study, id: &str, text: &str) -> bool {
    let Some(def) = study.meta().input(id) else {
        return false;
    };
    match parse_input(def, text) {
        Some(v) => study.set(id, v),
        None => false,
    }
}

/// Inputs, style and the eye of `ind` back to `preset` (factory, the user
/// default or a named preset): every input, plot, level, fill, precision,
/// status-line switch and visibility range.
pub fn reset_to(ind: &mut Indicator, preset: &IndicatorPreset) {
    preset.apply_to(ind);
    ind.visible = true;
}

/// The Bool input that switches plot `plot` on and off (TradingView shows
/// it as the plot's checkbox on the Style tab, e.g. Volume's "Volume MA").
pub fn plot_toggle(meta: &StudyMeta, plot: &PlotDef) -> Option<&'static InputDef> {
    meta.inputs
        .iter()
        .find(|d| matches!(d.kind, InputKind::Bool) && d.title == plot.title)
}

/// A source as TradingView's dialog spells it ("Close", "HL2").
pub fn source_label(s: Source) -> String {
    let l = s.label();
    if l.chars().any(|c| c.is_ascii_digit()) {
        l.to_ascii_uppercase()
    } else {
        let mut c = l.chars();
        c.next()
            .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
            .unwrap_or_default()
    }
}

/// "14", "2.5" (no trailing zeros).
fn trim_number(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// Text a number slot shows for `ind`.
fn slot_text(ind: &Indicator, slot: NumberSlot) -> String {
    match slot {
        NumberSlot::Input(id) => match ind.study.get(id) {
            Some(InputValue::Int(v)) => v.to_string(),
            Some(InputValue::Float(v)) => trim_number(v),
            _ => String::new(),
        },
        NumberSlot::HLine(id) => ind
            .study
            .meta()
            .hline(id)
            .map(|d| trim_number(ind.style.hline(d).value))
            .unwrap_or_default(),
        NumberSlot::VisFrom(u) | NumberSlot::VisTo(u) => {
            let (a, b) = vis_range(&ind.style.visibility, u);
            if matches!(slot, NumberSlot::VisFrom(_)) {
                a.to_string()
            } else {
                b.to_string()
            }
        }
    }
}

/// A unit's range, or its full range while unchecked.
fn vis_range(v: &IntervalVisibility, u: TimeUnit) -> (u16, u16) {
    v.get(u).unwrap_or(IntervalVisibility::full_range(u))
}

/// Color a control shows for `ind`. Volume columns that were never colored
/// follow the theme's volume colors, so the swatch shows those.
fn slot_color(ind: &Indicator, slot: ColorSlot, theme: &Theme) -> Rgba {
    let meta = ind.study.meta();
    match slot {
        ColorSlot::Input(id) => match ind.study.get(id) {
            Some(InputValue::Color(c)) => Rgba(c),
            _ => Rgba::new(0, 0, 0, 255),
        },
        ColorSlot::Plot(id, i) => {
            let Some(def) = meta.plot(id) else {
                return Rgba::new(0, 0, 0, 255);
            };
            let p = ind.style.plot(def);
            if def.kind == PlotKind::VolumeColumns && !p.is_overridden(i) {
                let c = if i == 0 {
                    theme.volume_down
                } else {
                    theme.volume_up
                };
                return Rgba::from_color32(c);
            }
            p.palette(i)
        }
        ColorSlot::HLine(id) => meta
            .hline(id)
            .map_or(Rgba::new(0, 0, 0, 255), |d| ind.style.hline(d).color),
        ColorSlot::Fill(id, second) => meta.fill(id).map_or(Rgba::new(0, 0, 0, 255), |d| {
            let f = ind.style.fill(d);
            if second {
                f.color2.unwrap_or(f.color)
            } else {
                f.color
            }
        }),
    }
}

fn rgba_of(c: Hsla) -> Rgba {
    let r = gpui_kit::Rgba::from(c);
    let b = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba::new(b(r.r), b(r.g), b(r.b), b(r.a))
}

fn hsla_of(c: Rgba) -> Hsla {
    tt_ui::hsla(c.to_color32())
}

fn dash(s: LineStyle) -> LineDash {
    match s {
        LineStyle::Solid => LineDash::Solid,
        LineStyle::Dashed => LineDash::Dashed,
        LineStyle::Dotted => LineDash::Dotted,
    }
}

fn line_style(d: LineDash) -> LineStyle {
    match d {
        LineDash::Solid => LineStyle::Solid,
        LineDash::Dashed => LineStyle::Dashed,
        LineDash::Dotted => LineStyle::Dotted,
    }
}

/// Every number slot of a study kind (in a fixed order).
fn number_slots(study: &Study) -> Vec<NumberSlot> {
    let meta = study.meta();
    let mut numbers = Vec::new();
    for d in meta.inputs {
        if let InputKind::Int { .. } | InputKind::Float { .. } = d.kind {
            numbers.push(NumberSlot::Input(d.id))
        }
    }
    for d in meta.hlines {
        numbers.push(NumberSlot::HLine(d.id));
    }
    for u in IntervalVisibility::UNITS {
        numbers.push(NumberSlot::VisFrom(u));
        numbers.push(NumberSlot::VisTo(u));
    }
    numbers
}

/// Plot types in TradingView's order and wording, with their menu icons.
const PLOT_TYPES: [(PlotType, &str, Lucide); 11] = [
    (PlotType::Line, "Line", Lucide::Spline),
    (PlotType::LineBr, "Line with breaks", Lucide::SplinePointer),
    (PlotType::Step, "Step line", Lucide::ChartNoAxesGantt),
    (
        PlotType::StepBr,
        "Step line with breaks",
        Lucide::ChartGantt,
    ),
    (
        PlotType::StepDiamond,
        "Step line with diamonds",
        Lucide::Diamond,
    ),
    (PlotType::Histogram, "Histogram", Lucide::ChartNoAxesColumn),
    (PlotType::Cross, "Cross", Lucide::Plus),
    (PlotType::Area, "Area", Lucide::ChartArea),
    (PlotType::AreaBr, "Area with breaks", Lucide::AreaChart),
    (PlotType::Columns, "Columns", Lucide::ChartColumn),
    (PlotType::Circles, "Circles", Lucide::CircleDot),
];

fn eid(prefix: &str, id: &str) -> ElementId {
    ElementId::Name(SharedString::from(format!("{prefix}-{id}")))
}

/// Width of `s` in the dialog's 14 px text.
fn text_w(window: &Window, s: &str) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    let run = window.text_style().to_run(s.len());
    let line = window.text_system().shape_line(
        SharedString::from(s.to_owned()),
        px(TEXT_BODY),
        &[run],
        None,
    );
    f32::from(line.width).ceil()
}

/// `el` greyed out and inert when `enabled` is false (`enabled_if`, an
/// unchecked plot's controls).
fn dim(el: impl IntoElement, enabled: bool) -> AnyElement {
    if enabled {
        return el.into_any_element();
    }
    div()
        .relative()
        .opacity(0.45)
        .child(el)
        .child(div().absolute().inset_0().occlude())
        .into_any_element()
}

/// A row: `h` tall, label cell `lw` wide (`None`: grows), then controls.
fn row(h: f32, lw: Option<f32>, label: impl IntoElement, controls: AnyElement) -> Div {
    let cell = div().flex_none().whitespace_nowrap().child(label);
    h_flex()
        .h(px(h))
        .flex_none()
        .items_center()
        .child(match lw {
            Some(w) => cell.w(px(w)),
            None => cell.flex_1().min_w_0(),
        })
        .child(controls)
}

/// "OUTPUT VALUES": 11 px uppercase muted caption.
fn caption(title: &str, first: bool, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .when(!first, |d| d.pt(px(PAD_CAPTION)))
        .child(
            div()
                .h(px(H_CAPTION))
                .flex()
                .items_center()
                .text_size(px(TEXT_CAPTION))
                .text_color(tt_ui::hsla(cx.ui().text_muted))
                .child(title.to_uppercase()),
        )
        .into_any_element()
}

fn set_color(ind: &mut Indicator, slot: ColorSlot, c: Rgba) {
    match slot {
        ColorSlot::Input(id) => {
            ind.study.set(id, InputValue::Color(c.to_core()));
        }
        ColorSlot::Plot(id, i) => ind.style.set_plot_color(id, i, c),
        ColorSlot::HLine(id) => ind.style.hline_mut(id).color = Some(c),
        ColorSlot::Fill(id, false) => ind.style.fill_mut(id).color = Some(c),
        ColorSlot::Fill(id, true) => ind.style.fill_mut(id).color2 = Some(c),
    }
}

// ---- the panel ----------------------------------------------------------------

pub struct IndicatorSettingsPanel {
    app: WeakEntity<TradeTerm>,
    chart: TabChart,
    id: u64,
    kind: StudyKind,
    original: Original,
    tab: SettingsTab,
    numbers: Vec<(NumberSlot, Entity<InputState>)>,
    /// The dialog's width, set per tab (TradingView's grows on Visibility).
    pub width: Rc<Cell<f32>>,
    committed: bool,
    _subs: Vec<Subscription>,
}

impl IndicatorSettingsPanel {
    /// `None` if the indicator no longer exists.
    pub fn new(
        app: WeakEntity<TradeTerm>,
        chart: TabChart,
        id: u64,
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
        let mut subs = Vec::new();
        let mut numbers = Vec::new();
        for slot in number_slots(&ind.study) {
            let text = slot_text(&ind, slot);
            let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
            subs.push(cx.subscribe_in(
                &input,
                window,
                move |this, input, ev: &InputEvent, w, cx| {
                    if let InputEvent::Change = ev {
                        let text = input.read(cx).value().to_string();
                        this.number_changed(slot, &text, w, cx);
                    }
                },
            ));
            numbers.push((slot, input));
        }
        // The dialog dropped this panel: closing any way but Ok (×, Esc,
        // Cancel, clicking outside) restores the indicator.
        subs.push(cx.on_release(|this, cx| {
            if !this.committed {
                this.restore(cx);
            }
        }));
        Some(Self {
            app,
            chart,
            id,
            kind: ind.study.kind(),
            original: Original::of(&ind),
            tab: SettingsTab::Inputs,
            numbers,
            width: Rc::new(Cell::new(C_W_DIALOG)),
            committed: false,
            _subs: subs,
        })
    }

    /// The dialog title: the study's short name, as TradingView ("RSI",
    /// "BB", "Vol").
    pub fn title(&self) -> String {
        self.kind.meta().short.to_owned()
    }

    /// Runs `f` on the indicator (if it still exists).
    fn with_indicator<R>(&self, cx: &App, f: impl FnOnce(&Indicator) -> R) -> Option<R> {
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
            .map(f)
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

    /// Ok: keep the changes (and save them).
    pub fn commit(&mut self, cx: &mut App) {
        self.committed = true;
        self.app.update(cx, |app, cx| app.persist(cx)).ok();
    }

    /// Restores the indicator as it was before the dialog opened.
    fn restore(&self, cx: &mut App) {
        let (id, o) = (self.id, self.original.clone());
        self.chart.update(cx, |chart, cx| {
            chart.update_state(cx, |s| {
                if let Some(ind) = s.source.tab.chart.indicator_mut(id) {
                    o.restore(ind);
                }
            })
        });
    }

    /// Writes every number field from the indicator (after a bulk change).
    /// `set_value` emits no change events, so nothing is applied back.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(texts) = self.with_indicator(cx, |ind| {
            self.numbers
                .iter()
                .map(|(s, _)| slot_text(ind, *s))
                .collect::<Vec<_>>()
        }) else {
            return;
        };
        for ((_, input), text) in self.numbers.iter().zip(texts) {
            input.update(cx, |s, cx| s.set_value(text, window, cx));
        }
        cx.notify();
    }

    /// Applies `preset` (factory, the user default or a named preset) and
    /// refreshes every control.
    fn apply(&mut self, preset: &IndicatorPreset, window: &mut Window, cx: &mut Context<Self>) {
        let p = preset.clone();
        self.edit(cx, move |ind| reset_to(ind, &p));
        self.sync(window, cx);
    }

    fn reset_factory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply(&IndicatorPreset::factory(self.kind), window, cx);
    }

    fn apply_default(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let kind = self.kind;
        let p = self
            .app
            .upgrade()
            .and_then(|a| a.read(cx).indicator_defaults.get(&kind).cloned());
        if let Some(p) = p {
            self.apply(&p, window, cx);
        }
    }

    /// The current inputs and style.
    fn preset(&self, cx: &App) -> Option<IndicatorPreset> {
        self.with_indicator(cx, IndicatorPreset::of)
    }

    fn save_default(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.preset(cx) else { return };
        self.app
            .update(cx, |a, cx| a.save_indicator_default(p, cx))
            .ok();
        let short = self.kind.meta().short;
        window.push_notification(
            format!("Saved as the default for new {short} indicators"),
            cx,
        );
    }

    fn remove_default(&mut self, cx: &mut Context<Self>) {
        let kind = self.kind;
        self.app
            .update(cx, |a, cx| a.remove_indicator_default(kind, cx))
            .ok();
    }

    fn apply_preset(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let kind = self.kind;
        let p = self.app.upgrade().and_then(|a| {
            a.read(cx)
                .indicator_presets
                .get(&kind)?
                .iter()
                .find(|p| p.name == name)
                .map(|p| p.preset.clone())
        });
        if let Some(p) = p {
            self.apply(&p, window, cx);
        }
    }

    fn number_changed(
        &mut self,
        slot: NumberSlot,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match slot {
            NumberSlot::Input(id) => self.edit(cx, |ind| {
                apply_number_input(&mut ind.study, id, text);
            }),
            NumberSlot::HLine(id) => {
                if let Ok(v) = text.trim().parse::<f64>()
                    && v.is_finite()
                {
                    self.edit(cx, |ind| ind.style.hline_mut(id).value = Some(v));
                }
            }
            NumberSlot::VisFrom(u) | NumberSlot::VisTo(u) => {
                let Ok(v) = text.trim().parse::<u16>() else {
                    return;
                };
                let Some(cur) = self.with_indicator(cx, |i| i.style.visibility.get(u)) else {
                    return;
                };
                // An unchecked unit keeps its fields editable for later.
                let Some((a, b)) = cur else {
                    cx.notify();
                    return;
                };
                let (lo, hi) = IntervalVisibility::full_range(u);
                let v = v.clamp(lo, hi);
                let (a, b) = if matches!(slot, NumberSlot::VisFrom(_)) {
                    (v, b.max(v))
                } else {
                    (a.min(v), v)
                };
                self.edit(cx, |ind| ind.style.visibility.set(u, Some((a, b))));
            }
        }
    }

    /// The range in a unit's from / to fields (to re-check a unit with what
    /// it showed).
    fn vis_fields(&self, u: TimeUnit, cx: &App) -> (u16, u16) {
        let read = |want: NumberSlot| {
            self.numbers
                .iter()
                .find(|(s, _)| *s == want)
                .and_then(|(_, i)| i.read(cx).value().trim().parse::<u16>().ok())
        };
        let (lo, hi) = IntervalVisibility::full_range(u);
        let (a, b) = (
            read(NumberSlot::VisFrom(u)).unwrap_or(lo).clamp(lo, hi),
            read(NumberSlot::VisTo(u)).unwrap_or(hi).clamp(lo, hi),
        );
        (a.min(b), a.max(b))
    }

    fn number(&self, slot: NumberSlot) -> Option<&Entity<InputState>> {
        self.numbers
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, e)| e)
    }

    /// The "Defaults ▾" menu, rebuilt each time it opens (TradingView's
    /// "Reset settings" and "Save as default" first).
    pub fn defaults_menu(this: &WeakEntity<Self>, menu: Menu, cx: &mut App) -> Menu {
        let Some(panel) = this.upgrade() else {
            return menu;
        };
        let (kind, app) = {
            let p = panel.read(cx);
            (p.kind, p.app.clone())
        };
        let (has_default, presets): (bool, Vec<NamedPreset>) = app
            .upgrade()
            .map(|a| {
                let a = a.read(cx);
                (
                    a.indicator_defaults.contains_key(&kind),
                    a.indicator_presets.get(&kind).cloned().unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        let on = |f: fn(&mut Self, &mut Window, &mut Context<Self>)| {
            let this = this.clone();
            move |w: &mut Window, cx: &mut App| {
                this.update(cx, |p, cx| f(p, w, cx)).ok();
            }
        };
        let mut menu = menu
            .row(
                MenuRow::new("ind-reset", "Reset settings")
                    .description("Factory settings of this indicator")
                    .on_select(on(Self::reset_factory)),
            )
            .row(
                MenuRow::new("ind-save-default", "Save as default")
                    .description("New indicators of this kind start with these settings")
                    .on_select(on(Self::save_default)),
            );
        if has_default {
            menu = menu
                .row(
                    MenuRow::new("ind-apply-default", "Apply default")
                        .description("Your saved default for this indicator")
                        .on_select(on(Self::apply_default)),
                )
                .row(
                    MenuRow::new("ind-remove-default", "Remove saved default")
                        .on_select(on(|p, _, cx| p.remove_default(cx))),
                );
        }
        menu = menu.separator().row(
            MenuRow::new("ind-save-preset", "Save preset as…")
                .on_select(on(|p, w, cx| p.open_save_preset(w, cx))),
        );
        if !presets.is_empty() {
            menu = menu.section("Presets");
            for (i, preset) in presets.into_iter().enumerate() {
                let name = preset.name.clone();
                let (apply, delete) = (this.clone(), app.clone());
                let apply_name = name.clone();
                menu = menu.row(
                    MenuRow::new(("ind-preset", i), name.clone())
                        .on_select(move |w, cx| {
                            apply
                                .update(cx, |p, cx| p.apply_preset(&apply_name, w, cx))
                                .ok();
                        })
                        .removable("Delete preset", move |_, cx| {
                            delete
                                .update(cx, |a, cx| a.delete_indicator_preset(kind, &name, cx))
                                .ok();
                        }),
                );
            }
        }
        menu
    }

    /// "Save preset as…": a small dialog for the name.
    fn open_save_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let kind = self.kind;
        let existing: Vec<String> = self
            .app
            .upgrade()
            .and_then(|a| {
                a.read(cx)
                    .indicator_presets
                    .get(&kind)
                    .map(|l| l.iter().map(|p| p.name.clone()).collect())
            })
            .unwrap_or_default();
        let panel = cx.new(|cx| SavePresetPanel::new(existing, window, cx));
        let (focus, read) = (panel.clone(), panel.clone());
        let this = cx.entity().downgrade();
        let footer = DialogFooter::new().cancel().ok("Save").on_ok(move |_, cx| {
            let name = read.read(cx).input.read(cx).value().to_string();
            if tt_session::settings::preset_name(&name).is_none() {
                return false;
            }
            this.update(cx, |p, cx| {
                if let Some(preset) = p.preset(cx) {
                    p.app
                        .update(cx, |a, cx| a.save_indicator_preset(&name, preset, cx))
                        .ok();
                }
            })
            .ok();
            true
        });
        let opts = DialogOpts::new("Save preset", DialogSize::Sm)
            .footer(footer)
            .on_open(move |window, cx| {
                let handle = focus.read(cx).input.read(cx).focus_handle(cx);
                handle.focus(window, cx);
            });
        tt_ui::open_dialog(window, cx, opts, panel);
    }

    // ---- rendering ------------------------------------------------------------

    /// A callback that edits the indicator.
    fn setter<V: 'static>(
        &self,
        cx: &Context<Self>,
        f: impl Fn(&mut Indicator, V) + 'static,
    ) -> impl Fn(V, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |v, _, cx| {
            this.update(cx, |p, cx| p.edit(cx, |ind| f(ind, v))).ok();
        }
    }

    /// A color swatch editing `slot`.
    fn swatch(&self, slot: ColorSlot, now: Rgba, cx: &Context<Self>) -> ColorSwatchButton {
        let set = self.setter(cx, move |ind, c: Hsla| set_color(ind, slot, rgba_of(c)));
        ColorSwatchButton::new(
            ElementId::Name(SharedString::from(format!("swatch-{slot:?}"))),
            hsla_of(now),
        )
        .on_change(set)
    }

    /// One input's control and its width.
    fn input_control(
        &self,
        def: &'static InputDef,
        study: &Study,
        cx: &Context<Self>,
    ) -> (AnyElement, f32) {
        let id = def.id;
        match (def.kind, study.get(id)) {
            (InputKind::Int { .. } | InputKind::Float { .. }, _) => {
                match self.number(NumberSlot::Input(id)) {
                    Some(s) => (tt_ui::Field::new(s).into_any_element(), C_W_FIELD),
                    None => (div().into_any_element(), 0.0),
                }
            }
            (InputKind::Bool, Some(InputValue::Bool(b))) => (
                CheckBox::new(eid("in", id), "", b)
                    .on_change(self.setter(cx, move |ind, v: bool| {
                        ind.study.set(id, InputValue::Bool(v));
                    }))
                    .into_any_element(),
                C_CHECKBOX,
            ),
            (InputKind::Choice(opts), Some(InputValue::Choice(cur))) => (
                Select::new(
                    eid("in", id),
                    opts.iter().enumerate().map(|(i, l)| (i as u16, *l)),
                    cur,
                )
                .comfortable()
                .width(C_W_FIELD)
                .on_change(self.setter(cx, move |ind, v: u16| {
                    ind.study.set(id, InputValue::Choice(v));
                }))
                .into_any_element(),
                C_W_FIELD,
            ),
            (InputKind::Source, Some(InputValue::Source(cur))) => (
                Select::new(
                    eid("in", id),
                    Source::ALL.iter().map(|s| (*s, source_label(*s))),
                    cur,
                )
                .comfortable()
                .width(C_W_FIELD)
                .on_change(self.setter(cx, move |ind, v: Source| {
                    ind.study.set(id, InputValue::Source(v));
                }))
                .into_any_element(),
                C_W_FIELD,
            ),
            (InputKind::Timeframe, Some(InputValue::Timeframe(cur))) => {
                let options = std::iter::once((None, "Chart".to_owned()))
                    .chain(Timeframe::ALL.iter().map(|t| (Some(*t), t.label())));
                (
                    Select::new(eid("in", id), options, cur)
                        .comfortable()
                        .width(C_W_FIELD)
                        .on_change(self.setter(cx, move |ind, v: Option<Timeframe>| {
                            ind.study.set(id, InputValue::Timeframe(v));
                        }))
                        .into_any_element(),
                    C_W_FIELD,
                )
            }
            (InputKind::Color, _) => {
                let theme = cx.ui().clone();
                let now = self
                    .with_indicator(cx, |i| slot_color(i, ColorSlot::Input(id), &theme))
                    .unwrap_or(Rgba::new(0, 0, 0, 255));
                (
                    self.swatch(ColorSlot::Input(id), now, cx)
                        .into_any_element(),
                    C_CONTROL,
                )
            }
            _ => (div().into_any_element(), 0.0),
        }
    }

    /// The Inputs tab and the dialog width it needs: labels on the left,
    /// controls in one right-hand column (TradingView's table), an ⓘ slot
    /// after them when any input has a tooltip.
    fn inputs_tab(&self, study: &Study, window: &Window, cx: &Context<Self>) -> (AnyElement, f32) {
        let meta = study.meta();
        let shown: Vec<&'static InputDef> = meta
            .inputs
            .iter()
            .filter(|d| {
                !meta
                    .plots
                    .iter()
                    .any(|p| plot_toggle(meta, p).is_some_and(|t| t.id == d.id))
            })
            .collect();
        let any_tip = shown.iter().any(|d| d.tooltip.is_some());
        let check_w = C_CHECKBOX + C_GAP;
        let mut lines: Vec<InputLine> = Vec::new();
        let (mut label_w, mut col): (f32, f32) = (0.0, 0.0);
        let mut i = 0;
        while i < shown.len() {
            let mut j = i + 1;
            if let Some(k) = shown[i].inline {
                while j < shown.len()
                    && shown[j].inline == Some(k)
                    && shown[j].group == shown[i].group
                {
                    j += 1;
                }
            }
            let run = &shown[i..j];
            i = j;
            let first = run[0];
            let id = first.id;
            let enabled = first.enabled_if.is_none_or(|f| f(study));
            let tip = run.iter().find_map(|d| d.tooltip.map(|t| (d.id, t)));
            // A leading Bool is the row's checkbox + label.
            let (label, rest): (AnyElement, &[&'static InputDef]) =
                match (first.kind, study.get(id)) {
                    (InputKind::Bool, Some(InputValue::Bool(b))) => {
                        label_w = label_w.max(text_w(window, first.title) + check_w);
                        (
                            CheckBox::new(eid("in", id), first.title, b)
                                .on_change(self.setter(cx, move |ind, v: bool| {
                                    ind.study.set(id, InputValue::Bool(v));
                                }))
                                .into_any_element(),
                            &run[1..],
                        )
                    }
                    _ => {
                        label_w = label_w.max(text_w(window, first.title));
                        (first.title.into_any_element(), run)
                    }
                };
            let controls = (!rest.is_empty()).then(|| {
                let mut w = 0.0;
                let els: Vec<AnyElement> = rest
                    .iter()
                    .enumerate()
                    .map(|(k, d)| {
                        let (c, cw) = self.input_control(d, study, cx);
                        w += cw + if k > 0 { C_GAP } else { 0.0 };
                        c
                    })
                    .collect();
                col = col.max(w);
                els
            });
            lines.push(InputLine {
                group: first.group,
                enabled,
                controls,
                label,
                tip,
            });
        }
        let col = col.max(C_W_FIELD);
        let info_w = if any_tip { W_INFO } else { 0.0 };
        let mut body = v_flex();
        let mut group = None;
        let mut first = true;
        for l in lines {
            if l.group != group {
                group = l.group;
                if let Some(g) = group {
                    body = body.child(caption(g, first, cx));
                }
            }
            first = false;
            let r = match l.controls {
                None => h_flex()
                    .h(px(H_CHECK_ROW))
                    .flex_none()
                    .items_center()
                    .child(l.label)
                    .when_some(l.tip, |d, (id, t)| d.child(self.info(id, t, cx))),
                Some(els) => row(
                    C_ROW,
                    None,
                    l.label,
                    h_flex()
                        .flex_none()
                        .items_center()
                        .child(
                            h_flex()
                                .flex_none()
                                .w(px(col))
                                .gap(px(C_GAP))
                                .items_center()
                                .children(els),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(px(info_w))
                                .when_some(l.tip, |d, (id, t)| d.child(self.info(id, t, cx))),
                        )
                        .into_any_element(),
                ),
            };
            body = body.child(dim(r, l.enabled));
        }
        if first {
            body = body.child(
                div()
                    .h(px(H_CHECK_ROW))
                    .flex()
                    .items_center()
                    .text_color(tt_ui::hsla(cx.ui().text_muted))
                    .child("No inputs"),
            );
        }
        let width = (2.0 * C_PAD_X + label_w + C_LABEL_GAP + col + info_w)
            .max(C_W_DIALOG)
            .ceil();
        (body.into_any_element(), width)
    }

    /// The ⓘ after an input with a tooltip.
    fn info(&self, id: &'static str, t: &'static str, cx: &Context<Self>) -> AnyElement {
        let muted = tt_ui::hsla(cx.ui().text_muted);
        div()
            .id(eid("in-tip", id))
            .flex_none()
            .pl(px(C_GAP))
            .child(glyph(Glyph::Lucide(Lucide::Info), ICON_MD).color(muted))
            .tooltip(tip(t, None))
            .into_any_element()
    }

    /// One plot's color + line control editing palette entry `i` (and the
    /// plot's width and style).
    fn plot_line(
        &self,
        def: &'static PlotDef,
        i: usize,
        ind: &Indicator,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> LinePicker {
        let p = ind.style.plot(def);
        let id = def.id;
        let set = self.setter(cx, move |ind, lv: LineValue| {
            ind.style.set_plot_color(id, i, rgba_of(lv.color));
            let s = ind.style.plot_mut(id);
            s.width = Some(lv.width);
            s.line_style = Some(line_style(lv.dash));
        });
        LinePicker::new(
            eid(&format!("plot-line-{i}"), id),
            LineValue {
                color: hsla_of(slot_color(ind, ColorSlot::Plot(id, i), theme)),
                width: p.width,
                dash: dash(p.line_style),
            },
        )
        .on_change(set)
    }

    /// The plot type select with "Price line" on top of its menu.
    fn plot_type(&self, def: &'static PlotDef, ind: &Indicator, cx: &Context<Self>) -> IconSelect {
        let p = ind.style.plot(def);
        let id = def.id;
        let now = PLOT_TYPES
            .iter()
            .position(|(t, _, _)| *t == p.plot_type)
            .unwrap_or(0);
        IconSelect::new(
            eid("plot-type", id),
            PLOT_TYPES.iter().map(|(_, l, g)| (Glyph::Lucide(*g), *l)),
            now,
        )
        .switch(
            "Price line",
            p.price_line,
            self.setter(cx, move |ind, v| {
                ind.style.plot_mut(id).price_line = Some(v)
            }),
        )
        .on_change(self.setter(cx, move |ind, i: usize| {
            ind.style.plot_mut(id).plot_type = Some(PLOT_TYPES[i.min(PLOT_TYPES.len() - 1)].0)
        }))
    }

    /// The Style tab and the dialog width it needs.
    fn style_tab(&self, ind: &Indicator, window: &Window, cx: &Context<Self>) -> (AnyElement, f32) {
        let study = &ind.study;
        let style = &ind.style;
        let meta = study.meta();
        let theme = cx.ui().clone();
        let check_w = C_CHECKBOX + C_GAP;
        // One label column for the tab (TradingView's table).
        let mut lw = text_w(window, "Precision");
        let mut ctl: f32 = C_W_FIELD;
        for d in meta.plots {
            lw = lw.max(text_w(window, d.title) + check_w);
            for (name, _) in d.palette.iter().filter(|_| d.palette.len() > 1) {
                lw = lw.max(text_w(window, name) + INDENT);
            }
            ctl = ctl.max(tt_ui::picker::W_LINE_PICKER + C_GAP + C_CONTROL);
        }
        for d in meta.hlines {
            lw = lw.max(text_w(window, d.title) + check_w);
            ctl = ctl.max(tt_ui::picker::W_LINE_PICKER + C_GAP + C_W_FIELD);
        }
        for d in meta.fills {
            lw = lw.max(text_w(window, d.title) + check_w);
        }
        let lw = lw + C_LABEL_GAP;
        let mut out: Vec<AnyElement> = Vec::new();
        for def in meta.plots {
            let p = style.plot(def);
            let id = def.id;
            let toggle = plot_toggle(meta, def);
            let on = p.visible
                && toggle.is_none_or(|t| matches!(study.get(t.id), Some(InputValue::Bool(true))));
            let toggle_id = toggle.map(|t| t.id);
            let check = CheckBox::new(eid("plot-vis", id), def.title, on).on_change(self.setter(
                cx,
                move |ind, v: bool| {
                    if let Some(t) = toggle_id {
                        ind.study.set(t, InputValue::Bool(v));
                    }
                    ind.style.plot_mut(id).visible = Some(v);
                },
            ));
            let enabled = toggle.is_some() || def.enabled_if.is_none_or(|f| f(study));
            let n = def.palette.len().max(1);
            if n > 1 {
                // A checkbox row, then one indented row per color; the
                // first carries the plot type.
                let mut rows = vec![
                    h_flex()
                        .h(px(H_CHECK_ROW))
                        .items_center()
                        .child(check)
                        .into_any_element(),
                ];
                // TradingView lists Volume's "Growing" before "Falling".
                let order: Vec<usize> = if def.kind == PlotKind::VolumeColumns {
                    (0..n).rev().collect()
                } else {
                    (0..n).collect()
                };
                for (k, &i) in order.iter().enumerate() {
                    let name = def.palette.get(i).map_or("Color", |(n, _)| *n);
                    let mut c = h_flex()
                        .gap(px(C_GAP))
                        .items_center()
                        .child(self.plot_line(def, i, ind, &theme, cx));
                    if k == 0 {
                        c = c.child(self.plot_type(def, ind, cx));
                    }
                    rows.push(
                        row(
                            H_COLOR_ROW,
                            Some(lw),
                            div().pl(px(INDENT)).child(name),
                            dim(c, on),
                        )
                        .into_any_element(),
                    );
                }
                rows.push(div().h(px(GAP_GROUP)).into_any_element());
                out.push(dim(v_flex().children(rows), enabled));
            } else {
                let c = h_flex()
                    .gap(px(C_GAP))
                    .items_center()
                    .child(self.plot_line(def, 0, ind, &theme, cx))
                    .child(self.plot_type(def, ind, cx));
                out.push(dim(row(C_ROW, Some(lw), check, dim(c, on)), enabled));
            }
        }
        for def in meta.hlines {
            let h = style.hline(def);
            let id = def.id;
            let check = CheckBox::new(eid("hline-vis", id), def.title, h.visible).on_change(
                self.setter(cx, move |ind, v| ind.style.hline_mut(id).visible = Some(v)),
            );
            let set = self.setter(cx, move |ind, lv: LineValue| {
                set_color(ind, ColorSlot::HLine(id), rgba_of(lv.color));
                let s = ind.style.hline_mut(id);
                s.width = Some(lv.width);
                s.style = Some(line_style(lv.dash));
            });
            let mut c = h_flex().gap(px(C_GAP)).items_center().child(
                LinePicker::new(
                    eid("hline-line", id),
                    LineValue {
                        color: hsla_of(h.color),
                        width: h.width,
                        dash: dash(h.style),
                    },
                )
                .on_change(set),
            );
            if let Some(s) = self.number(NumberSlot::HLine(id)) {
                c = c.child(tt_ui::Field::new(s));
            }
            out.push(row(C_ROW, Some(lw), check, dim(c, h.visible)).into_any_element());
        }
        for def in meta.fills {
            let f = style.fill(def);
            let id = def.id;
            let check = CheckBox::new(eid("fill-vis", id), def.title, f.visible)
                .on_change(self.setter(cx, move |ind, v| ind.style.fill_mut(id).visible = Some(v)));
            let mut c = h_flex().gap(px(C_GAP)).items_center().child(self.swatch(
                ColorSlot::Fill(id, false),
                f.color,
                cx,
            ));
            if def.color2.is_some() {
                c = c.child(self.swatch(
                    ColorSlot::Fill(id, true),
                    f.color2.unwrap_or(f.color),
                    cx,
                ));
            }
            out.push(row(C_ROW, Some(lw), check, dim(c, f.visible)).into_any_element());
        }
        out.push(caption("Output values", out.is_empty(), cx));
        let precisions = std::iter::once((None, "Default".to_owned()))
            .chain((0..=MAX_PRECISION).map(|p| (Some(p), p.to_string())));
        out.push(
            row(
                C_ROW,
                Some(lw),
                "Precision",
                Select::new("precision", precisions, style.precision())
                    .comfortable()
                    .width(C_W_FIELD)
                    .on_change(self.setter(cx, |ind, p| ind.style.precision = p))
                    .into_any_element(),
            )
            .into_any_element(),
        );
        let check_row =
            |id: &'static str, label: &'static str, on: bool, f: fn(&mut IndicatorStyle, bool)| {
                h_flex()
                    .h(px(H_CHECK_ROW))
                    .flex_none()
                    .items_center()
                    .child(
                        CheckBox::new(id, label, on)
                            .on_change(self.setter(cx, move |ind, v| f(&mut ind.style, v))),
                    )
                    .into_any_element()
            };
        out.push(check_row(
            "labels-on-price-scale",
            "Labels on price scale",
            style.labels_on_price_scale,
            |s, v| s.labels_on_price_scale = v,
        ));
        out.push(check_row(
            "values-in-status-line",
            "Values in status line",
            style.values_in_status_line,
            |s, v| s.values_in_status_line = v,
        ));
        out.push(caption("Input values", false, cx));
        out.push(check_row(
            "inputs-in-status-line",
            "Inputs in status line",
            style.inputs_in_status_line,
            |s, v| s.inputs_in_status_line = v,
        ));
        let width = (2.0 * C_PAD_X + lw + ctl).max(C_W_DIALOG).ceil();
        (v_flex().children(out).into_any_element(), width)
    }

    /// Ticks, Seconds … Months with range sliders, Ranges.
    fn visibility_tab(&self, style: &IndicatorStyle, cx: &Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let v = style.visibility;
        let flag = |id: &'static str,
                    label: &'static str,
                    on: bool,
                    f: fn(&mut IntervalVisibility, bool)| {
            let t = this.clone();
            visibility_tv::check_row(id, label, on, move |on, _, cx| {
                t.update(cx, |p, cx| {
                    p.edit(cx, move |ind| f(&mut ind.style.visibility, on))
                })
                .ok();
            })
        };
        let mut body = v_flex().child(flag("ind-vis-ticks", "Ticks", v.ticks, |v, on| {
            v.ticks = on
        }));
        for u in IntervalVisibility::UNITS {
            let (Some(lo), Some(hi)) = (
                self.number(NumberSlot::VisFrom(u)),
                self.number(NumberSlot::VisTo(u)),
            ) else {
                continue;
            };
            let on = v.get(u).is_some();
            let (a, b) = match v.get(u) {
                Some(r) => r,
                None => self.vis_fields(u, cx),
            };
            let (min, max) = IntervalVisibility::full_range(u);
            let (t, t2) = (this.clone(), this.clone());
            body = body.child(visibility_tv::unit_row(
                "ind",
                u,
                on,
                lo,
                hi,
                (min as f32, max as f32),
                (a as f32, b as f32),
                move |on, _, cx| {
                    t.update(cx, |p, cx| {
                        let r = p.vis_fields(u, cx);
                        p.edit(cx, |ind| ind.style.visibility.set(u, on.then_some(r)));
                    })
                    .ok();
                },
                move |a, b, _, cx| {
                    t2.update(cx, |p, cx| {
                        let on = p
                            .with_indicator(cx, |i| i.style.visibility.get(u).is_some())
                            .unwrap_or(false);
                        if on {
                            p.edit(cx, |ind| ind.style.visibility.set(u, Some((a, b))));
                        } else {
                            cx.notify();
                        }
                    })
                    .ok();
                },
            ));
        }
        body.child(flag("ind-vis-ranges", "Ranges", v.ranges, |v, on| {
            v.ranges = on
        }))
        .into_any_element()
    }
}

/// One row of the Inputs tab before layout.
struct InputLine {
    group: Option<&'static str>,
    enabled: bool,
    /// `None`: a lone checkbox row.
    controls: Option<Vec<AnyElement>>,
    label: AnyElement,
    tip: Option<(&'static str, &'static str)>,
}

impl Render for IndicatorSettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(ind) = self.with_indicator(cx, |i| i.clone()) else {
            return DialogFrame::new(DialogSize::Sm)
                .id("indicator-settings")
                .comfortable()
                .body(EmptyState::new("This indicator was removed."));
        };
        let (body, width) = match self.tab {
            SettingsTab::Inputs => self.inputs_tab(&ind.study, window, cx),
            SettingsTab::Style => self.style_tab(&ind, window, cx),
            SettingsTab::Visibility => (self.visibility_tab(&ind.style, cx), W_VIS_DIALOG),
        };
        if (self.width.get() - width).abs() > 0.5 {
            self.width.set(width);
            window.refresh();
        }
        let this = cx.entity().downgrade();
        let nav = NavTabs::horizontal()
            .item(SettingsTab::Inputs.id(), "Inputs", None)
            .item(SettingsTab::Style.id(), "Style", None)
            .item(SettingsTab::Visibility.id(), "Visibility", None)
            .selected(self.tab.id())
            .on_change(move |id, _, cx| {
                let tab = match id {
                    "style" => SettingsTab::Style,
                    "visibility" => SettingsTab::Visibility,
                    _ => SettingsTab::Inputs,
                };
                this.update(cx, |p, cx| {
                    p.tab = tab;
                    cx.notify();
                })
                .ok();
            });
        DialogFrame::new(DialogSize::Sm)
            .id("indicator-settings")
            .comfortable()
            .min_body(2.0 * C_ROW)
            .nav(nav)
            .body(
                v_flex()
                    .text_size(px(TEXT_BODY))
                    .text_color(tt_ui::hsla(cx.ui().text))
                    .child(body),
            )
    }
}

impl TradeTerm {
    /// "Save as default": new indicators of the kind start from `p`.
    pub(crate) fn save_indicator_default(&mut self, p: IndicatorPreset, cx: &mut Context<Self>) {
        self.indicator_defaults.insert(p.kind(), p);
        self.persist(cx);
    }

    pub(crate) fn remove_indicator_default(&mut self, kind: StudyKind, cx: &mut Context<Self>) {
        if self.indicator_defaults.remove(&kind).is_some() {
            self.persist(cx);
        }
    }

    pub(crate) fn save_indicator_preset(
        &mut self,
        name: &str,
        p: IndicatorPreset,
        cx: &mut Context<Self>,
    ) {
        if tt_session::settings::save_preset(&mut self.indicator_presets, name, p) {
            self.persist(cx);
        }
    }

    pub(crate) fn delete_indicator_preset(
        &mut self,
        kind: StudyKind,
        name: &str,
        cx: &mut Context<Self>,
    ) {
        if tt_session::settings::delete_preset(&mut self.indicator_presets, kind, name) {
            self.persist(cx);
        }
    }
}

// ---- "Save preset as…" --------------------------------------------------------

struct SavePresetPanel {
    input: Entity<InputState>,
    existing: Vec<String>,
    _sub: Subscription,
}

impl SavePresetPanel {
    fn new(existing: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Preset name"));
        let _sub = cx.subscribe(&input, |_, _, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                cx.notify();
            }
        });
        Self {
            input,
            existing,
            _sub,
        }
    }
}

impl Render for SavePresetPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self.input.read(cx).value().trim().to_lowercase();
        let replaces = !name.is_empty() && self.existing.iter().any(|e| e.to_lowercase() == name);
        let muted = tt_ui::hsla(cx.ui().text_muted);
        v_flex()
            .gap(px(S1))
            .child(
                FormRow::new("Name")
                    .label_width(SIZE.label_width() * 0.6)
                    .control(div().flex_1().child(Input::new(&self.input))),
            )
            .child(
                div()
                    .min_h(px(H_SECTION))
                    .text_size(px(TEXT_SMALL))
                    .text_color(muted)
                    .child(if replaces {
                        "A preset with this name exists and will be replaced."
                    } else {
                        "Presets keep this indicator's inputs and style."
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every color a dialog shows for a study kind.
    fn color_slots(study: &Study) -> Vec<ColorSlot> {
        let meta = study.meta();
        let mut colors = Vec::new();
        for d in meta.inputs {
            if let InputKind::Color = d.kind {
                colors.push(ColorSlot::Input(d.id));
            }
        }
        for d in meta.plots {
            for i in 0..d.palette.len().max(1) {
                colors.push(ColorSlot::Plot(d.id, i));
            }
        }
        for d in meta.hlines {
            colors.push(ColorSlot::HLine(d.id));
        }
        for d in meta.fills {
            colors.push(ColorSlot::Fill(d.id, false));
            if d.color2.is_some() {
                colors.push(ColorSlot::Fill(d.id, true));
            }
        }
        colors
    }

    #[::core::prelude::v1::test]
    fn toggle_inputs_and_source_labels() {
        let vol = StudyKind::Volume.meta();
        let ma = vol.plots.iter().find(|p| p.id == "vol_ma").unwrap();
        assert_eq!(plot_toggle(vol, ma).map(|d| d.id), Some("show_ma"));
        assert!(plot_toggle(vol, &vol.plots[0]).is_none());
        assert_eq!(source_label(Source::Close), "Close");
        assert_eq!(source_label(Source::Hlc3), "HLC3");
    }
    use tt_chart::indicator_style::IntervalVisibility;

    #[::core::prelude::v1::test]
    fn number_fields_set_inputs_through_study_set_with_clamping() {
        let mut rsi = StudyKind::Rsi.default_study();
        assert!(apply_number_input(&mut rsi, "length", " 21 "));
        assert_eq!(rsi.get("length"), Some(InputValue::Int(21)));
        assert!(apply_number_input(&mut rsi, "length", "99999"));
        assert_eq!(rsi.get("length"), Some(InputValue::Int(2000)), "clamped");
        assert!(apply_number_input(&mut rsi, "length", "0"));
        assert_eq!(rsi.get("length"), Some(InputValue::Int(1)), "clamped");
        // Not a number yet (typing): nothing changes.
        assert!(!apply_number_input(&mut rsi, "length", ""));
        assert!(!apply_number_input(&mut rsi, "length", "-"));
        assert!(!apply_number_input(&mut rsi, "nope", "5"));
        assert_eq!(rsi.get("length"), Some(InputValue::Int(1)));

        let mut bb = StudyKind::BollingerBands.default_study();
        let mult = bb
            .meta()
            .inputs
            .iter()
            .find(|d| matches!(d.kind, InputKind::Float { .. }));
        let mult = mult.expect("BB has a float input").id;
        assert!(apply_number_input(&mut bb, mult, "2.5"));
        assert_eq!(bb.get(mult), Some(InputValue::Float(2.5)));
        // Every int / float input of every kind round-trips its own text.
        for kind in StudyKind::ALL {
            let mut s = kind.default_study();
            for def in s.meta().inputs {
                if matches!(def.kind, InputKind::Int { .. } | InputKind::Float { .. }) {
                    let ind = Indicator::new(s.clone());
                    let text = slot_text(&ind, NumberSlot::Input(def.id));
                    assert!(
                        apply_number_input(&mut s, def.id, &text),
                        "{kind:?} {}",
                        def.id
                    );
                    assert_eq!(s.get(def.id), Some(def.default), "{kind:?} {}", def.id);
                }
            }
        }
    }

    #[::core::prelude::v1::test]
    fn reset_restores_every_field_and_every_control() {
        let theme = Theme::dark();
        for kind in StudyKind::ALL {
            let factory = Indicator::new(kind.default_study());
            let mut ind = factory.clone();
            let meta = kind.meta();
            // Change everything a dialog can change.
            for def in meta.inputs {
                if let InputKind::Int { max, .. } = def.kind {
                    ind.study.set(def.id, InputValue::Int(max));
                }
                if let InputKind::Bool = def.kind
                    && let InputValue::Bool(b) = def.default
                {
                    ind.study.set(def.id, InputValue::Bool(!b));
                }
            }
            for def in meta.plots {
                let s = ind.style.plot_mut(def.id);
                s.visible = Some(!def.default.visible);
                s.width = Some(3);
                s.plot_type = Some(PlotType::Circles);
                s.line_style = Some(LineStyle::Dotted);
                s.price_line = Some(true);
                for i in 0..def.palette.len().max(1) {
                    ind.style.set_plot_color(def.id, i, Rgba::new(1, 2, 3, 4));
                }
            }
            for def in meta.hlines {
                let h = ind.style.hline_mut(def.id);
                h.visible = Some(false);
                h.value = Some(def.value + 7.0);
                h.color = Some(Rgba::new(9, 9, 9, 9));
            }
            for def in meta.fills {
                ind.style.fill_mut(def.id).visible = Some(!def.visible);
                ind.style.fill_mut(def.id).color = Some(Rgba::new(5, 5, 5, 5));
            }
            ind.style.precision = Some(5);
            ind.style.labels_on_price_scale = false;
            ind.style.values_in_status_line = false;
            ind.style.inputs_in_status_line = false;
            ind.style.visibility.set(TimeUnit::Minute, None);
            ind.style.visibility.set(TimeUnit::Day, Some((2, 3)));
            ind.visible = false;

            let (numbers, colors) = (number_slots(&ind.study), color_slots(&ind.study));
            let texts =
                |i: &Indicator| numbers.iter().map(|s| slot_text(i, *s)).collect::<Vec<_>>();
            let swatches = |i: &Indicator| {
                colors
                    .iter()
                    .map(|s| slot_color(i, *s, &theme))
                    .collect::<Vec<_>>()
            };
            assert_ne!(swatches(&ind), swatches(&factory), "{kind:?}");

            reset_to(&mut ind, &IndicatorPreset::factory(kind));
            assert_eq!(ind.study, factory.study, "{kind:?}");
            assert_eq!(ind.style, factory.style, "{kind:?}");
            assert!(ind.visible);
            assert!(ind.style.visibility.is_all());
            // What the controls show after `sync` equals a fresh dialog's.
            assert_eq!(texts(&ind), texts(&factory), "{kind:?}");
            assert_eq!(swatches(&ind), swatches(&factory), "{kind:?}");
        }
    }

    #[::core::prelude::v1::test]
    fn cancel_restores_exactly() {
        let mut ind = Indicator::new(StudyKind::Macd.default_study());
        ind.style.set_plot_color("macd", 0, Rgba::new(1, 1, 1, 255));
        let original = Original::of(&ind);
        reset_to(&mut ind, &IndicatorPreset::factory(StudyKind::Macd));
        ind.visible = false;
        ind.style.precision = Some(1);
        original.restore(&mut ind);
        assert_eq!(Original::of(&ind), original);
    }

    #[::core::prelude::v1::test]
    fn volume_swatches_show_the_theme_colors_until_overridden() {
        let theme = Theme::dark();
        let mut vol = Indicator::new(StudyKind::Volume.default_study());
        assert_eq!(
            slot_color(&vol, ColorSlot::Plot("vol", 1), &theme),
            Rgba::from_color32(theme.volume_up)
        );
        vol.style.set_plot_color("vol", 1, Rgba::new(1, 2, 3, 255));
        assert_eq!(
            slot_color(&vol, ColorSlot::Plot("vol", 1), &theme),
            Rgba::new(1, 2, 3, 255)
        );
        let _ = IntervalVisibility::default();
    }

    #[::core::prelude::v1::test]
    fn numbers_trim() {
        assert_eq!(trim_number(14.0), "14");
        assert_eq!(trim_number(2.5), "2.5");
        assert_eq!(trim_number(0.1 + 0.2), "0.3");
    }
}
