//! Chart "Settings" dialog, TradingView-style: a left nav (Symbol, Status
//! line, Scales and lines, Canvas) and rows of options for **one chart**
//! (the tab it was opened on). Every field of `ChartStyle` v2 that applies
//! to crypto charts has a row.
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
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, v_flex};
use gpui_kit::*;
use std::collections::HashMap;
use std::rc::Rc;
use tt_chart::style::LineKind;
use tt_chart::{ChartStyle, ChartType, ColorSlot};
use tt_theme::Theme;
use tt_ui::prelude::*;
use tt_ui::{DialogFooter, DialogOpts, DialogSize, TemplateMenu, hsla};

/// A color row control for the style field at `path` (get and set).
macro_rules! color {
    ($this:ident, $w:ident, $cx:ident, $id:expr, $($path:ident).+, $default:expr) => {
        $this.color(
            $id,
            move |s: &ChartStyle| s.$($path).+,
            move |s: &mut ChartStyle, v: ColorSlot| s.$($path).+ = v,
            $default,
            $w,
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

const SIZE: DialogSize = DialogSize::Lg;
/// Narrow selects inside multi-control rows (width, line style).
const W_SELECT_XS: f32 = 76.0;
const W_SELECT_SM: f32 = 104.0;
/// Selects with long option labels ("Visible on mouse over").
const W_SELECT_WIDE: f32 = tokens::W_FIELD_SELECT + tokens::S6 * 2.0;

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

struct ColorEntry {
    state: Entity<ColorPickerState>,
    effective: Effective,
    _sub: Subscription,
}

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
    colors: HashMap<SharedString, ColorEntry>,
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
            colors: HashMap::new(),
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
            DialogOpts::new("Settings", SIZE).footer(footer),
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
    /// field that keeps its own state (color pickers, number inputs).
    pub(crate) fn replace_draft(
        &mut self,
        style: ChartStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft = style;
        let theme = self.draft.theme(&self.base);
        for e in self.colors.values() {
            let v = hsla((e.effective)(&self.draft, &theme));
            e.state.update(cx, |p, cx| p.set_value(v, window, cx));
        }
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

    // ---- row helpers -------------------------------------------------------

    fn row(label: impl Into<SharedString>) -> FormRow {
        FormRow::new(label).label_width(SIZE.label_width())
    }

    /// A checkbox row.
    fn check(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: bool,
        set: impl Fn(&mut ChartStyle, bool) + 'static,
        cx: &mut Context<Self>,
    ) -> FormRow {
        let this = cx.entity().downgrade();
        FormRow::check(ElementId::Name(id.into()), label, value, move |v, _, cx| {
            this.update(cx, |p, cx| p.edit(cx, |s| set(s, v))).ok();
        })
        .label_width(SIZE.label_width())
    }

    /// A dropdown of fixed options.
    fn select<T: Clone + PartialEq + 'static>(
        &self,
        id: impl Into<SharedString>,
        options: impl IntoIterator<Item = (T, impl Into<SharedString>)>,
        current: T,
        set: impl Fn(&mut ChartStyle, T) + 'static,
        cx: &mut Context<Self>,
    ) -> Select<T> {
        let this = cx.entity().downgrade();
        Select::new(ElementId::Name(id.into()), options, current).on_change(move |v, _, cx| {
            this.update(cx, |p, cx| p.edit(cx, |s| set(s, v))).ok();
        })
    }

    /// Line width 1–4 px.
    fn width(
        &self,
        id: impl Into<SharedString>,
        current: f32,
        set: impl Fn(&mut ChartStyle, f32) + 'static,
        cx: &mut Context<Self>,
    ) -> Select<u8> {
        let cur = current.round().clamp(1.0, 4.0) as u8;
        self.select(
            id,
            (1..=4u8).map(|w| (w, format!("{w} px"))),
            cur,
            move |s, v| set(s, v as f32),
            cx,
        )
        .width(W_SELECT_XS)
    }

    /// Solid / Dotted / Dashed.
    fn line_kind(
        &self,
        id: impl Into<SharedString>,
        current: LineKind,
        set: impl Fn(&mut ChartStyle, LineKind) + 'static,
        cx: &mut Context<Self>,
    ) -> Select<LineKind> {
        self.select(
            id,
            LineKind::ALL.iter().map(|k| (*k, k.label())),
            current,
            set,
            cx,
        )
        .width(W_SELECT_SM)
    }

    /// A color swatch with the palette / opacity popover. `default` is the
    /// color drawn while the slot is `None` (the theme token).
    fn color(
        &mut self,
        id: impl Into<SharedString>,
        get: impl Fn(&ChartStyle) -> ColorSlot + 'static,
        set: impl Fn(&mut ChartStyle, ColorSlot) + 'static,
        default: impl Fn(&ChartStyle, &Theme) -> Color32 + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ColorField {
        let id = id.into();
        if !self.colors.contains_key(&id) {
            let effective: Effective = Rc::new(move |s, t| match get(s) {
                Some(c) => c.to_color32(),
                None => default(s, t),
            });
            let value = hsla(effective(&self.draft, &self.draft.theme(&self.base)));
            let state = cx.new(|cx| ColorPickerState::new(window, cx).default_value(value));
            let _sub = cx.subscribe(&state, move |this, _, ev: &ColorPickerEvent, cx| {
                if let ColorPickerEvent::Change(Some(c)) = ev {
                    let c = rgba(*c);
                    this.edit(cx, |s| set(s, Some(c)));
                }
            });
            self.colors.insert(
                id.clone(),
                ColorEntry {
                    state,
                    effective,
                    _sub,
                },
            );
        }
        ColorField::new(&self.colors[&id].state)
    }

    /// A number input clamped to `min..=max`.
    #[allow(clippy::too_many_arguments)]
    fn number(
        &mut self,
        id: impl Into<SharedString>,
        get: impl Fn(&ChartStyle) -> f64 + 'static,
        set: impl Fn(&mut ChartStyle, f64) + 'static,
        min: f64,
        max: f64,
        step: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> NumberField {
        let id = id.into();
        if !self.numbers.contains_key(&id) {
            let value = fmt_number(get(&self.draft));
            let state = cx.new(|cx| {
                InputState::new(window, cx)
                    .step(step)
                    .min(min)
                    .max(max)
                    .default_value(value)
            });
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
        NumberField::new(&self.numbers[&id].state)
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

impl Render for ChartSettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let mut nav = NavTabs::vertical().selected(self.page.id());
        for (_, id, label, icon) in Page::ALL {
            nav = nav.item(id, label, Some(icon.into()));
        }
        let nav = nav.on_change(move |id, _, cx| {
            if let Some((page, ..)) = Page::ALL.iter().find(|p| p.1 == id) {
                let page = *page;
                this.update(cx, |p, cx| {
                    p.page = page;
                    cx.notify();
                })
                .ok();
            }
        });
        let body = match self.page {
            Page::Symbol => self.symbol_page(window, cx),
            Page::StatusLine => self.status_line_page(cx),
            Page::Scales => self.scales_page(window, cx),
            Page::Canvas => self.canvas_page(window, cx),
        };
        DialogFrame::new(SIZE).id("chart-settings").nav(nav).body(
            v_flex()
                .id(SharedString::from(format!("page-{}", self.page.id())))
                .pr(px(tokens::S2))
                .pb(px(tokens::S2))
                .children(body),
        )
    }
}

/// A page's rows, in order.
type Rows = Vec<AnyElement>;

fn section(title: &'static str) -> AnyElement {
    div()
        .pt(px(tokens::S2))
        .child(SectionHeader::new(title))
        .into_any_element()
}
