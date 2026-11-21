//! Color and line pickers on the comfortable density (§2.10), measured on
//! TradingView's drawing dialog and floating toolbar:
//!
//! - [`ColorPalette`]: the popover body: a 10 × 8 swatch grid
//!   ([`TV_PALETTE`]) and an opacity slider with its percent.
//! - [`ColorSwatchButton`]: a 34 px bordered square with a 24 px swatch
//!   (dialog color fields, the Text tab's color).
//! - [`ColorBarButton`]: a 38 px floating-toolbar button, an icon with the
//!   color as a 22 × 4 bar under it.
//! - [`LinePicker`]: ONE composite button (swatch + line preview) whose
//!   popover edits color, opacity, thickness (1–4) and style.
//! - [`TextStyleBar`]: `[color] [size ▾] [B] [I]` (the Text tab).
//! - [`LineWidthButton`] / [`LineStyleButton`]: the toolbar's "— 1px" and
//!   "——" buttons with previews in their menus.
//!
//! All are stateless: the caller passes the current value and gets every
//! change through `on_change`.

use crate::button::{child_id, variant};
use crate::form::Select;
use crate::glyph::{Glyph, glyph};
use crate::menu::{Menu, MenuNav, MenuRow, MenuWidth};
use crate::slider::RangeSlider;
use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Selectable, Sizable as _, Size, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;
use tt_theme::{Theme, mix};

type ColorHandler = Rc<dyn Fn(Hsla, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type WidthHandler = Rc<dyn Fn(u8, &mut Window, &mut App)>;
type DashHandler = Rc<dyn Fn(LineDash, &mut Window, &mut App)>;
/// One segmented cell: selected, its picture (given the fg color), action.
type SegmentCell = (bool, Box<dyn Fn(Hsla) -> AnyElement>, ActionHandler);

/// TradingView's color palette (measured): a gray row, the ten base
/// colors, then six rows of tints and shades, row by row.
pub const TV_PALETTE: [u32; 80] = [
    0xffffff, 0xdbdbdb, 0xb8b8b8, 0x9c9c9c, 0x808080, 0x636363, 0x464646, 0x222222, 0x131313,
    0x000000, //
    0xf23645, 0xff9800, 0xffeb3b, 0x4caf50, 0x089981, 0x00bcd4, 0x2962ff, 0x673ab7, 0x9c27b0,
    0xe91e63, //
    0xfccbcd, 0xffe0b2, 0xfff9c4, 0xc8e6c9, 0xace5dc, 0xb2ebf2, 0xbbd9fb, 0xd1c4e9, 0xe1bee7,
    0xf8bbd0, //
    0xfaa1a4, 0xffcc80, 0xfff59d, 0xa5d6a7, 0x70ccbd, 0x80deea, 0x90bff9, 0xb39ddb, 0xce93d8,
    0xf48fb1, //
    0xf77c80, 0xffb74d, 0xfff176, 0x81c784, 0x42bda8, 0x4dd0e1, 0x5b9cf6, 0x9575cd, 0xba68c8,
    0xf06292, //
    0xf7525f, 0xffa726, 0xffee58, 0x66bb6a, 0x22ab94, 0x26c6da, 0x3179f5, 0x7e57c2, 0xab47bc,
    0xec407a, //
    0xb22833, 0xf57c00, 0xfbc02d, 0x388e3c, 0x056656, 0x0097a7, 0x1848cc, 0x512da8, 0x7b1fa2,
    0xc2185b, //
    0x801922, 0xe65100, 0xf57f17, 0x1b5e20, 0x00332a, 0x006064, 0x0c3299, 0x311b92, 0x4a148c,
    0x880e4f, //
];

/// Columns of [`TV_PALETTE`].
pub const PALETTE_COLUMNS: usize = 10;

/// `0xRRGGBB` → opaque gpui color.
pub fn rgb_hsla(c: u32) -> Hsla {
    Hsla::from(Rgba {
        r: ((c >> 16) & 0xff) as f32 / 255.0,
        g: ((c >> 8) & 0xff) as f32 / 255.0,
        b: (c & 0xff) as f32 / 255.0,
        a: 1.0,
    })
}

/// Same RGB (to the 8-bit channel), any alpha.
pub fn same_rgb(a: Hsla, b: Hsla) -> bool {
    let (a, b) = (Rgba::from(a), Rgba::from(b));
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as i32;
    q(a.r) == q(b.r) && q(a.g) == q(b.g) && q(a.b) == q(b.b)
}

/// `c` with alpha `a`.
pub fn with_alpha(c: Hsla, a: f32) -> Hsla {
    Hsla {
        a: a.clamp(0.0, 1.0),
        ..c
    }
}

/// Border of the square toggle / swatch buttons (TradingView's 1 px
/// `#555` on dark).
pub(crate) fn toggle_border(t: &Theme) -> Hsla {
    hsla(if t.dark {
        mix(t.border, t.text, 0.2)
    } else {
        mix(t.border, t.text, 0.12)
    })
}

/// A line style for the previews (the kit does not know the chart's type).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineDash {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl LineDash {
    pub const ALL: [LineDash; 3] = [LineDash::Solid, LineDash::Dashed, LineDash::Dotted];

    pub fn label(self) -> &'static str {
        match self {
            LineDash::Solid => "Solid",
            LineDash::Dashed => "Dashed",
            LineDash::Dotted => "Dotted",
        }
    }
}

/// A `len` px picture of a line: `thickness` px tall, solid, dashed
/// (5 on, 3 off) or dotted (thickness-square dots, 3 off).
pub fn line_preview(len: f32, thickness: f32, dash: LineDash, color: Hsla) -> AnyElement {
    let th = thickness.max(1.0);
    match dash {
        LineDash::Solid => div()
            .flex_none()
            .w(px(len))
            .h(px(th))
            .bg(color)
            .into_any_element(),
        LineDash::Dashed | LineDash::Dotted => {
            let (on, off) = if dash == LineDash::Dashed {
                (5.0_f32.max(th * 2.5), 3.0_f32.max(th * 1.5))
            } else {
                (th.max(2.0), 3.0_f32.max(th * 1.5))
            };
            let n = ((len + off) / (on + off)).floor().max(1.0) as usize;
            h_flex()
                .flex_none()
                .w(px(len))
                .h(px(th.max(2.0)))
                .items_center()
                .gap(px(off))
                .overflow_hidden()
                .children((0..n).map(|_| {
                    div()
                        .flex_none()
                        .w(px(on))
                        .h(px(if dash == LineDash::Dotted {
                            th.max(2.0)
                        } else {
                            th
                        }))
                        .bg(color)
                }))
                .into_any_element()
        }
    }
}

/// A swatch: the color over a checkerboard hint when translucent.
fn swatch(size: f32, radius: f32, c: Hsla, t: &Theme) -> Div {
    let base = div()
        .flex_none()
        .size(px(size))
        .rounded(px(radius))
        .overflow_hidden()
        .relative();
    let base = if c.a < 0.999 {
        // Half light / half dark under a translucent color shows opacity.
        base.bg(hsla(if t.dark { t.bg } else { t.selection }))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(size / 2.0))
                    .h(px(size / 2.0))
                    .bg(hsla(if t.dark { t.text_muted } else { t.bg })),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .right_0()
                    .w(px(size / 2.0))
                    .h(px(size / 2.0))
                    .bg(hsla(if t.dark { t.text_muted } else { t.bg })),
            )
    } else {
        base
    };
    base.child(div().absolute().inset_0().bg(c))
}

/// The palette popover's body: swatches, then "Opacity" with a slider and
/// its percent. Picking a swatch keeps the opacity.
#[derive(IntoElement)]
pub struct ColorPalette {
    id: ElementId,
    current: Hsla,
    opacity: bool,
    on_change: Option<ColorHandler>,
}

impl ColorPalette {
    pub fn new(id: impl Into<ElementId>, current: Hsla) -> Self {
        Self {
            id: id.into(),
            current,
            opacity: true,
            on_change: None,
        }
    }

    /// Hide the opacity row (opaque-only colors).
    pub fn no_opacity(mut self) -> Self {
        self.opacity = false;
        self
    }

    pub fn on_change(mut self, f: impl Fn(Hsla, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ColorPalette {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let cur = self.current;
        let mut grid = v_flex().gap(px(C_PALETTE_GAP));
        for (r, row) in TV_PALETTE.chunks(PALETTE_COLUMNS).enumerate() {
            let mut h = h_flex().gap(px(C_PALETTE_GAP));
            for (k, &c) in row.iter().enumerate() {
                let color = rgb_hsla(c);
                let selected = same_rgb(color, cur);
                let f = self.on_change.clone();
                h = h.child(
                    div()
                        .id(ElementId::NamedInteger(
                            format!("palette-sw{r}").into(),
                            k as u64,
                        ))
                        .flex_none()
                        .size(px(C_PALETTE_SWATCH))
                        .rounded(px(R_XS))
                        .bg(color)
                        .border_1()
                        .border_color(if c == 0xffffff || c == 0x000000 {
                            hsla(t.border)
                        } else {
                            color
                        })
                        .cursor_pointer()
                        .when(selected, |d| d.border_2().border_color(hsla(t.text)))
                        .hover(|s| s.opacity(0.85))
                        .on_click(move |_, w, cx| {
                            if let Some(f) = &f {
                                f(with_alpha(color, cur.a), w, cx)
                            }
                        }),
                );
            }
            grid = grid.child(h);
        }
        let pct = (cur.a * 100.0).round();
        let f = self.on_change.clone();
        let opaque = with_alpha(cur, 1.0);
        let opacity = self.opacity.then(|| {
            v_flex()
                .gap(px(S1))
                .child(
                    div()
                        .text_size(px(C_TEXT_SECTION))
                        .text_color(hsla(t.text_muted))
                        .child("Opacity"),
                )
                .child(
                    h_flex()
                        .gap(px(C_GAP))
                        .items_center()
                        .h(px(26.0))
                        .child(
                            div().flex_1().child(
                                RangeSlider::single(child_id(&self.id, "opacity"), 0.0, 100.0, pct)
                                    .width(0.0)
                                    .track_fill(linear_gradient(
                                        90.,
                                        linear_color_stop(with_alpha(opaque, 0.0), 0.),
                                        linear_color_stop(opaque, 1.),
                                    ))
                                    .on_change(move |_, v, w, cx| {
                                        if let Some(f) = &f {
                                            f(with_alpha(cur, v / 100.0), w, cx)
                                        }
                                    }),
                            ),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(px(47.0))
                                .h(px(26.0))
                                .px(px(S1_5))
                                .flex()
                                .items_center()
                                .justify_end()
                                .rounded(px(R_SM))
                                .border_1()
                                .border_color(toggle_border(&t))
                                .text_size(px(TEXT_BODY))
                                .child(format!("{pct}%")),
                        ),
                )
        });
        v_flex().gap(px(S3)).child(grid).when_some(opacity, |d, o| {
            d.child(div().h(px(1.0)).bg(hsla(t.border))).child(o)
        })
    }
}

/// The popover surface the pickers open: `overlay_bg`, radius `FT_RADIUS`,
/// 12 px padding, `C_W_POPOVER` wide.
fn picker_popover<T: Selectable + IntoElement + 'static>(
    id: ElementId,
    trigger: T,
    content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
) -> Popover {
    Popover::new(child_id(&id, "pop"))
        .anchor(Anchor::TopLeft)
        .offset(px(S1))
        .trigger(trigger)
        .content(move |_, w, cx| content(w, cx))
        .p(px(S3))
        .w(px(C_W_POPOVER))
        .rounded(px(FT_RADIUS))
}

/// A plain kit button with `content` (hover / pressed from the tokens).
fn bare_button(id: ElementId, w: f32, h: f32, radius: f32, t: &Theme, cx: &App) -> Button {
    Button::new(id)
        .custom(variant(t, hsla(t.text), cx))
        .with_size(Size::Size(px(h)))
        .rounded(px(radius))
        .w(px(w))
        .h(px(h))
        .px_0()
        .py_0()
}

/// A 34 px bordered square with a 24 px swatch; opens [`ColorPalette`].
#[derive(IntoElement)]
pub struct ColorSwatchButton {
    id: ElementId,
    color: Hsla,
    tooltip: Option<SharedString>,
    on_change: Option<ColorHandler>,
}

impl ColorSwatchButton {
    pub fn new(id: impl Into<ElementId>, color: Hsla) -> Self {
        Self {
            id: id.into(),
            color,
            tooltip: None,
            on_change: None,
        }
    }

    pub fn tooltip(mut self, s: impl Into<SharedString>) -> Self {
        self.tooltip = Some(s.into());
        self
    }

    pub fn on_change(mut self, f: impl Fn(Hsla, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ColorSwatchButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let s = C_CONTROL;
        let trigger = bare_button(child_id(&self.id, "btn"), s, s, C_R_TOGGLE, &t, cx)
            .border_1()
            .border_color(toggle_border(&t))
            .child(swatch(C_SWATCH, 3.0, self.color, &t))
            .when_some(self.tooltip, |b, tip| b.tooltip(tip));
        let (id, color, f) = (self.id.clone(), self.color, self.on_change);
        picker_popover(self.id, trigger, move |_, _| {
            let f = f.clone();
            ColorPalette::new(child_id(&id, "palette"), color)
                .on_change(move |c, w, cx| {
                    if let Some(f) = &f {
                        f(c, w, cx)
                    }
                })
                .into_any_element()
        })
    }
}

/// Floating-toolbar color button (`FT_BUTTON` square): `glyph`
/// (`FT_BAR_ICON`) with the color as a `FT_BAR_W × FT_BAR_H` bar under it;
/// opens [`ColorPalette`].
#[derive(IntoElement)]
pub struct ColorBarButton {
    id: ElementId,
    glyph: Glyph,
    color: Hsla,
    tooltip: Option<SharedString>,
    on_change: Option<ColorHandler>,
}

impl ColorBarButton {
    pub fn new(id: impl Into<ElementId>, glyph: impl Into<Glyph>, color: Hsla) -> Self {
        Self {
            id: id.into(),
            glyph: glyph.into(),
            color,
            tooltip: None,
            on_change: None,
        }
    }

    pub fn tooltip(mut self, s: impl Into<SharedString>) -> Self {
        self.tooltip = Some(s.into());
        self
    }

    pub fn on_change(mut self, f: impl Fn(Hsla, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ColorBarButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let s = FT_BUTTON;
        let content = div()
            .relative()
            .size(px(s))
            .child(
                div()
                    .absolute()
                    .top(px(7.0))
                    .left(px((s - FT_BAR_ICON) / 2.0))
                    .child(glyph(self.glyph, FT_BAR_ICON)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(27.0))
                    .left(px((s - FT_BAR_W) / 2.0))
                    .w(px(FT_BAR_W))
                    .h(px(FT_BAR_H))
                    .rounded(px(R_XS))
                    .overflow_hidden()
                    .child(swatch(FT_BAR_W, 0.0, self.color, &t).h(px(FT_BAR_H))),
            );
        let trigger = bare_button(child_id(&self.id, "btn"), s, s, 0.0, &t, cx)
            .child(content)
            .when_some(self.tooltip, |b, tip| b.tooltip(tip));
        let (id, color, f) = (self.id.clone(), self.color, self.on_change);
        picker_popover(self.id, trigger, move |_, _| {
            let f = f.clone();
            ColorPalette::new(child_id(&id, "palette"), color)
                .on_change(move |c, w, cx| {
                    if let Some(f) = &f {
                        f(c, w, cx)
                    }
                })
                .into_any_element()
        })
    }
}

/// A line's look: color (with opacity), thickness 1–4, dash.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineValue {
    pub color: Hsla,
    pub width: u8,
    pub dash: LineDash,
}

type LineHandler = Rc<dyn Fn(LineValue, &mut Window, &mut App)>;

/// Segmented options: `n` cells `C_H_SEGMENT` tall in a 1 px border,
/// outer corners radius 3; the selected cell inverted (`text` fill).
fn segmented(
    id: &ElementId,
    name: &'static str,
    cells: Vec<SegmentCell>,
    t: &Theme,
) -> impl IntoElement {
    let n = cells.len();
    let border = toggle_border(t);
    h_flex()
        .w_full()
        .h(px(C_H_SEGMENT))
        .children(cells.into_iter().enumerate().map(|(i, (sel, el, f))| {
            let fg = if sel { hsla(t.bg) } else { hsla(t.text) };
            div()
                .id(ElementId::NamedInteger(
                    format!("{id:?}-{name}").into(),
                    i as u64,
                ))
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(if sel { hsla(t.text) } else { border })
                .when(i > 0, |d| d.ml(px(-1.0)))
                .when(i == 0, |d| d.rounded_l(px(3.0)))
                .when(i + 1 == n, |d| d.rounded_r(px(3.0)))
                .when(sel, |d| d.bg(hsla(t.text)))
                .when(!sel, |d| d.hover(|s| s.bg(hsla(t.hover))))
                .cursor_pointer()
                .child(el(fg))
                .on_click(move |_, w, cx| f(w, cx))
        }))
}

/// TradingView's "Line" control: one 34 px button (24 px swatch + a 30 px
/// preview of the line) opening palette, opacity, "Thickness" and "Line
/// style".
#[derive(IntoElement)]
pub struct LinePicker {
    id: ElementId,
    value: LineValue,
    on_change: Option<LineHandler>,
}

impl LinePicker {
    pub fn new(id: impl Into<ElementId>, value: LineValue) -> Self {
        Self {
            id: id.into(),
            value,
            on_change: None,
        }
    }

    pub fn on_change(mut self, f: impl Fn(LineValue, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

/// Width of the [`LinePicker`] trigger.
pub const W_LINE_PICKER: f32 = 4.0 + C_SWATCH + C_GAP + 30.0 + 8.0;

impl RenderOnce for LinePicker {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let v = self.value;
        let trigger = bare_button(
            child_id(&self.id, "btn"),
            W_LINE_PICKER,
            C_CONTROL,
            C_R_TOGGLE,
            &t,
            cx,
        )
        .border_1()
        .border_color(toggle_border(&t))
        .child(
            h_flex()
                .w_full()
                .h_full()
                .pl(px(4.0))
                .gap(px(C_GAP))
                .items_center()
                .child(swatch(C_SWATCH, 3.0, v.color, &t))
                .child(line_preview(30.0, v.width as f32, v.dash, v.color)),
        );
        let (id, f) = (self.id.clone(), self.on_change);
        picker_popover(self.id, trigger, move |_, cx| {
            let t = cx.ui().clone();
            let emit = {
                let f = f.clone();
                move |nv: LineValue, w: &mut Window, cx: &mut App| {
                    if let Some(f) = &f {
                        f(nv, w, cx)
                    }
                }
            };
            let section = |s: &'static str| {
                div()
                    .pt(px(S1))
                    .text_size(px(C_TEXT_SECTION))
                    .text_color(hsla(t.text_muted))
                    .child(s)
            };
            let e = emit.clone();
            let widths = (1u8..=4)
                .map(|w| {
                    let e = e.clone();
                    let cell: (bool, Box<dyn Fn(Hsla) -> AnyElement>, _) = (
                        w == v.width,
                        Box::new(move |fg| line_preview(30.0, w as f32, LineDash::Solid, fg)),
                        Rc::new(move |win: &mut Window, cx: &mut App| {
                            e(LineValue { width: w, ..v }, win, cx)
                        }) as Rc<dyn Fn(&mut Window, &mut App)>,
                    );
                    cell
                })
                .collect();
            let e = emit.clone();
            let dashes = LineDash::ALL
                .into_iter()
                .map(|d| {
                    let e = e.clone();
                    let cell: (bool, Box<dyn Fn(Hsla) -> AnyElement>, _) = (
                        d == v.dash,
                        Box::new(move |fg| line_preview(30.0, 1.0, d, fg)),
                        Rc::new(move |win: &mut Window, cx: &mut App| {
                            e(LineValue { dash: d, ..v }, win, cx)
                        }) as Rc<dyn Fn(&mut Window, &mut App)>,
                    );
                    cell
                })
                .collect();
            let e = emit.clone();
            v_flex()
                .gap(px(S2))
                .child(
                    ColorPalette::new(child_id(&id, "palette"), v.color)
                        .on_change(move |c, w, cx| e(LineValue { color: c, ..v }, w, cx)),
                )
                .child(div().h(px(1.0)).bg(hsla(t.border)))
                .child(section("Thickness"))
                .child(segmented(&id, "width", widths, &t))
                .child(section("Line style"))
                .child(segmented(&id, "dash", dashes, &t))
                .into_any_element()
        })
    }
}

/// Text formatting: color, size, bold, italic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyleValue {
    pub color: Hsla,
    pub size: u8,
    pub bold: bool,
    pub italic: bool,
}

type TextStyleHandler = Rc<dyn Fn(TextStyleValue, &mut Window, &mut App)>;

/// A 34 px bordered square toggle (B / I): `text` fill when on.
#[derive(IntoElement)]
pub struct ToggleSquare {
    id: ElementId,
    label: SharedString,
    on: bool,
    bold: bool,
    italic: bool,
    tooltip: Option<SharedString>,
    on_click: Option<ActionHandler>,
}

impl ToggleSquare {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, on: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            on,
            bold: false,
            italic: false,
            tooltip: None,
            on_click: None,
        }
    }

    /// Label drawn bold ("B").
    pub fn bold_label(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Label drawn italic ("I").
    pub fn italic_label(mut self) -> Self {
        self.italic = true;
        self
    }

    pub fn tooltip(mut self, s: impl Into<SharedString>) -> Self {
        self.tooltip = Some(s.into());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ToggleSquare {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let s = C_CONTROL;
        let fg = if self.on { t.bg } else { t.text };
        let f = self.on_click;
        bare_button(self.id, s, s, C_R_TOGGLE, &t, cx)
            .border_1()
            .border_color(if self.on {
                hsla(t.text)
            } else {
                toggle_border(&t)
            })
            .when(self.on, |b| b.bg(hsla(t.text)))
            .child(
                div()
                    .text_size(px(C_TEXT_BUTTON + 2.0))
                    .text_color(hsla(fg))
                    .when(self.bold, |d| d.font_weight(FontWeight::BOLD))
                    .when(!self.bold, |d| d.font_weight(FontWeight::NORMAL))
                    .when(self.italic, |d| d.italic())
                    .child(self.label),
            )
            .when_some(self.tooltip, |b, tip| b.tooltip(tip))
            .when_some(f, |b, f| b.on_click(move |_, w, cx| f(w, cx)))
    }
}

/// The Text tab's first row: `[color] [size ▾] [B] [I]`, `C_GAP` apart.
#[derive(IntoElement)]
pub struct TextStyleBar {
    id: ElementId,
    value: TextStyleValue,
    sizes: Vec<u8>,
    on_change: Option<TextStyleHandler>,
}

impl TextStyleBar {
    pub fn new(id: impl Into<ElementId>, value: TextStyleValue, sizes: &[u8]) -> Self {
        Self {
            id: id.into(),
            value,
            sizes: sizes.to_vec(),
            on_change: None,
        }
    }

    pub fn on_change(
        mut self,
        f: impl Fn(TextStyleValue, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for TextStyleBar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let v = self.value;
        let f = self.on_change;
        let emit = move |nv: TextStyleValue, w: &mut Window, cx: &mut App| {
            if let Some(f) = &f {
                f(nv, w, cx)
            }
        };
        let (e1, e2, e3, e4) = (emit.clone(), emit.clone(), emit.clone(), emit);
        h_flex()
            .gap(px(C_GAP))
            .items_center()
            .child(
                ColorSwatchButton::new(child_id(&self.id, "color"), v.color)
                    .tooltip("Text color")
                    .on_change(move |c, w, cx| e1(TextStyleValue { color: c, ..v }, w, cx)),
            )
            .child(
                Select::new(
                    child_id(&self.id, "size"),
                    self.sizes.iter().map(|s| (*s, s.to_string())),
                    v.size,
                )
                .comfortable()
                .on_change(move |s, w, cx| e2(TextStyleValue { size: s, ..v }, w, cx)),
            )
            .child(
                ToggleSquare::new(child_id(&self.id, "bold"), "B", v.bold)
                    .bold_label()
                    .tooltip("Bold")
                    .on_click(move |w, cx| e3(TextStyleValue { bold: !v.bold, ..v }, w, cx)),
            )
            .child(
                ToggleSquare::new(child_id(&self.id, "italic"), "I", v.italic)
                    .italic_label()
                    .tooltip("Italic")
                    .on_click(move |w, cx| {
                        e4(
                            TextStyleValue {
                                italic: !v.italic,
                                ..v
                            },
                            w,
                            cx,
                        )
                    }),
            )
    }
}

/// Floating-toolbar "— 1px": a width preview and label (`FT_W_WIDTH_BUTTON`
/// wide); its menu lists 1–4 px with previews.
#[derive(IntoElement)]
pub struct LineWidthButton {
    id: ElementId,
    width: u8,
    boxed: bool,
    on_change: Option<WidthHandler>,
}

impl LineWidthButton {
    pub fn new(id: impl Into<ElementId>, width: u8) -> Self {
        Self {
            id: id.into(),
            width,
            boxed: false,
            on_change: None,
        }
    }

    /// The dialog variant (TradingView's Fib "Levels line"): a bordered
    /// `C_CONTROL` button showing only the line.
    pub fn boxed(mut self) -> Self {
        self.boxed = true;
        self
    }

    pub fn on_change(mut self, f: impl Fn(u8, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for LineWidthButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let now = self.width;
        let fg = hsla(t.text);
        let trigger = if self.boxed {
            boxed_button(child_id(&self.id, "btn"), W_BOXED_WIDTH, &t, cx).child(line_preview(
                30.0,
                now as f32,
                LineDash::Solid,
                fg,
            ))
        } else {
            bare_button(
                child_id(&self.id, "btn"),
                FT_W_WIDTH_BUTTON,
                FT_BUTTON,
                0.0,
                &t,
                cx,
            )
            .child(
                h_flex()
                    .w_full()
                    .pl(px(10.0))
                    .gap(px(7.0))
                    .items_center()
                    .child(line_preview(18.0, now as f32, LineDash::Solid, fg))
                    .child(
                        div()
                            .text_size(px(TEXT_BODY))
                            .whitespace_nowrap()
                            .child(format!("{now}px")),
                    ),
            )
        }
        .tooltip("Line width");
        let f = self.on_change;
        menu_with(self.id, trigger, window, cx, move |mut menu, cx| {
            let fg = hsla(cx.ui().text);
            menu = menu.width(MenuWidth::Sm);
            for w in 1u8..=4 {
                let f = f.clone();
                menu = menu.row(
                    MenuRow::new(("ft-width-opt", w as usize), format!("{w}px"))
                        .leading_element(move || line_preview(24.0, w as f32, LineDash::Solid, fg))
                        .checked(w == now)
                        .on_select(move |win, cx| {
                            if let Some(f) = &f {
                                f(w, win, cx)
                            }
                        }),
                );
            }
            menu
        })
    }
}

/// Floating-toolbar line style button: a 20 px preview; its menu lists
/// solid / dashed / dotted with previews.
#[derive(IntoElement)]
pub struct LineStyleButton {
    id: ElementId,
    dash: LineDash,
    boxed: bool,
    on_change: Option<DashHandler>,
}

impl LineStyleButton {
    pub fn new(id: impl Into<ElementId>, dash: LineDash) -> Self {
        Self {
            id: id.into(),
            dash,
            boxed: false,
            on_change: None,
        }
    }

    /// The dialog variant: a bordered `C_CONTROL` square.
    pub fn boxed(mut self) -> Self {
        self.boxed = true;
        self
    }

    pub fn on_change(mut self, f: impl Fn(LineDash, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for LineStyleButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let now = self.dash;
        let trigger = if self.boxed {
            boxed_button(child_id(&self.id, "btn"), C_CONTROL, &t, cx)
        } else {
            bare_button(child_id(&self.id, "btn"), FT_BUTTON, FT_BUTTON, 0.0, &t, cx)
        }
        .child(line_preview(20.0, 1.0, now, hsla(t.text)))
        .tooltip("Line style");
        let f = self.on_change;
        menu_with(self.id, trigger, window, cx, move |mut menu, cx| {
            let fg = hsla(cx.ui().text);
            menu = menu.width(MenuWidth::Sm);
            for (i, d) in LineDash::ALL.into_iter().enumerate() {
                let f = f.clone();
                menu = menu.row(
                    MenuRow::new(("ft-dash-opt", i), d.label())
                        .leading_element(move || line_preview(24.0, 1.0, d, fg))
                        .checked(d == now)
                        .on_select(move |win, cx| {
                            if let Some(f) = &f {
                                f(d, win, cx)
                            }
                        }),
                );
            }
            menu
        })
    }
}

/// Width of the boxed [`LineWidthButton`].
const W_BOXED_WIDTH: f32 = 54.0;

/// A bordered dialog button (`C_CONTROL` tall, `C_R_TOGGLE`).
fn boxed_button(id: ElementId, w: f32, t: &Theme, cx: &App) -> Button {
    bare_button(id, w, C_CONTROL, C_R_TOGGLE, t, cx)
        .border_1()
        .border_color(toggle_border(t))
}

type EndHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A line end preview: a short line with a dot (normal) or an arrow head
/// at its right / left end.
fn line_end_preview(right: bool, arrow: bool, fg: Hsla) -> AnyElement {
    let line = div().w(px(12.0)).h(px(1.0)).bg(fg);
    let end: AnyElement = if arrow {
        glyph(
            if right {
                Glyph::CHEVRON_RIGHT
            } else {
                Glyph::CHEVRON_LEFT
            },
            ICON_SM,
        )
        .color(fg)
        .into_any_element()
    } else {
        div()
            .size(px(5.0))
            .rounded_full()
            .border_1()
            .border_color(fg)
            .into_any_element()
    };
    let row = h_flex().items_center();
    if right {
        row.child(line).child(end)
    } else {
        row.child(end).child(line)
    }
    .into_any_element()
}

/// TradingView's line end select (the icons after a line picker): a
/// bordered `C_CONTROL` square previewing the end; its menu offers Normal
/// and Arrow.
#[derive(IntoElement)]
pub struct LineEndSelect {
    id: ElementId,
    right: bool,
    arrow: bool,
    on_change: Option<EndHandler>,
}

impl LineEndSelect {
    /// `right`: the line's right end (else its left end); `arrow`: the
    /// current end.
    pub fn new(id: impl Into<ElementId>, right: bool, arrow: bool) -> Self {
        Self {
            id: id.into(),
            right,
            arrow,
            on_change: None,
        }
    }

    /// The new end: `true` for an arrow.
    pub fn on_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for LineEndSelect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let (right, now) = (self.right, self.arrow);
        let trigger = boxed_button(child_id(&self.id, "btn"), C_CONTROL, &t, cx)
            .child(line_end_preview(right, now, hsla(t.text)))
            .tooltip(if right { "Right end" } else { "Left end" });
        let f = self.on_change;
        menu_with(self.id, trigger, window, cx, move |mut menu, cx| {
            let fg = hsla(cx.ui().text);
            menu = menu.width(MenuWidth::Sm);
            for (i, (arrow, label)) in [(false, "Normal"), (true, "Arrow")].into_iter().enumerate()
            {
                let f = f.clone();
                menu = menu.row(
                    MenuRow::new(("line-end-opt", i), label)
                        .leading_element(move || line_end_preview(right, arrow, fg))
                        .checked(arrow == now)
                        .on_select(move |win, cx| {
                            if let Some(f) = &f {
                                f(arrow, win, cx)
                            }
                        }),
                );
            }
            menu
        })
    }
}

/// A kit [`Menu`] below `trigger` (keyboard nav state under `id`).
pub(crate) fn menu_with(
    id: ElementId,
    trigger: Button,
    window: &mut Window,
    cx: &mut App,
    build: impl Fn(Menu, &mut App) -> Menu + 'static,
) -> AnyElement {
    let nav = MenuNav::keyed(child_id(&id, "menu-nav"), window, cx);
    let focus = nav.read(cx).focus.clone();
    Popover::new(child_id(&id, "menu-pop"))
        .anchor(Anchor::TopLeft)
        .offset(px(S1))
        .appearance(false)
        .track_focus(&focus)
        .trigger(trigger)
        .content(move |_, _, cx| {
            let pop = cx.entity();
            build(Menu::new(), cx)
                .nav(nav.clone())
                .on_dismiss(move |w, cx| pop.update(cx, |p, cx| p.dismiss(w, cx)))
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn palette_is_ten_by_eight_and_has_tv_blue() {
        assert_eq!(TV_PALETTE.len(), PALETTE_COLUMNS * 8);
        assert_eq!(TV_PALETTE[PALETTE_COLUMNS + 6], 0x2962ff);
    }

    #[::core::prelude::v1::test]
    fn same_rgb_ignores_alpha() {
        let a = rgb_hsla(0x2962ff);
        assert!(same_rgb(a, with_alpha(a, 0.3)));
        assert!(!same_rgb(a, rgb_hsla(0x2962fe)));
    }
}
