//! The drawing settings dialog (drawings spec 6.4): Style, Text,
//! Coordinates and Visibility tabs for the selected drawing (or several
//! drawings of the same tool, which are edited together).
//!
//! The Style and Text tabs are generated from the family's form
//! (`tt_chart::drawings::form`), so tool families add fields there without
//! touching this file. Edits apply live (no undo steps); Ok records them as
//! one undo step and remembers the style as the tool's default; Cancel, Esc
//! or × restores the drawings as they were.

use crate::app::{TabChart, TradeTerm};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{
    Input, InputEvent, InputState, Textarea, TextareaState,
};
use gpui_kit::component::{Sizable as _, WindowExt as _, h_flex, v_flex};

use gpui_kit::*;
use serde_json::Value;
use std::collections::HashMap;
use std::rc::Rc;
use tt_chart::drawings::form::{self, Field, FieldKind, FONT_SIZES};
use tt_chart::{Drawing, DrawingTool, Drawings, LineStyle, Props, Rgba, Visibility};
use tt_chart_gpui::{ChartEvent, DrawingPrefs, TemplateTarget, template_menu};
use tt_core::TimeUnit;
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{DialogOpts, TemplateMenu, hsla, open_dialog};

const W_SMALL_SELECT: f32 = 92.0;
const W_COEFF: f32 = 72.0;
const W_TIME: f32 = 150.0;

/// The dialog's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Style,
    Text,
    Coordinates,
    Visibility,
}

impl Tab {
    fn id(self) -> &'static str {
        match self {
            Tab::Style => "style",
            Tab::Text => "text",
            Tab::Coordinates => "coords",
            Tab::Visibility => "visibility",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Tab::Style => "Style",
            Tab::Text => "Text",
            Tab::Coordinates => "Coordinates",
            Tab::Visibility => "Visibility",
        }
    }

    fn from_id(id: &str) -> Tab {
        match id {
            "text" => Tab::Text,
            "coords" => Tab::Coordinates,
            "visibility" => Tab::Visibility,
            _ => Tab::Style,
        }
    }
}

/// The last tab shown per tool (in memory, like TradingView).
#[derive(Default)]
struct LastTabs(HashMap<DrawingTool, Tab>);

impl Global for LastTabs {}

fn to_hsla(c: Rgba) -> Hsla {
    gpui_kit::Rgba {
        r: c.r() as f32 / 255.0,
        g: c.g() as f32 / 255.0,
        b: c.b() as f32 / 255.0,
        a: c.a() as f32 / 255.0,
    }
    .into()
}

fn from_hsla(c: Hsla) -> Rgba {
    let r = gpui_kit::Rgba::from(c);
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba::new(b(r.r), b(r.g), b(r.b), b(r.a))
}

/// "14", "2.5", "0.618" (no trailing zeros).
fn trim_number(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.to_owned()
    }
}

/// What a color picker of the dialog edits.
#[derive(Debug, Clone, PartialEq)]
enum ColorTarget {
    /// The color at a JSON pointer of the props.
    Path(String),
    /// Level `i`'s color.
    Level(usize),
    /// "Use one color": every level.
    AllLevels,
}

/// What a text input of the dialog edits.
#[derive(Debug, Clone, PartialEq)]
enum NumberTarget {
    /// A number at a JSON pointer.
    Path(String),
    /// A 0-1 fraction shown as percent.
    Percent(String),
    LevelCoeff(usize),
    PointTime(usize),
    PointPrice(usize),
    VisMin(TimeUnit),
    VisMax(TimeUnit),
}

pub struct DrawingSettingsPanel {
    chart: TabChart,
    /// The drawings edited (same tool; the first is shown).
    ids: Vec<u64>,
    tool: DrawingTool,
    /// Their state at open (Cancel restores it, Ok records the change).
    original: Vec<Drawing>,
    tab: Tab,
    style: Vec<Field>,
    text_fields: Vec<Field>,
    colors: Vec<(ColorTarget, Entity<ColorPickerState>)>,
    numbers: Vec<(NumberTarget, Entity<InputState>)>,
    text: Option<Entity<TextareaState>>,
    committed: bool,
    _subs: Vec<Subscription>,
}

impl DrawingSettingsPanel {
    fn drawings<'a>(chart: &TabChart, cx: &'a App) -> &'a Drawings {
        &chart.read(cx).state.read(cx).source.tab.chart.drawings
    }

    /// `None` if the drawing no longer exists.
    pub fn new(
        chart: TabChart,
        ids: Vec<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Self> {
        let ds = Self::drawings(&chart, cx);
        let original: Vec<Drawing> = ids
            .iter()
            .filter_map(|id| ds.items.iter().find(|d| d.id == *id).cloned())
            .collect();
        let first = original.first()?.clone();
        let tool = first.tool;
        let style = form::style_fields(tool, &first.props);
        let text_fields = form::text_fields(tool, &first.props);
        let tab = cx
            .try_global::<LastTabs>()
            .and_then(|t| t.0.get(&tool).copied())
            .unwrap_or(Tab::Style);
        let mut panel = Self {
            chart,
            ids: original.iter().map(|d| d.id).collect(),
            tool,
            original,
            tab,
            style,
            text_fields,
            colors: Vec::new(),
            numbers: Vec::new(),
            text: None,
            committed: false,
            _subs: Vec::new(),
        };
        panel.build_controls(&first, window, cx);
        if !panel.tabs().contains(&panel.tab) {
            panel.tab = Tab::Style;
        }
        // Closing any way but Ok (×, Esc, Cancel, clicking outside) restores
        // the drawings.
        panel._subs.push(cx.on_release(|this, cx| {
            if !this.committed {
                this.restore(cx);
            }
        }));
        Some(panel)
    }

    pub fn title(&self) -> String {
        match self.ids.len() {
            1 => self.tool.label().to_owned(),
            n => format!("{} ({n})", self.tool.label()),
        }
    }

    fn tabs(&self) -> Vec<Tab> {
        let mut t = vec![Tab::Style];
        if !self.text_fields.is_empty() {
            t.push(Tab::Text);
        }
        // A multi-selection hides Coordinates (spec 6.4).
        if self.ids.len() == 1 && form::has_coordinates(self.tool) {
            t.push(Tab::Coordinates);
        }
        t.push(Tab::Visibility);
        t
    }

    /// The color pickers and inputs of every tab, subscribed to edits.
    fn build_controls(&mut self, d: &Drawing, window: &mut Window, cx: &mut Context<Self>) {
        let v = form::props_value(&d.props);
        let mut colors = Vec::new();
        let mut numbers = Vec::new();
        let fields: Vec<Field> = self.style.iter().chain(&self.text_fields).cloned().collect();
        for f in &fields {
            match &f.kind {
                FieldKind::Color => colors.push(ColorTarget::Path(f.path.clone())),
                FieldKind::Line | FieldKind::TextStyle => {
                    colors.push(ColorTarget::Path(format!("{}/color", f.path)))
                }
                FieldKind::Number { .. } => numbers.push(NumberTarget::Path(f.path.clone())),
                FieldKind::Percent => numbers.push(NumberTarget::Percent(f.path.clone())),
                FieldKind::Levels => {
                    for i in 0..form::levels(&d.props).len() {
                        colors.push(ColorTarget::Level(i));
                        numbers.push(NumberTarget::LevelCoeff(i));
                    }
                    if matches!(d.props, Props::Fib(_)) {
                        colors.push(ColorTarget::AllLevels);
                    }
                }
                _ => {}
            }
        }
        for k in 0..d.points.len() {
            let (time, price) = form::coordinate_parts(d.tool, k);
            if time {
                numbers.push(NumberTarget::PointTime(k));
            }
            if price {
                numbers.push(NumberTarget::PointPrice(k));
            }
        }
        for u in Visibility::UNITS {
            numbers.push(NumberTarget::VisMin(u));
            numbers.push(NumberTarget::VisMax(u));
        }
        for target in colors {
            let value = color_value(&target, &d.props, &v);
            let picker =
                cx.new(|cx| ColorPickerState::new(window, cx).default_value(to_hsla(value)));
            let t = target.clone();
            self._subs
                .push(cx.subscribe(&picker, move |this, _, ev: &ColorPickerEvent, cx| {
                    if let ColorPickerEvent::Change(Some(c)) = ev {
                        let c = from_hsla(*c);
                        let t = t.clone();
                        this.edit(cx, move |d| match &t {
                            ColorTarget::Path(p) => {
                                form::set_color(&mut d.props, p, c);
                            }
                            ColorTarget::Level(i) => {
                                form::level::set_color(&mut d.props, *i, c);
                            }
                            ColorTarget::AllLevels => {
                                form::level::set_all_colors(&mut d.props, c);
                            }
                        });
                    }
                }));
            self.colors.push((target, picker));
        }
        for target in numbers {
            let text = number_text(&target, d, &v);
            let input = cx.new(|cx| {
                let s = InputState::new(window, cx).default_value(text);
                match &target {
                    NumberTarget::PointTime(_) => s.placeholder("YYYY-MM-DD HH:MM"),
                    _ => s,
                }
            });
            let t = target.clone();
            self._subs
                .push(cx.subscribe(&input, move |this, input, ev: &InputEvent, cx| {
                    if !matches!(ev, InputEvent::Change) {
                        return;
                    }
                    let s = input.read(cx).value().trim().to_owned();
                    this.number_changed(&t, &s, cx);
                }));
            self.numbers.push((target, input));
        }
        if form::has_text_content(d.tool, &d.props) {
            let text = d.text.clone();
            let area = cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(3, 8)
                    .default_value(text)
            });
            self._subs
                .push(cx.subscribe(&area, |this, area, ev: &InputEvent, cx| {
                    if !matches!(ev, InputEvent::Change) {
                        return;
                    }
                    let s = area.read(cx).value().to_string();
                    // Typing text turns it on (TradingView).
                    this.edit(cx, move |d| {
                        if !s.is_empty() && d.text.is_empty() {
                            d.props.set_text_on(true);
                        }
                        d.text = s.clone();
                    });
                }));
            self.text = Some(area);
        }
    }

    fn number_changed(&mut self, t: &NumberTarget, s: &str, cx: &mut Context<Self>) {
        let num = s.parse::<f64>().ok().filter(|v| v.is_finite());
        match t {
            NumberTarget::Path(p) => {
                if let Some(v) = num {
                    let p = p.clone();
                    self.edit(cx, move |d| {
                        // Integers stay integers (u8 / u16 fields).
                        let json = if v.fract() == 0.0 && v >= 0.0 {
                            Value::from(v as u64)
                        } else {
                            Value::from(v)
                        };
                        if !form::set(&mut d.props, &p, json) {
                            form::set(&mut d.props, &p, Value::from(v));
                        }
                    });
                }
            }
            NumberTarget::Percent(p) => {
                if let Some(v) = num {
                    let (p, v) = (p.clone(), (v / 100.0).clamp(0.0, 1.0));
                    self.edit(cx, move |d| {
                        form::set(&mut d.props, &p, Value::from(v));
                    });
                }
            }
            NumberTarget::LevelCoeff(i) => {
                if let Some(v) = num {
                    let i = *i;
                    self.edit(cx, move |d| {
                        form::level::set_coeff(&mut d.props, i, v);
                    });
                }
            }
            NumberTarget::PointTime(k) => {
                if let Some(ms) = form::parse_time(s) {
                    let k = *k;
                    self.edit(cx, move |d| {
                        form::set_point(&mut d.points, k, Some(ms), None);
                    });
                }
            }
            NumberTarget::PointPrice(k) => {
                if let Some(v) = num {
                    let k = *k;
                    self.edit(cx, move |d| {
                        form::set_point(&mut d.points, k, None, Some(v));
                    });
                }
            }
            NumberTarget::VisMin(u) | NumberTarget::VisMax(u) => {
                let Some(v) = num else {
                    return;
                };
                let (u, max_row) = (*u, Visibility::row_max(*u));
                let n = (v.round() as i64).clamp(1, max_row as i64) as u16;
                let is_min = matches!(t, NumberTarget::VisMin(_));
                self.edit(cx, move |d| {
                    let s = d.visibility.span_mut(u);
                    if is_min {
                        s.min = n;
                        s.max = s.max.max(n);
                    } else {
                        s.max = n;
                        s.min = s.min.min(n);
                    }
                });
            }
        }
    }

    /// Changes the edited drawings live and repaints the chart.
    fn edit(&mut self, cx: &mut Context<Self>, mut f: impl FnMut(&mut Drawing)) {
        let ids = self.ids.clone();
        self.chart.update(cx, |chart, cx| {
            chart.update_state(cx, |s| s.source.tab.chart.drawings.preview(&ids, &mut f))
        });
        cx.notify();
    }

    fn current(&self, cx: &App) -> Option<Drawing> {
        let ds = Self::drawings(&self.chart, cx);
        ds.items.iter().find(|d| Some(d.id) == self.ids.first().copied()).cloned()
    }

    /// Applies a template's style to the edited drawings and refreshes the
    /// controls.
    fn apply_style(&mut self, props: Props, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, |d| {
            d.apply_style(&props);
        });
        self.sync_controls(window, cx);
    }

    /// Puts the current values into every picker and input (after a
    /// template was applied).
    fn sync_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = self.current(cx) else {
            return;
        };
        let v = form::props_value(&d.props);
        for (t, picker) in &self.colors {
            let c = to_hsla(color_value(t, &d.props, &v));
            picker.update(cx, |p, cx| p.set_value(c, window, cx));
        }
        for (t, input) in &self.numbers {
            let s = number_text(t, &d, &v);
            if input.read(cx).value().trim() != s {
                input.update(cx, |i, cx| i.set_value(s, window, cx));
            }
        }
        cx.notify();
    }

    /// Ok: records the edit as one undo step and remembers the style as the
    /// tool's default.
    pub fn commit(&mut self, cx: &mut App) {
        self.committed = true;
        let original = self.original.clone();
        let tool = self.tool;
        let done = self.chart.update(cx, |chart, cx| {
            let done = chart.update_state(cx, |s| {
                s.source.tab.chart.drawings.commit_preview(&original)
            });
            if done {
                cx.emit(ChartEvent::DrawingsChanged);
            }
            done
        });
        if done && let Some(d) = self.current(cx) {
            DrawingPrefs::remember_style(cx, tool, &d.props);
        }
    }

    /// Restores the drawings as they were before the dialog opened.
    fn restore(&self, cx: &mut App) {
        let original = self.original.clone();
        self.chart.update(cx, |chart, cx| {
            chart.update_state(cx, |s| s.source.tab.chart.drawings.restore(&original))
        });
    }

    fn picker(&self, t: &ColorTarget) -> Option<Entity<ColorPickerState>> {
        self.colors.iter().find(|(x, _)| x == t).map(|(_, p)| p.clone())
    }

    fn input(&self, t: &NumberTarget) -> Option<Entity<InputState>> {
        self.numbers.iter().find(|(x, _)| x == t).map(|(_, p)| p.clone())
    }

    /// A setter of the value at `path` for checkboxes and selects.
    fn setter(&self, cx: &mut Context<Self>, path: String) -> Rc<dyn Fn(Value, &mut App)> {
        let this = cx.entity().downgrade();
        Rc::new(move |value, cx| {
            let (p, v) = (path.clone(), value.clone());
            this.update(cx, |this, cx| {
                this.edit(cx, move |d| {
                    form::set(&mut d.props, &p, v.clone());
                })
            })
            .ok();
        })
    }

    fn color_field(&self, t: ColorTarget) -> AnyElement {
        match self.picker(&t) {
            Some(p) => ColorField::new(&p).into_any_element(),
            None => div().into_any_element(),
        }
    }

    fn width_select(&self, id: String, path: String, now: u8, cx: &mut Context<Self>) -> AnyElement {
        let set = self.setter(cx, path);
        Select::new(
            SharedString::from(id),
            (1u8..=4).map(|w| (w, format!("{w}px"))),
            now,
        )
        .width(W_SMALL_SELECT)
        .on_change(move |w, _, cx| set(Value::from(w), cx))
        .into_any_element()
    }

    fn style_select(
        &self,
        id: String,
        path: String,
        now: LineStyle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let set = self.setter(cx, path);
        Select::new(
            SharedString::from(id),
            LineStyle::ALL.map(|s| (s, s.label())),
            now,
        )
        .width(W_SMALL_SELECT)
        .on_change(move |s, _, cx| {
            if let Ok(v) = serde_json::to_value(s) {
                set(v, cx)
            }
        })
        .into_any_element()
    }

    /// One field of the Style or Text tab.
    fn field(&self, f: &Field, d: &Drawing, v: &Value, cx: &mut Context<Self>) -> AnyElement {
        let id = |suffix: &str| format!("dlg{}{suffix}", f.path.replace('/', "-"));
        let label_w = DialogSize::Sm.label_width();
        let mut row = match &f.check {
            Some(c) if f.kind != FieldKind::Check => {
                let set = self.setter(cx, c.clone());
                FormRow::check(
                    SharedString::from(id("-on")),
                    f.label,
                    form::get_bool(v, c),
                    move |on, _, cx| set(Value::Bool(on), cx),
                )
            }
            _ => FormRow::new(f.label),
        }
        .label_width(label_w);
        if f.indent {
            row = row.indent();
        }
        match &f.kind {
            FieldKind::Section => SectionHeader::new(f.label).into_any_element(),
            FieldKind::Check => {
                let set = self.setter(cx, f.path.clone());
                FormRow::check(
                    SharedString::from(id("")),
                    f.label,
                    form::get_bool(v, &f.path),
                    move |on, _, cx| set(Value::Bool(on), cx),
                )
                .label_width(label_w)
                .into_any_element()
            }
            FieldKind::Color => row
                .control(self.color_field(ColorTarget::Path(f.path.clone())))
                .into_any_element(),
            FieldKind::Line => {
                let s = form::get_stroke(v, &f.path).unwrap_or_default();
                row.control(self.color_field(ColorTarget::Path(format!("{}/color", f.path))))
                    .control(self.width_select(id("-w"), format!("{}/width", f.path), s.width, cx))
                    .control(self.style_select(id("-s"), format!("{}/style", f.path), s.style, cx))
                    .into_any_element()
            }
            FieldKind::WidthStyle => {
                let w = form::get_f64(v, &format!("{}/width", f.path)) as u8;
                let st: LineStyle = form::get(v, &format!("{}/style", f.path))
                    .and_then(|x| serde_json::from_value(x.clone()).ok())
                    .unwrap_or_default();
                row.control(self.width_select(id("-w"), format!("{}/width", f.path), w, cx))
                    .control(self.style_select(id("-s"), format!("{}/style", f.path), st, cx))
                    .into_any_element()
            }
            FieldKind::Choice(options) => {
                let set = self.setter(cx, f.path.clone());
                let now = form::get_choice(v, &f.path);
                row.control(
                    Select::new(
                        SharedString::from(id("")),
                        options.iter().map(|(val, label)| (val.to_string(), *label)),
                        now,
                    )
                    .on_change(move |val: String, _, cx| set(form::choice_value(&val), cx)),
                )
                .into_any_element()
            }
            FieldKind::Number { .. } => match self.input(&NumberTarget::Path(f.path.clone())) {
                Some(i) => row.control(NumberField::new(&i)).into_any_element(),
                None => row.into_any_element(),
            },
            FieldKind::Percent => match self.input(&NumberTarget::Percent(f.path.clone())) {
                Some(i) => row
                    .control(NumberField::new(&i).width(W_SMALL_SELECT))
                    .unit("%")
                    .into_any_element(),
                None => row.into_any_element(),
            },
            FieldKind::FontSize => {
                let set = self.setter(cx, f.path.clone());
                let now = form::get_f64(v, &f.path) as u8;
                row.control(
                    Select::new(
                        SharedString::from(id("")),
                        FONT_SIZES.map(|s| (s, s.to_string())),
                        now,
                    )
                    .width(W_SMALL_SELECT)
                    .on_change(move |s: u8, _, cx| set(Value::from(s), cx)),
                )
                .into_any_element()
            }
            FieldKind::TextStyle => {
                let path = &f.path;
                let size = form::get_f64(v, &format!("{path}/size")) as u8;
                let bold = form::get_bool(v, &format!("{path}/bold"));
                let italic = form::get_bool(v, &format!("{path}/italic"));
                let set_size = self.setter(cx, format!("{path}/size"));
                let set_bold = self.setter(cx, format!("{path}/bold"));
                let set_italic = self.setter(cx, format!("{path}/italic"));
                row.control(self.color_field(ColorTarget::Path(format!("{path}/color"))))
                    .control(
                        Select::new(
                            SharedString::from(id("-size")),
                            FONT_SIZES.map(|s| (s, s.to_string())),
                            size,
                        )
                        .width(W_SMALL_SELECT - 16.0)
                        .on_change(move |s: u8, _, cx| set_size(Value::from(s), cx)),
                    )
                    .control(
                        IconButton::text(SharedString::from(id("-b")), "B")
                            .size(ControlSize::Sm)
                            .toggled(bold)
                            .tooltip("Bold")
                            .on_click(move |_, _, cx| set_bold(Value::Bool(!bold), cx)),
                    )
                    .control(
                        IconButton::text(SharedString::from(id("-i")), "I")
                            .size(ControlSize::Sm)
                            .toggled(italic)
                            .tooltip("Italic")
                            .on_click(move |_, _, cx| set_italic(Value::Bool(!italic), cx)),
                    )
                    .into_any_element()
            }
            FieldKind::Levels => self.levels(d, cx),
        }
    }

    /// Fib / channel levels: two columns of `[on] [coeff] [color]`, then
    /// "Use one color" (Fib).
    fn levels(&self, d: &Drawing, cx: &mut Context<Self>) -> AnyElement {
        let levels = form::levels(&d.props).to_vec();
        let this = cx.entity().downgrade();
        let cell = |i: usize, on: bool| {
            let t = this.clone();
            h_flex()
                .gap(px(S2))
                .items_center()
                .h(px(H_LIST_ROW))
                .child(
                    Checkbox::new(("dlg-level-on", i))
                        .checked(on)
                        .on_click(move |v, _, cx| {
                            let v = *v;
                            t.update(cx, |p, cx| {
                                p.edit(cx, move |d| {
                                    form::level::set_on(&mut d.props, i, v);
                                })
                            })
                            .ok();
                        }),
                )
                .children(self.input(&NumberTarget::LevelCoeff(i)).map(|inp| {
                    div()
                        .w(px(W_COEFF))
                        .child(Input::new(&inp).small().h(px(H_FIELD)))
                }))
                .child(self.color_field(ColorTarget::Level(i)))
        };
        let half = levels.len().div_ceil(2);
        let col = |range: std::ops::Range<usize>| {
            let mut c = v_flex().flex_1();
            for i in range {
                c = c.child(cell(i, levels[i].on));
            }
            c
        };
        let mut out = v_flex().child(
            h_flex()
                .gap(px(S4))
                .items_start()
                .child(col(0..half))
                .child(col(half..levels.len())),
        );
        if self.picker(&ColorTarget::AllLevels).is_some() {
            out = out.child(
                FormRow::new("Use one color")
                    .label_width(DialogSize::Sm.label_width())
                    .control(self.color_field(ColorTarget::AllLevels)),
            );
        }
        out.into_any_element()
    }

    fn coordinates(&self, d: &Drawing, cx: &App) -> AnyElement {
        let mut body = v_flex();
        for k in 0..d.points.len() {
            let mut row = FormRow::new(SharedString::from(form::point_label(k)))
                .label_width(DialogSize::Sm.label_width() - 80.0);
            if let Some(i) = self.input(&NumberTarget::PointTime(k)) {
                row = row.control(div().w(px(W_TIME)).child(Input::new(&i).small().h(px(H_FIELD))));
            }
            if let Some(i) = self.input(&NumberTarget::PointPrice(k)) {
                row = row.control(NumberField::new(&i));
            }
            body = body.child(row);
        }
        body.child(
            div()
                .pt(px(S2))
                .text_size(px(TEXT_SMALL))
                .text_color(hsla(cx.ui().text_muted))
                .child("Bar time (UTC, YYYY-MM-DD HH:MM) and price of each point."),
        )
        .into_any_element()
    }

    fn visibility(&self, d: &Drawing, cx: &mut Context<Self>) -> AnyElement {
        let mut body = v_flex();
        let this = cx.entity().downgrade();
        for u in Visibility::UNITS {
            let span = d.visibility.span(u);
            let t = this.clone();
            let mut row = FormRow::check(
                SharedString::from(format!("dlg-vis-{}", form::unit_label(u))),
                form::unit_label(u),
                span.on,
                move |on, _, cx| {
                    t.update(cx, |p, cx| p.edit(cx, move |d| d.visibility.span_mut(u).on = on))
                        .ok();
                },
            )
            .label_width(DialogSize::Sm.label_width() - 40.0);
            if let Some(i) = self.input(&NumberTarget::VisMin(u)) {
                row = row.control(NumberField::new(&i).width(W_SMALL_SELECT));
            }
            row = row.control(div().child("–"));
            if let Some(i) = self.input(&NumberTarget::VisMax(u)) {
                row = row.control(NumberField::new(&i).width(W_SMALL_SELECT));
            }
            body = body.child(row);
        }
        body.into_any_element()
    }
}


/// The color a picker shows.
fn color_value(t: &ColorTarget, props: &Props, v: &Value) -> Rgba {
    match t {
        ColorTarget::Path(p) => form::get_color(v, p).unwrap_or(Rgba::rgb(0, 0, 0)),
        ColorTarget::Level(i) => form::level::color(props, *i),
        ColorTarget::AllLevels => form::levels(props)
            .first()
            .map_or(Rgba::rgb(0x78, 0x7b, 0x86), |l| l.color),
    }
}

/// The text an input shows.
fn number_text(t: &NumberTarget, d: &Drawing, v: &Value) -> String {
    match t {
        NumberTarget::Path(p) => trim_number(form::get_f64(v, p)),
        NumberTarget::Percent(p) => trim_number((form::get_f64(v, p) * 100.0).round()),
        NumberTarget::LevelCoeff(i) => form::levels(&d.props)
            .get(*i)
            .map_or_else(String::new, |l| trim_number(l.coeff)),
        NumberTarget::PointTime(k) => d
            .points
            .get(*k)
            .map_or_else(String::new, |a| form::format_time(a.time_ms)),
        NumberTarget::PointPrice(k) => d
            .points
            .get(*k)
            .map_or_else(String::new, |a| trim_number(a.price)),
        NumberTarget::VisMin(u) => d.visibility.span(*u).min.to_string(),
        NumberTarget::VisMax(u) => d.visibility.span(*u).max.to_string(),
    }
}

impl Render for DrawingSettingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(d) = self.current(cx) else {
            return div().child("This drawing was removed.").into_any_element();
        };
        let v = form::props_value(&d.props);
        let tabs = self.tabs();
        let this = cx.entity().downgrade();
        let tool = self.tool;
        let mut nav = NavTabs::horizontal()
            .selected(self.tab.id())
            .on_change(move |id, _, cx| {
                let tab = Tab::from_id(id);
                cx.default_global::<LastTabs>().0.insert(tool, tab);
                this.update(cx, |p, cx| {
                    p.tab = tab;
                    cx.notify();
                })
                .ok();
            });
        for t in &tabs {
            nav = nav.item(t.id(), t.label(), None);
        }
        let body: AnyElement = match self.tab {
            Tab::Style => {
                let fields = self.style.clone();
                let mut b = v_flex();
                for f in &fields {
                    b = b.child(self.field(f, &d, &v, cx));
                }
                b.into_any_element()
            }
            Tab::Text => {
                let fields = self.text_fields.clone();
                let mut b = v_flex().gap(px(S1));
                if let Some(area) = &self.text {
                    b = b.child(div().py(px(S2)).child(Textarea::new(area).small()));
                }
                for f in &fields {
                    b = b.child(self.field(f, &d, &v, cx));
                }
                b.into_any_element()
            }
            Tab::Coordinates => self.coordinates(&d, cx),
            Tab::Visibility => self.visibility(&d, cx),
        };
        DialogFrame::new(DialogSize::Md)
            .id("drawing-settings")
            .nav(nav)
            .body(div().pr(px(S2)).child(body))
            .into_any_element()
    }
}

impl TradeTerm {
    /// Opens the drawing settings dialog for drawing `id` (double-click,
    /// the toolbar's gear or "Settings…"). A selection of drawings of the
    /// same tool that includes `id` is edited together.
    pub(crate) fn open_drawing_settings(
        &mut self,
        chart: TabChart,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_dialog(cx) {
            return;
        }
        let ids: Vec<u64> = {
            let ds = DrawingSettingsPanel::drawings(&chart, cx);
            let Some(tool) = ds.items.iter().find(|d| d.id == id).map(|d| d.tool) else {
                return;
            };
            let sel: Vec<u64> = ds.selected_drawings().map(|d| d.id).collect();
            if sel.contains(&id) && ds.selected_drawings().all(|d| d.tool == tool) {
                // The clicked one first (its values are shown).
                std::iter::once(id)
                    .chain(sel.into_iter().filter(|s| *s != id))
                    .collect()
            } else {
                vec![id]
            }
        };
        // The drawing exists (checked above), so the panel has one.
        let panel = cx.new(|cx| {
            DrawingSettingsPanel::new(chart, ids, window, cx).expect("drawing checked above")
        });
        let title = panel.read(cx).title();
        let tool = panel.read(cx).tool;
        let (cur, apply, ok) = (panel.downgrade(), panel.downgrade(), panel.downgrade());
        let target = TemplateTarget {
            tool,
            current: Rc::new(move |cx| cur.upgrade()?.read(cx).current(cx).map(|d| d.props)),
            apply: Rc::new(move |props, window, cx| {
                apply
                    .update(cx, |p, cx| p.apply_style(props, window, cx))
                    .ok();
            }),
        };
        let footer = DialogFooter::new()
            .template(TemplateMenu::build(move |menu, _, cx| {
                template_menu(menu, target.clone(), cx)
            }))
            .cancel()
            .ok("Ok")
            .on_ok(move |_, cx| {
                ok.update(cx, |p, cx| p.commit(cx)).ok();
                true
            });
        open_dialog(
            window,
            cx,
            DialogOpts::new(title, DialogSize::Sm).footer(footer),
            panel,
        );
    }

    /// Saves drawing preferences when they change (templates, last-used
    /// styles, the toolbar's place) and gives every chart the tools' new
    /// defaults.
    pub(crate) fn observe_drawing_prefs(&mut self, cx: &mut Context<Self>) {
        let sub = cx.observe_global::<DrawingPrefs>(|app, cx| {
            for chart in app.tabs.clone() {
                app.apply_drawing_defaults_to(&chart, cx);
            }
            app.persist(cx);
        });
        self._subscriptions.push(sub);
    }

    /// The style new drawings of each tool start from, in one chart.
    pub(crate) fn apply_drawing_defaults_to(&self, chart: &TabChart, cx: &mut App) {
        let defaults = DrawingPrefs::get(cx).templates.defaults();
        chart.update(cx, |c, cx| {
            c.state.update(cx, |s, _| {
                s.source.tab.chart.drawings.set_tool_defaults(defaults)
            })
        });
    }
}
