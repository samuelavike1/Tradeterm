//! One indicator's settings (TradingView's indicator settings dialog),
//! generated from the study metadata ([`tt_core::StudyMeta`]) and the
//! per-plot style ([`IndicatorStyle`]):
//!
//! - **Inputs**: every `InputDef` in order, in groups with inline rows,
//!   tooltips and `enabled_if`; then "Inputs in status line".
//! - **Style**: per plot visibility, color(s) with opacity, width and line
//!   style, plot type and price line; per level visibility, color, width,
//!   style and value; per fill visibility and color(s); precision, labels on
//!   price scale, values in status line.
//! - **Visibility**: per interval unit a checkbox and a from–to range.
//!
//! Every change applies live. Cancel (×, Esc, clicking outside) restores the
//! indicator exactly as it was; Ok keeps it. The footer's "Defaults ▾" menu
//! resets to factory settings, saves / applies / removes the user default of
//! the kind (new instances start from it) and manages named presets.
//!
//! Widget states (number fields, color pickers, sliders) are created once
//! per dialog; after a bulk change (reset, default, preset) [`Self::sync`]
//! writes every one of them from the indicator again, so no control can
//! show a stale value.

use crate::app::{TabChart, TradeTerm};
use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState, SliderValue};
use gpui_kit::component::{Sizable as _, Size, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use lucide_icons::Icon as Lucide;
use std::rc::Rc;
use tt_chart::indicator_style::{IntervalVisibility, MAX_PRECISION, MAX_WIDTH};
use tt_chart::{Indicator, IndicatorStyle, Rgba};
use tt_core::{
    InputDef, InputKind, InputValue, LineStyle, PlotKind, PlotType, Source, Study, StudyKind,
    TimeUnit, Timeframe,
};
use tt_session::settings::{IndicatorPreset, NamedPreset};
use tt_theme::Theme;
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{DialogOpts, Menu, MenuNav, MenuWidth, open_dialog};

/// The settings dialog's size (§3.6: three tabs → Sm with top tabs).
pub const SIZE: DialogSize = DialogSize::Sm;

/// From / to fields of a visibility range (a 3-digit count and stepper).
const W_RANGE_FIELD: f32 = W_FIELD_NUMBER * 0.75;
/// Width / style field of a line (sample + caret).
const W_LINE_FIELD: f32 = CONTROL_LG + CONTROL_SM;
/// Plot type select.
const W_TYPE_FIELD: f32 = W_FIELD_NUMBER;

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

/// What a color picker edits.
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

/// Color a picker shows for `ind`. Volume columns that were never colored
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

/// Every number and color slot of a study kind (in a fixed order).
fn slots(study: &Study) -> (Vec<NumberSlot>, Vec<ColorSlot>) {
    let meta = study.meta();
    let mut numbers = Vec::new();
    let mut colors = Vec::new();
    for d in meta.inputs {
        match d.kind {
            InputKind::Int { .. } | InputKind::Float { .. } => {
                numbers.push(NumberSlot::Input(d.id))
            }
            InputKind::Color => colors.push(ColorSlot::Input(d.id)),
            _ => {}
        }
    }
    for d in meta.plots {
        for i in 0..d.palette.len().max(1) {
            colors.push(ColorSlot::Plot(d.id, i));
        }
    }
    for d in meta.hlines {
        numbers.push(NumberSlot::HLine(d.id));
        colors.push(ColorSlot::HLine(d.id));
    }
    for d in meta.fills {
        colors.push(ColorSlot::Fill(d.id, false));
        if d.color2.is_some() {
            colors.push(ColorSlot::Fill(d.id, true));
        }
    }
    for u in IntervalVisibility::UNITS {
        numbers.push(NumberSlot::VisFrom(u));
        numbers.push(NumberSlot::VisTo(u));
    }
    (numbers, colors)
}

fn unit_title(u: TimeUnit) -> &'static str {
    match u {
        TimeUnit::Second => "Seconds",
        TimeUnit::Minute => "Minutes",
        TimeUnit::Hour => "Hours",
        TimeUnit::Day => "Days",
        TimeUnit::Week => "Weeks",
        TimeUnit::Month => "Months",
    }
}

/// Plot types in TradingView's order and wording.
const PLOT_TYPES: [(PlotType, &str); 11] = [
    (PlotType::Line, "Line"),
    (PlotType::LineBr, "Line with breaks"),
    (PlotType::Step, "Step line"),
    (PlotType::StepBr, "Step line with breaks"),
    (PlotType::StepDiamond, "Step line with diamonds"),
    (PlotType::Histogram, "Histogram"),
    (PlotType::Cross, "Cross"),
    (PlotType::Area, "Area"),
    (PlotType::AreaBr, "Area with breaks"),
    (PlotType::Columns, "Columns"),
    (PlotType::Circles, "Circles"),
];

const LINE_STYLES: [(LineStyle, &str); 3] = [
    (LineStyle::Solid, "Solid"),
    (LineStyle::Dashed, "Dashed"),
    (LineStyle::Dotted, "Dotted"),
];

fn eid(prefix: &str, id: &str) -> ElementId {
    ElementId::Name(SharedString::from(format!("{prefix}-{id}")))
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
    colors: Vec<(ColorSlot, Entity<ColorPickerState>)>,
    sliders: Vec<(TimeUnit, Entity<SliderState>)>,
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
        let theme = cx.ui().clone();
        let (number_slots, color_slots) = slots(&ind.study);
        let mut subs = Vec::new();
        let mut numbers = Vec::new();
        for slot in number_slots {
            let (min, max, step) = match slot {
                NumberSlot::Input(id) => match ind.study.meta().input(id).map(|d| d.kind) {
                    Some(InputKind::Int { min, max, step }) => {
                        (min as f64, max as f64, step.max(1) as f64)
                    }
                    Some(InputKind::Float { min, max, step }) => (min, max, step),
                    _ => (f64::MIN, f64::MAX, 1.0),
                },
                NumberSlot::HLine(_) => (-1e12, 1e12, 1.0),
                NumberSlot::VisFrom(u) | NumberSlot::VisTo(u) => {
                    let (lo, hi) = IntervalVisibility::full_range(u);
                    (lo as f64, hi as f64, 1.0)
                }
            };
            let text = slot_text(&ind, slot);
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .step(step)
                    .min(min)
                    .max(max)
                    .default_value(text)
            });
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
        let mut colors = Vec::new();
        for slot in color_slots {
            let c = hsla_of(slot_color(&ind, slot, &theme));
            let picker = cx.new(|cx| ColorPickerState::new(window, cx).default_value(c));
            subs.push(
                cx.subscribe(&picker, move |this, _, ev: &ColorPickerEvent, cx| {
                    if let ColorPickerEvent::Change(Some(c)) = ev {
                        let c = rgba_of(*c);
                        this.edit(cx, |ind| set_color(ind, slot, c));
                    }
                }),
            );
            colors.push((slot, picker));
        }
        let mut sliders = Vec::new();
        for u in IntervalVisibility::UNITS {
            let (lo, hi) = IntervalVisibility::full_range(u);
            let (a, b) = vis_range(&ind.style.visibility, u);
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(lo as f32)
                    .max(hi as f32)
                    .step(1.0)
                    .default_value(SliderValue::Range(a as f32, b as f32))
            });
            subs.push(
                cx.subscribe_in(&slider, window, move |this, _, ev: &SliderEvent, w, cx| {
                    let (SliderEvent::Change(v) | SliderEvent::Release(v)) = ev;
                    if let SliderValue::Range(a, b) = *v {
                        this.slider_changed(u, a.round() as u16, b.round() as u16, w, cx);
                    }
                }),
            );
            sliders.push((u, slider));
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
            colors,
            sliders,
            committed: false,
            _subs: subs,
        })
    }

    /// The dialog title: the study's full name.
    pub fn title(&self) -> String {
        self.kind.meta().name.to_owned()
    }

    /// The indicator as it is now (`None` once removed).
    fn current(&self, cx: &App) -> Option<(Study, bool, IndicatorStyle)> {
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
            .map(|i| (i.study.clone(), i.visible, i.style.clone()))
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

    /// Writes every widget state from the indicator (after a bulk change).
    /// `set_value` emits no change events, so nothing is applied back.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let theme = cx.ui().clone();
        let Some((texts, colors, ranges)) = self.with_indicator(cx, |ind| {
            (
                self.numbers
                    .iter()
                    .map(|(s, _)| slot_text(ind, *s))
                    .collect::<Vec<_>>(),
                self.colors
                    .iter()
                    .map(|(s, _)| hsla_of(slot_color(ind, *s, &theme)))
                    .collect::<Vec<_>>(),
                self.sliders
                    .iter()
                    .map(|(u, _)| vis_range(&ind.style.visibility, *u))
                    .collect::<Vec<_>>(),
            )
        }) else {
            return;
        };
        for ((_, input), text) in self.numbers.iter().zip(texts) {
            input.update(cx, |s, cx| s.set_value(text, window, cx));
        }
        for ((_, picker), c) in self.colors.iter().zip(colors) {
            picker.update(cx, |s, cx| s.set_value(c, window, cx));
        }
        for ((_, slider), (a, b)) in self.sliders.iter().zip(ranges) {
            slider.update(cx, |s, cx| {
                s.set_value(SliderValue::Range(a as f32, b as f32), window, cx)
            });
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
        window: &mut Window,
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
                let Some((a, b)) = cur else { return };
                let (lo, hi) = IntervalVisibility::full_range(u);
                let v = v.clamp(lo, hi);
                let (a, b) = if matches!(slot, NumberSlot::VisFrom(_)) {
                    (v, b.max(v))
                } else {
                    (a.min(v), v)
                };
                self.edit(cx, |ind| ind.style.visibility.set(u, Some((a, b))));
                if let Some((_, s)) = self.sliders.iter().find(|(x, _)| *x == u) {
                    s.update(cx, |s, cx| {
                        s.set_value(SliderValue::Range(a as f32, b as f32), window, cx)
                    });
                }
            }
        }
    }

    fn slider_changed(
        &mut self,
        u: TimeUnit,
        a: u16,
        b: u16,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let on = self
            .with_indicator(cx, |i| i.style.visibility.get(u).is_some())
            .unwrap_or(false);
        if !on {
            return;
        }
        self.edit(cx, |ind| ind.style.visibility.set(u, Some((a, b))));
        for (slot, input) in &self.numbers {
            let text = match *slot {
                NumberSlot::VisFrom(x) if x == u => a.to_string(),
                NumberSlot::VisTo(x) if x == u => b.to_string(),
                _ => continue,
            };
            if input.read(cx).value().as_ref() != text {
                input.update(cx, |s, cx| s.set_value(text, window, cx));
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
        (
            read(NumberSlot::VisFrom(u)).unwrap_or(lo),
            read(NumberSlot::VisTo(u)).unwrap_or(hi),
        )
    }

    fn number(&self, slot: NumberSlot) -> Option<&Entity<InputState>> {
        self.numbers
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, e)| e)
    }

    fn color(&self, slot: ColorSlot) -> Option<&Entity<ColorPickerState>> {
        self.colors.iter().find(|(s, _)| *s == slot).map(|(_, e)| e)
    }

    /// The "Defaults ▾" menu, rebuilt each time it opens.
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
                MenuRow::new("ind-apply-default", "Apply default")
                    .disabled(!has_default)
                    .description("Your saved default for this indicator")
                    .on_select(on(Self::apply_default)),
            )
            .row(
                MenuRow::new("ind-save-default", "Save as default")
                    .description("New indicators of this kind start with these settings")
                    .on_select(on(Self::save_default)),
            );
        if has_default {
            menu = menu.row(
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
        open_dialog(window, cx, opts, panel);
    }

    // ---- rendering ------------------------------------------------------------

    /// `FormRow` callback that edits the indicator.
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

    fn input_control(
        &self,
        def: &'static InputDef,
        study: &Study,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = def.id;
        match (def.kind, study.get(id)) {
            (InputKind::Int { .. } | InputKind::Float { .. }, _) => {
                match self.number(NumberSlot::Input(id)) {
                    Some(s) => NumberField::new(s).into_any_element(),
                    None => div().into_any_element(),
                }
            }
            (InputKind::Bool, Some(InputValue::Bool(b))) => {
                let f = self.setter(cx, move |ind, v: bool| {
                    ind.study.set(id, InputValue::Bool(v));
                });
                Checkbox::new(eid("in", id))
                    .label(def.title)
                    .checked(b)
                    .on_click(move |v, w, cx| f(*v, w, cx))
                    .into_any_element()
            }
            (InputKind::Choice(opts), Some(InputValue::Choice(cur))) => Select::new(
                eid("in", id),
                opts.iter().enumerate().map(|(i, l)| (i as u16, *l)),
                cur,
            )
            .on_change(self.setter(cx, move |ind, v: u16| {
                ind.study.set(id, InputValue::Choice(v));
            }))
            .into_any_element(),
            (InputKind::Source, Some(InputValue::Source(cur))) => Select::new(
                eid("in", id),
                Source::ALL.iter().map(|s| (*s, s.label())),
                cur,
            )
            .on_change(self.setter(cx, move |ind, v: Source| {
                ind.study.set(id, InputValue::Source(v));
            }))
            .into_any_element(),
            (InputKind::Timeframe, Some(InputValue::Timeframe(cur))) => {
                let options = std::iter::once((None, "Chart".to_owned()))
                    .chain(Timeframe::ALL.iter().map(|t| (Some(*t), t.label())));
                Select::new(eid("in", id), options, cur)
                    .on_change(self.setter(cx, move |ind, v: Option<Timeframe>| {
                        ind.study.set(id, InputValue::Timeframe(v));
                    }))
                    .into_any_element()
            }
            (InputKind::Color, _) => match self.color(ColorSlot::Input(id)) {
                Some(s) => ColorField::new(s).into_any_element(),
                None => div().into_any_element(),
            },
            _ => div().into_any_element(),
        }
    }

    /// One row of inputs: a single input, or an inline run sharing a row
    /// (the first one's label or checkbox leads).
    fn input_row(&self, run: &[InputDef], study: &Study, cx: &Context<Self>) -> AnyElement {
        let meta = study.meta();
        let first = meta.input(run[0].id).expect("from the metadata");
        let enabled = first.enabled_if.is_none_or(|f| f(study));
        let id = first.id;
        let mut row = match (first.kind, study.get(id)) {
            (InputKind::Bool, Some(InputValue::Bool(b))) => FormRow::check(
                eid("in", id),
                first.title,
                b,
                self.setter(cx, move |ind, v: bool| {
                    ind.study.set(id, InputValue::Bool(v));
                }),
            ),
            _ => FormRow::new(first.title).control(self.input_control(first, study, cx)),
        }
        .label_width(SIZE.label_width());
        for d in &run[1..] {
            if let Some(def) = meta.input(d.id) {
                row = row.control(self.input_control(def, study, cx));
            }
        }
        if let Some(t) = first.tooltip {
            let muted = tt_ui::hsla(cx.ui().text_muted);
            row = row.control(
                div()
                    .id(eid("in-tip", id))
                    .child(glyph(Glyph::Lucide(Lucide::Info), ICON_XS).color(muted))
                    .tooltip(tip(t, None)),
            );
        }
        dim(row, enabled)
    }

    fn inputs_tab(
        &self,
        study: &Study,
        style: &IndicatorStyle,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let meta = study.meta();
        let mut out = Vec::new();
        let mut group = None;
        let mut i = 0;
        while i < meta.inputs.len() {
            let def = &meta.inputs[i];
            if def.group != group {
                group = def.group;
                if let Some(g) = group {
                    out.push(SectionHeader::new(g).into_any_element());
                }
            }
            let mut j = i + 1;
            if let Some(k) = def.inline {
                while j < meta.inputs.len()
                    && meta.inputs[j].inline == Some(k)
                    && meta.inputs[j].group == def.group
                {
                    j += 1;
                }
            }
            out.push(self.input_row(&meta.inputs[i..j], study, cx));
            i = j;
        }
        if !meta.inputs.is_empty() {
            out.push(Divider::horizontal().into_any_element());
        }
        out.push(
            FormRow::check(
                "inputs-in-status-line",
                "Inputs in status line",
                style.inputs_in_status_line,
                self.setter(cx, |ind, v| ind.style.inputs_in_status_line = v),
            )
            .label_width(SIZE.label_width() * 2.0)
            .into_any_element(),
        );
        out
    }

    fn swatch(&self, slot: ColorSlot, name: Option<&'static str>) -> AnyElement {
        let Some(s) = self.color(slot) else {
            return div().into_any_element();
        };
        let key = format!("{slot:?}");
        div()
            .id(ElementId::Name(SharedString::from(format!("swatch-{key}"))))
            .flex_none()
            .child(ColorField::new(s))
            .when_some(name, |d, n| d.tooltip(tip(n, None)))
            .into_any_element()
    }

    fn style_tab(
        &self,
        study: &Study,
        style: &IndicatorStyle,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let meta = study.meta();
        let lw = SIZE.label_width();
        let mut out = Vec::new();
        for def in meta.plots {
            let p = style.plot(def);
            let id = def.id;
            let enabled = def.enabled_if.is_none_or(|f| f(study));
            let n = def.palette.len().max(1);
            let swatches = h_flex().flex_none().gap(px(S1)).children((0..n).map(|i| {
                self.swatch(
                    ColorSlot::Plot(id, i),
                    def.palette.get(i).map(|(name, _)| *name),
                )
            }));
            let mut row = FormRow::check(
                eid("plot-vis", id),
                def.title,
                p.visible,
                self.setter(cx, move |ind, v| ind.style.plot_mut(id).visible = Some(v)),
            )
            .label_width(lw);
            let wide = n > 2;
            let mut swatches = Some(swatches);
            if !wide && let Some(s) = swatches.take() {
                row = row.control(s);
            }
            row = row
                .control(LineField::new(
                    eid("plot-line", id),
                    p.width,
                    p.line_style,
                    self.setter(cx, move |ind, w| ind.style.plot_mut(id).width = Some(w)),
                    self.setter(cx, move |ind, s| {
                        ind.style.plot_mut(id).line_style = Some(s)
                    }),
                ))
                .control(
                    Select::new(eid("plot-type", id), PLOT_TYPES, p.plot_type)
                        .width(W_TYPE_FIELD)
                        .on_change(
                            self.setter(cx, move |ind, t| {
                                ind.style.plot_mut(id).plot_type = Some(t)
                            }),
                        ),
                );
            let mut rows = vec![row.into_any_element()];
            if let Some(s) = swatches {
                // MACD's four histogram colors get their own row.
                rows.push(
                    FormRow::new("Colors")
                        .indent()
                        .label_width(lw)
                        .control(s)
                        .into_any_element(),
                );
            }
            rows.push(
                FormRow::check(
                    eid("plot-price-line", id),
                    "Price line",
                    p.price_line,
                    self.setter(cx, move |ind, v| {
                        ind.style.plot_mut(id).price_line = Some(v)
                    }),
                )
                .indent()
                .label_width(lw)
                .into_any_element(),
            );
            out.push(dim(v_flex().children(rows), enabled));
        }
        if !meta.hlines.is_empty() {
            out.push(SectionHeader::new("Levels").into_any_element());
        }
        for def in meta.hlines {
            let h = style.hline(def);
            let id = def.id;
            let mut row = FormRow::check(
                eid("hline-vis", id),
                def.title,
                h.visible,
                self.setter(cx, move |ind, v| ind.style.hline_mut(id).visible = Some(v)),
            )
            .label_width(lw)
            .control(self.swatch(ColorSlot::HLine(id), None))
            .control(LineField::new(
                eid("hline-line", id),
                h.width,
                h.style,
                self.setter(cx, move |ind, w| ind.style.hline_mut(id).width = Some(w)),
                self.setter(cx, move |ind, s| ind.style.hline_mut(id).style = Some(s)),
            ));
            if let Some(s) = self.number(NumberSlot::HLine(id)) {
                row = row.control(NumberField::new(s).width(W_TYPE_FIELD));
            }
            out.push(row.into_any_element());
        }
        if !meta.fills.is_empty() {
            out.push(SectionHeader::new("Fills").into_any_element());
        }
        for def in meta.fills {
            let f = style.fill(def);
            let id = def.id;
            let mut row = FormRow::check(
                eid("fill-vis", id),
                def.title,
                f.visible,
                self.setter(cx, move |ind, v| ind.style.fill_mut(id).visible = Some(v)),
            )
            .label_width(lw)
            .control(self.swatch(ColorSlot::Fill(id, false), None));
            if def.color2.is_some() {
                row = row.control(self.swatch(ColorSlot::Fill(id, true), None));
            }
            out.push(row.into_any_element());
        }
        out.push(Divider::horizontal().into_any_element());
        let precisions = std::iter::once((None, "Default".to_owned()))
            .chain((0..=MAX_PRECISION).map(|p| (Some(p), p.to_string())));
        out.push(
            FormRow::new("Precision")
                .label_width(lw)
                .control(
                    Select::new("precision", precisions, style.precision())
                        .on_change(self.setter(cx, |ind, p| ind.style.precision = p)),
                )
                .into_any_element(),
        );
        out.push(
            FormRow::check(
                "labels-on-price-scale",
                "Labels on price scale",
                style.labels_on_price_scale,
                self.setter(cx, |ind, v| ind.style.labels_on_price_scale = v),
            )
            .label_width(lw * 2.0)
            .into_any_element(),
        );
        out.push(
            FormRow::check(
                "values-in-status-line",
                "Values in status line",
                style.values_in_status_line,
                self.setter(cx, |ind, v| ind.style.values_in_status_line = v),
            )
            .label_width(lw * 2.0)
            .into_any_element(),
        );
        out
    }

    fn visibility_tab(&self, style: &IndicatorStyle, cx: &Context<Self>) -> Vec<AnyElement> {
        let this = cx.entity().downgrade();
        IntervalVisibility::UNITS
            .iter()
            .map(|&u| {
                let on = style.visibility.get(u).is_some();
                let this = this.clone();
                let mut row = FormRow::check(
                    eid("vis", unit_title(u)),
                    unit_title(u),
                    on,
                    move |v, _, cx| {
                        this.update(cx, |p, cx| {
                            let r = p.vis_fields(u, cx);
                            p.edit(cx, |ind| ind.style.visibility.set(u, v.then_some(r)));
                        })
                        .ok();
                    },
                )
                .label_width(SIZE.label_width() * 0.55);
                let field = |slot| {
                    self.number(slot)
                        .map(|s| NumberField::new(s).width(W_RANGE_FIELD))
                };
                if let Some(f) = field(NumberSlot::VisFrom(u)) {
                    row = row.control(f);
                }
                if let Some((_, s)) = self.sliders.iter().find(|(x, _)| *x == u) {
                    row = row.control(div().flex_1().min_w_0().child(Slider::new(s).disabled(!on)));
                }
                if let Some(f) = field(NumberSlot::VisTo(u)) {
                    row = row.control(f);
                }
                row.into_any_element()
            })
            .collect()
    }
}

/// `el` greyed out and inert when `enabled` is false (`enabled_if`).
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

impl Render for IndicatorSettingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((study, _, style)) = self.current(cx) else {
            return DialogFrame::new(SIZE)
                .id("indicator-settings")
                .body(EmptyState::new("This indicator was removed."));
        };
        let body = match self.tab {
            SettingsTab::Inputs => self.inputs_tab(&study, &style, cx),
            SettingsTab::Style => self.style_tab(&study, &style, cx),
            SettingsTab::Visibility => self.visibility_tab(&style, cx),
        };
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
        DialogFrame::new(SIZE)
            .id("indicator-settings")
            .nav(nav)
            .body(v_flex().pr(px(S2)).children(body))
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

// ---- line width / style field ---------------------------------------------------

type Pick<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

/// A line's width (1–4 px) and style (solid / dashed / dotted) in one field:
/// an outline trigger with a sample of the line, and a menu with both lists.
#[derive(IntoElement)]
struct LineField {
    id: ElementId,
    width: u8,
    style: LineStyle,
    on_width: Pick<u8>,
    on_style: Pick<LineStyle>,
}

impl LineField {
    fn new(
        id: ElementId,
        width: u8,
        style: LineStyle,
        on_width: impl Fn(u8, &mut Window, &mut App) + 'static,
        on_style: impl Fn(LineStyle, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id,
            width,
            style,
            on_width: Rc::new(on_width),
            on_style: Rc::new(on_style),
        }
    }
}

/// A short horizontal sample of a line.
fn line_sample(width: u8, style: LineStyle, color: Hsla) -> Div {
    let h = px(width.max(1) as f32);
    let seg = |w: f32| div().flex_none().w(px(w)).h(h).bg(color);
    let sample = h_flex().w(px(CONTROL_SM)).items_center();
    match style {
        LineStyle::Solid => sample.child(seg(CONTROL_SM)),
        LineStyle::Dashed => sample.gap(px(S0_5)).children((0..3).map(|_| seg(S1_5))),
        LineStyle::Dotted => sample
            .gap(px(S0_5))
            .children((0..6).map(|_| seg(width.max(1) as f32))),
    }
}

impl RenderOnce for LineField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let key = self.id.to_string();
        let nav = MenuNav::keyed(
            ElementId::Name(SharedString::from(format!("{key}-nav"))),
            window,
            cx,
        );
        let focus = nav.read(cx).focus.clone();
        let trigger = Button::new(ElementId::Name(SharedString::from(format!(
            "{key}-trigger"
        ))))
        .outline()
        .with_size(Size::Size(px(H_FIELD)))
        .h(px(H_FIELD))
        .w(px(W_LINE_FIELD))
        .px(px(S2))
        .py_0()
        .tooltip(format!(
            "{} px, {}",
            self.width,
            match self.style {
                LineStyle::Solid => "solid",
                LineStyle::Dashed => "dashed",
                LineStyle::Dotted => "dotted",
            }
        ))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap(px(S1))
                .child(line_sample(self.width, self.style, tt_ui::hsla(t.text)))
                .child(glyph(Glyph::CHEVRON_DOWN, ICON_XS).color(tt_ui::hsla(t.text_muted))),
        );
        let (width, style) = (self.width, self.style);
        let (on_width, on_style) = (self.on_width, self.on_style);
        Popover::new(ElementId::Name(SharedString::from(format!("{key}-pop"))))
            .anchor(Anchor::TopLeft)
            .offset(px(S1))
            .appearance(false)
            .track_focus(&focus)
            .trigger(trigger)
            .content(move |_, _, cx| {
                let pop = cx.entity();
                let mut menu = Menu::new()
                    .width(MenuWidth::Sm)
                    .nav(nav.clone())
                    .on_dismiss(move |w, cx| pop.update(cx, |p, cx| p.dismiss(w, cx)));
                for w in 1..=MAX_WIDTH {
                    let f = on_width.clone();
                    menu = menu.row(
                        MenuRow::new(("line-width", w as usize), format!("{w} px"))
                            .checked(w == width)
                            .on_select(move |win, cx| f(w, win, cx)),
                    );
                }
                menu = menu.separator();
                for (i, (s, label)) in LINE_STYLES.iter().enumerate() {
                    let (s, f) = (*s, on_style.clone());
                    menu = menu.row(
                        MenuRow::new(("line-style", i), *label)
                            .checked(s == style)
                            .on_select(move |win, cx| f(s, win, cx)),
                    );
                }
                menu
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

            let (numbers, colors) = slots(&ind.study);
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
