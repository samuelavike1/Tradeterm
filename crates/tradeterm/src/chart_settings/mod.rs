//! Chart "Settings" dialog, laid out like TradingView's
//! (docs/specs/chart-settings-tv-ref.md): the comfortable density
//! (ui-guidelines §2.10), a 236 px left nav with icons (Symbol, Status
//! line, Scales and lines, Canvas) and rows of options for **one chart**
//! (the tab it was opened on). Every field of `ChartStyle` v2 that applies
//! to crypto charts has a row.
//!
//! Rows: checkbox + label, then the controls (`ColorSwatchButton`,
//! `LinePicker`, comfortable `Select` / `CheckSelect`, `Field`); each page
//! has one label column as wide as its widest label (+ 20), like
//! TradingView's table; a lone checkbox spans the row.
//!
//! Changes preview live on that chart; Ok keeps them for the tab, Cancel / ×
//! / Esc restore the previous settings. The footer has the Template ▾ menu
//! (Save as…, Save as default, Apply defaults, Factory defaults, saved
//! templates with delete, Rename…) and "Apply to all tabs".
//!
//! Chrome (labels, headers, nav) reads the UI theme (`cx.ui()`); only color
//! swatches show draft colors.

use crate::app::TradeTerm;
use ecolor::Color32;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, v_flex};
use gpui_kit::*;
use std::collections::HashMap;
use std::rc::Rc;
use tt_chart::style::LineKind;
use tt_chart::{ChartStyle, ChartType, ColorSlot};
use tt_theme::Theme;
use tt_ui::prelude::*;
use tt_ui::side_nav::{C_SUB_INDENT, C_W_SETTINGS_DIALOG};
use tt_ui::tokens::{C_CHECKBOX, C_CONTROL, C_GAP, C_LABEL_GAP, C_R_CONTROL, TEXT_BODY};
use tt_ui::{
    DialogFooter, DialogOpts, DialogSize, SectionTitle, SideNavFrame, TemplateMenu, hsla, sub_row,
};

/// A color swatch for the style field at `path` (get and set).
macro_rules! color {
    ($this:ident, $cx:ident, $id:expr, $($path:ident).+, $default:expr) => {
        $this.color(
            $id,
            move |s: &ChartStyle| s.$($path).+,
            move |s: &mut ChartStyle, v: ColorSlot| s.$($path).+ = v,
            $default,
            $cx,
        )
    };
}

/// A checkbox row for the bool style field at `path`.
macro_rules! check {
    ($this:ident, $cx:ident, $id:expr, $label:expr, $($path:ident).+) => {
        $this.check(
            $id,
            $label,
            $this.draft.$($path).+,
            move |s: &mut ChartStyle, v: bool| s.$($path).+ = v,
            $cx,
        )
    };
}

mod canvas;
mod scales;
mod status_line;
mod symbol;
mod templates;

/// TradingView's select widths (default / normal / big).
const W_SELECT_SM: f32 = 100.0;
const W_SELECT: f32 = 150.0;
const W_SELECT_LG: f32 = 180.0;
/// Number fields: margins (100) and the wider ones (lock ratio, base
/// level, box sizes: 150).
const W_FIELD_SM: f32 = 100.0;
const W_FIELD: f32 = 150.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Symbol,
    StatusLine,
    Scales,
    Canvas,
}

impl Page {
    const ALL: [(Page, &'static str, &'static str, lucide_icons::Icon); 4] = [
        (
            Page::Symbol,
            "symbol",
            "Symbol",
            lucide_icons::Icon::ChartCandlestick,
        ),
        (
            Page::StatusLine,
            "status",
            "Status line",
            lucide_icons::Icon::AlignLeft,
        ),
        (
            Page::Scales,
            "scales",
            "Scales and lines",
            lucide_icons::Icon::Axis3d,
        ),
        (
            Page::Canvas,
            "canvas",
            "Canvas",
            lucide_icons::Icon::Paintbrush,
        ),
    ];

    fn id(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|p| p.0 == self)
            .map_or("symbol", |p| p.1)
    }
}

type Effective = Rc<dyn Fn(&ChartStyle, &Theme) -> Color32>;
type NumGet = Rc<dyn Fn(&ChartStyle) -> f64>;

struct NumberEntry {
    state: Entity<InputState>,
    get: NumGet,
    _sub: Subscription,
}

pub struct ChartSettingsPanel {
    app: WeakEntity<TradeTerm>,
    /// `ChartTab::id` of the chart being edited.
    tab_id: u64,
    chart_type: ChartType,
    /// The palette without style overrides (dark or light).
    base: Theme,
    /// The tab's own style when the dialog opened (`None`: it followed the
    /// user default), restored on Cancel.
    original: Option<ChartStyle>,
    draft: ChartStyle,
    page: Page,
    /// Created on first render of their row, keyed by field id.
    numbers: HashMap<SharedString, NumberEntry>,
    committed: bool,
    _subs: Vec<Subscription>,
}

impl ChartSettingsPanel {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app: WeakEntity<TradeTerm>,
        tab_id: u64,
        chart_type: ChartType,
        original: Option<ChartStyle>,
        style: ChartStyle,
        base: Theme,
        page: Page,
        cx: &mut Context<Self>,
    ) -> Self {
        // Closed without Ok (×, Esc, Cancel, outside click): restore.
        let release = cx.on_release(|this, cx| {
            if !this.committed {
                let (app, tab, original) = (this.app.clone(), this.tab_id, this.original.clone());
                app.update(cx, |a, cx| a.restore_tab_style(tab, original, cx))
                    .ok();
            }
        });
        Self {
            app,
            tab_id,
            chart_type,
            base,
            original,
            draft: style,
            page,
            numbers: HashMap::new(),
            committed: false,
            _subs: vec![release],
        }
    }

    /// Opens the dialog for the active tab.
    pub fn open(app: &mut TradeTerm, page: Page, window: &mut Window, cx: &mut Context<TradeTerm>) {
        let Some((tab_id, chart_type, own, style)) = app.active_tab_style(cx) else {
            return;
        };
        let weak = cx.entity().downgrade();
        let base = Theme::for_dark(app.theme.dark);
        let panel = cx.new(|cx| {
            ChartSettingsPanel::new(weak, tab_id, chart_type, own, style, base, page, cx)
        });
        let (p1, p2, p3) = (panel.clone(), panel.downgrade(), panel.downgrade());
        let footer = DialogFooter::new()
            .comfortable()
            .template(TemplateMenu::build(move |menu, _, cx| {
                templates::menu(&p1, menu, cx)
            }))
            .action("Apply to all tabs", move |window, cx| {
                p2.update(cx, |p, cx| p.apply_to_all(cx)).ok();
                window.close_dialog(cx);
            })
            .cancel()
            .ok("Ok")
            .on_ok(move |_, cx| {
                p3.update(cx, |p, cx| p.commit(cx)).ok();
                true
            });
        tt_ui::open_dialog(
            window,
            cx,
            DialogOpts::new("Settings", DialogSize::Lg)
                .comfortable()
                .width(C_W_SETTINGS_DIALOG)
                .footer(footer),
            panel,
        );
    }

    /// Changes the draft and previews it on the chart.
    fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut ChartStyle)) {
        f(&mut self.draft);
        self.preview(cx);
        cx.notify();
    }

    fn preview(&self, cx: &mut Context<Self>) {
        let (app, tab, style) = (self.app.clone(), self.tab_id, self.draft.clone());
        cx.defer(move |cx| {
            app.update(cx, |a, cx| a.preview_tab_style(tab, style, cx))
                .ok();
        });
    }

    /// Replaces the whole draft (template, defaults) and refreshes every
    /// field that keeps its own state (number inputs).
    pub(crate) fn replace_draft(
        &mut self,
        style: ChartStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft = style;
        for e in self.numbers.values() {
            let v = fmt_number((e.get)(&self.draft));
            e.state.update(cx, |s, cx| s.set_value(v, window, cx));
        }
        self.preview(cx);
        cx.notify();
    }

    pub(crate) fn draft(&self) -> &ChartStyle {
        &self.draft
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        self.committed = true;
        let (tab, style) = (self.tab_id, self.draft.clone());
        self.app
            .update(cx, |a, cx| a.commit_tab_style(tab, style, cx))
            .ok();
    }

    /// The draft becomes the user default and every tab follows it.
    fn apply_to_all(&mut self, cx: &mut Context<Self>) {
        self.committed = true;
        let style = self.draft.clone();
        self.app
            .update(cx, |a, cx| a.apply_style_to_all_tabs(style, cx))
            .ok();
    }

    /// "Save as default": the draft becomes the user default (tabs that
    /// follow the default change too); the dialog stays open.
    pub(crate) fn save_as_default(&mut self, cx: &mut Context<Self>) {
        let style = self.draft.clone();
        self.app
            .update(cx, |a, cx| a.set_default_style(style, cx))
            .ok();
    }

    /// The draft's palette (theme tokens with its overrides).
    fn theme(&self) -> Theme {
        self.draft.theme(&self.base)
    }

    // ---- row helpers -------------------------------------------------------

    /// A labelled row.
    fn row(label: impl Into<SharedString>) -> Line {
        let label = label.into();
        Line {
            label: label.clone(),
            check: false,
            controls: false,
            row: FormRow::new(label).comfortable(),
        }
    }

    /// A checkbox row (spans the row while it has no controls).
    fn check(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: bool,
        set: impl Fn(&mut ChartStyle, bool) + 'static,
        cx: &mut Context<Self>,
    ) -> Line {
        let label = label.into();
        let this = cx.entity().downgrade();
        Line {
            label: label.clone(),
            check: true,
            controls: false,
            row: FormRow::check(ElementId::Name(id.into()), label, value, move |v, _, cx| {
                this.update(cx, |p, cx| p.edit(cx, |s| set(s, v))).ok();
            })
            .comfortable(),
        }
    }

    /// A follow-up checkbox (42 px, indented under the row above).
    fn sub_check(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: bool,
        set: impl Fn(&mut ChartStyle, bool) + 'static,
        cx: &mut Context<Self>,
    ) -> Item {
        let this = cx.entity().downgrade();
        Item::Sub(
            false,
            CheckBox::new(ElementId::Name(id.into()), label, value)
                .on_change(move |v, _, cx| {
                    this.update(cx, |p, cx| p.edit(cx, |s| set(s, v))).ok();
                })
                .into_any_element(),
        )
    }

    /// A comfortable dropdown of fixed options (`W_SELECT_SM` wide unless
    /// widened).
    fn select<T: Clone + PartialEq + 'static>(
        &self,
        id: impl Into<SharedString>,
        options: impl IntoIterator<Item = (T, impl Into<SharedString>)>,
        current: T,
        set: impl Fn(&mut ChartStyle, T) + 'static,
        cx: &mut Context<Self>,
    ) -> Select<T> {
        let this = cx.entity().downgrade();
        Select::new(ElementId::Name(id.into()), options, current)
            .comfortable()
            .on_change(move |v, _, cx| {
                this.update(cx, |p, cx| p.edit(cx, |s| set(s, v))).ok();
            })
    }

    /// A color swatch with TradingView's palette / opacity popover.
    /// `default` is the color drawn while the slot is `None` (the theme
    /// token); picking a color stores it.
    fn color(
        &self,
        id: impl Into<SharedString>,
        get: impl Fn(&ChartStyle) -> ColorSlot + 'static,
        set: impl Fn(&mut ChartStyle, ColorSlot) + 'static,
        default: impl Fn(&ChartStyle, &Theme) -> Color32 + 'static,
        cx: &mut Context<Self>,
    ) -> ColorSwatchButton {
        let now = match get(&self.draft) {
            Some(c) => c.to_color32(),
            None => default(&self.draft, &self.theme()),
        };
        let this = cx.entity().downgrade();
        ColorSwatchButton::new(ElementId::Name(id.into()), hsla(now)).on_change(move |c, _, cx| {
            let c = rgba(c);
            this.update(cx, |p, cx| p.edit(cx, |s| set(s, Some(c))))
                .ok();
        })
    }

    /// TradingView's line control: color (+ opacity), and the thickness
    /// and style when the field has them.
    #[allow(clippy::too_many_arguments)]
    fn line(
        &self,
        id: impl Into<SharedString>,
        get: impl Fn(&ChartStyle) -> ColorSlot + 'static,
        set: impl Fn(&mut ChartStyle, ColorSlot) + 'static,
        default: impl Fn(&ChartStyle, &Theme) -> Color32 + 'static,
        width: Option<(f32, WidthSet)>,
        kind: Option<(LineKind, KindSet)>,
        cx: &mut Context<Self>,
    ) -> LinePicker {
        let effective: Effective = Rc::new(move |s, t| match get(s) {
            Some(c) => c.to_color32(),
            None => default(s, t),
        });
        let color = hsla(effective(&self.draft, &self.theme()));
        let value = LineValue {
            color,
            width: width
                .as_ref()
                .map_or(1, |(w, _)| w.round().clamp(1.0, 4.0) as u8),
            dash: kind.as_ref().map_or(LineDash::Solid, |(k, _)| dash(*k)),
        };
        let (has_width, has_kind) = (width.is_some(), kind.is_some());
        let set_w = width.map(|(_, f)| f);
        let set_k = kind.map(|(_, f)| f);
        let this = cx.entity().downgrade();
        let picker =
            LinePicker::new(ElementId::Name(id.into()), value).on_change(move |lv, _, cx| {
                // Only what changed: a width change keeps a theme-default
                // color following the theme.
                let apply = |s: &mut ChartStyle| {
                    if lv.color != value.color {
                        set(s, Some(rgba(lv.color)));
                    }
                    if let Some(f) = &set_w
                        && lv.width != value.width
                    {
                        f(s, lv.width as f32);
                    }
                    if let Some(f) = &set_k
                        && lv.dash != value.dash
                    {
                        f(s, line_kind(lv.dash));
                    }
                };
                this.update(cx, |p, cx| p.edit(cx, apply)).ok();
            });
        let picker = if has_width {
            picker
        } else {
            picker.no_thickness()
        };
        if has_kind { picker } else { picker.no_style() }
    }

    /// A number field (`w` wide) clamped to `min..=max`.
    #[allow(clippy::too_many_arguments)]
    fn number(
        &mut self,
        id: impl Into<SharedString>,
        get: impl Fn(&ChartStyle) -> f64 + 'static,
        set: impl Fn(&mut ChartStyle, f64) + 'static,
        min: f64,
        max: f64,
        w: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Field {
        let id = id.into();
        if !self.numbers.contains_key(&id) {
            let value = fmt_number(get(&self.draft));
            let state = cx.new(|cx| InputState::new(window, cx).default_value(value));
            let _sub = cx.subscribe(&state, move |this, input, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev
                    && let Ok(v) = input.read(cx).value().trim().parse::<f64>()
                    && v.is_finite()
                {
                    this.edit(cx, |s| set(s, v.clamp(min, max)));
                }
            });
            self.numbers.insert(
                id.clone(),
                NumberEntry {
                    state,
                    get: Rc::new(get),
                    _sub,
                },
            );
        }
        Field::new(&self.numbers[&id].state).width(w)
    }
}

type WidthSet = Rc<dyn Fn(&mut ChartStyle, f32)>;
type KindSet = Rc<dyn Fn(&mut ChartStyle, LineKind)>;

fn dash(k: LineKind) -> LineDash {
    match k {
        LineKind::Solid => LineDash::Solid,
        LineKind::Dotted => LineDash::Dotted,
        LineKind::Dashed => LineDash::Dashed,
    }
}

fn line_kind(d: LineDash) -> LineKind {
    match d {
        LineDash::Solid => LineKind::Solid,
        LineDash::Dotted => LineKind::Dotted,
        LineDash::Dashed => LineKind::Dashed,
    }
}

/// A picked color, unmultiplied (as the picker shows it).
fn rgba(c: Hsla) -> tt_chart::Rgba {
    let r = Rgba::from(c);
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    tt_chart::Rgba::new(b(r.r), b(r.g), b(r.b), b(r.a))
}

fn fmt_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// `c` at `alpha` (0..=1) opacity.
fn alpha(c: Color32, a: f32) -> Color32 {
    tt_theme::with_alpha(c, a)
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

/// A field that is off (TradingView's disabled "Lock price to bar ratio"
/// number): the value, muted, in the field's outline.
fn disabled_field(value: String, w: f32, cx: &App) -> AnyElement {
    let t = cx.ui();
    div()
        .flex_none()
        .w(px(w))
        .h(px(C_CONTROL))
        .px(px(tokens::S3))
        .flex()
        .items_center()
        .rounded(px(C_R_CONTROL))
        .border_1()
        .border_color(hsla(t.border))
        .text_size(px(TEXT_BODY))
        .text_color(hsla(t.text_muted))
        .opacity(0.6)
        .child(value)
        .into_any_element()
}

/// A row being built: its label (measured for the page's label column)
/// and the kit `FormRow`.
pub(super) struct Line {
    label: SharedString,
    check: bool,
    controls: bool,
    row: FormRow,
}

impl Line {
    fn control(mut self, c: impl IntoElement) -> Self {
        self.controls = true;
        self.row = self.row.control(c);
        self
    }

    fn unit(mut self, u: impl Into<SharedString>) -> Self {
        self.controls = true;
        self.row = self.row.unit(u);
        self
    }
}

/// One entry of a page.
pub(super) enum Item {
    Section(SharedString),
    Line(Line),
    /// A 42 px follow-up row: in the control column (`true`) or indented
    /// under a checkbox (`false`).
    Sub(bool, AnyElement),
}

impl From<Line> for Item {
    fn from(l: Line) -> Self {
        Item::Line(l)
    }
}

/// A page's entries, in order.
type Rows = Vec<Item>;

fn section(title: impl Into<SharedString>) -> Item {
    Item::Section(title.into())
}

/// Lays out a page: one label column for its labelled rows.
fn layout(rows: Rows, window: &Window) -> Vec<AnyElement> {
    let check = C_CHECKBOX + C_GAP;
    let lw = rows
        .iter()
        .filter_map(|r| match r {
            Item::Line(l) if l.controls => {
                Some(text_w(window, &l.label) + if l.check { check } else { 0.0 })
            }
            _ => None,
        })
        .fold(0.0, f32::max)
        + C_LABEL_GAP;
    rows.into_iter()
        .enumerate()
        .map(|(i, r)| match r {
            Item::Section(t) => SectionTitle::new(t).first(i == 0).into_any_element(),
            Item::Line(l) => l.row.label_width(lw).into_any_element(),
            Item::Sub(controls, el) => {
                sub_row(if controls { lw } else { C_SUB_INDENT }, el).into_any_element()
            }
        })
        .collect()
}

impl Render for ChartSettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        // One id per page: each page opens scrolled to its top.
        let mut frame =
            SideNavFrame::new(SharedString::from(format!("chart-settings-{}", self.page.id())))
                .selected(self.page.id());
        for (_, id, label, icon) in Page::ALL {
            frame = frame.item(id, label, Some(icon.into()));
        }
        let frame = frame.on_change(move |id, _, cx| {
            if let Some((page, ..)) = Page::ALL.iter().find(|p| p.1 == id) {
                let page = *page;
                this.update(cx, |p, cx| {
                    p.page = page;
                    cx.notify();
                })
                .ok();
            }
        });
        let rows = match self.page {
            Page::Symbol => self.symbol_page(window, cx),
            Page::StatusLine => self.status_line_page(cx),
            Page::Scales => self.scales_page(window, cx),
            Page::Canvas => self.canvas_page(window, cx),
        };
        frame.body(
            v_flex()
                .id(SharedString::from(format!("page-{}", self.page.id())))
                .children(layout(rows, window)),
        )
    }
}
