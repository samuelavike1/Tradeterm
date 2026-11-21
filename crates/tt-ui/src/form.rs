//! Form controls (§3.7). Every control is `H_FIELD` (28) tall.
//!
//! Binary options in dialogs are [`FormRow::check`] (TradingView's
//! checkboxes); toolbar toggles are `IconButton::toggled`; `Switch` is not
//! used.

use crate::button::child_id;
use crate::glyph::{Glyph, glyph};
use crate::menu::{Menu, MenuNav, MenuRow, MenuWidth};
use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerState};
use gpui_kit::component::input::{Input, InputState, NumberInput};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Sizable as _, Size, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;

/// The shared color palette (TradingView-like): grays, then the series
/// colors and their light / dark variants, row by row.
pub const PALETTE: [u32; 40] = [
    0xffffff, 0xd1d4dc, 0xb2b5be, 0x9598a1, 0x787b86, 0x5d606b, 0x434651, 0x2a2e39, //
    0xf23645, 0xff9800, 0xffeb3b, 0x4caf50, 0x089981, 0x00bcd4, 0x2962ff, 0x673ab7, //
    0xfccbcd, 0xffe0b2, 0xfff9c4, 0xc8e6c9, 0xace5dc, 0xb2ebf2, 0xbbd9fb, 0xd1c4e9, //
    0xf77c80, 0xffb74d, 0xfff176, 0x81c784, 0x42bda8, 0x4dd0e1, 0x5b9cf6, 0x9575cd, //
    0xb22833, 0xf57c00, 0xfbc02d, 0x388e3c, 0x056656, 0x0097a7, 0x1848cc, 0x512da8, //
];

/// [`PALETTE`] as gpui colors (for `ColorPicker::featured_colors`).
pub fn palette() -> Vec<Hsla> {
    PALETTE
        .iter()
        .map(|&c| {
            hsla(tt_theme::Color32::from_rgb(
                (c >> 16) as u8,
                (c >> 8) as u8,
                c as u8,
            ))
        })
        .collect()
}

type CheckHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type ChangeHandler<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

/// A form row: label column + controls (h ≥ 36, label 14 in `text`, one
/// line with ellipsis).
#[derive(IntoElement)]
pub struct FormRow {
    label: SharedString,
    label_width: f32,
    check: Option<(ElementId, bool, CheckHandler)>,
    controls: Vec<AnyElement>,
    unit: Option<SharedString>,
    indent: bool,
    comfortable: bool,
    label_auto: bool,
}

impl FormRow {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            label_width: W_LABEL_SM,
            check: None,
            controls: Vec::new(),
            unit: None,
            indent: false,
            comfortable: false,
            label_auto: false,
        }
    }

    /// TradingView-measured density (section 2.10): rows `C_ROW` (50) tall,
    /// controls `C_GAP` apart, the 18 px [`CheckBox`].
    pub fn comfortable(mut self) -> Self {
        self.comfortable = true;
        self
    }

    /// The label column hugs the label, `C_LABEL_GAP` before the controls
    /// (like TradingView's table cells).
    pub fn label_auto(mut self) -> Self {
        self.label_auto = true;
        self
    }

    /// Checkbox + label in the label column; extra controls align to the
    /// control column.
    pub fn check(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        checked: bool,
        on: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        let mut r = Self::new(label);
        r.check = Some((id.into(), checked, Rc::new(on)));
        r
    }

    /// Default `W_LABEL_SM` (160); Lg dialogs use `W_LABEL_LG` (220), see
    /// `DialogSize::label_width`.
    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = w;
        self
    }

    /// A control (gap `S3` between controls).
    pub fn control(mut self, c: impl IntoElement) -> Self {
        self.controls.push(c.into_any_element());
        self
    }

    /// Trailing muted unit ("px", "bars").
    pub fn unit(mut self, u: impl Into<SharedString>) -> Self {
        self.unit = Some(u.into());
        self
    }

    /// Nested option (+24 left).
    pub fn indent(mut self) -> Self {
        self.indent = true;
        self
    }
}

impl RenderOnce for FormRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        let indent = if self.indent { S6 } else { 0.0 };
        let label_w = (self.label_width - indent).max(0.0);
        if self.comfortable {
            let label: AnyElement = match self.check {
                Some((id, checked, on)) => CheckBox::new(id, self.label, checked)
                    .on_change(move |v, w, cx| on(v, w, cx))
                    .into_any_element(),
                None => div()
                    .whitespace_nowrap()
                    .child(self.label)
                    .into_any_element(),
            };
            let lone = self.controls.is_empty() && self.unit.is_none();
            return h_flex()
                .min_h(px(C_ROW))
                .pl(px(indent))
                .items_center()
                .text_size(px(TEXT_BODY))
                .text_color(hsla(t.text))
                .child(if lone {
                    div().flex_1().whitespace_nowrap().child(label)
                } else if self.label_auto {
                    div().flex_none().pr(px(C_LABEL_GAP)).child(label)
                } else {
                    div().flex_none().w(px(label_w)).child(label)
                })
                .child(
                    h_flex()
                        .gap(px(C_GAP))
                        .items_center()
                        .children(self.controls)
                        .when_some(self.unit, |d, u| {
                            d.child(div().text_color(hsla(t.text_muted)).child(u))
                        }),
                )
                .into_any_element();
        }
        let label: AnyElement = match self.check {
            Some((id, checked, on)) => Checkbox::new(id)
                .checked(checked)
                .label(self.label)
                .on_click(move |v, w, cx| on(*v, w, cx))
                .into_any_element(),
            None => div()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(self.label)
                .into_any_element(),
        };
        h_flex()
            .min_h(px(H_LIST_ROW))
            .pl(px(indent))
            .gap(px(S3))
            .items_center()
            .text_size(px(TEXT_BODY))
            .text_color(hsla(t.text))
            // A row without controls (a lone checkbox) lets its label use
            // the whole row, so long labels stay on one line.
            .child(if self.controls.is_empty() && self.unit.is_none() {
                div()
                    .flex_1()
                    .min_w(px(label_w))
                    .whitespace_nowrap()
                    .child(label)
            } else {
                div().flex_none().w(px(label_w)).child(label)
            })
            .children(self.controls)
            .when_some(self.unit, |d, u| {
                d.child(div().text_color(hsla(t.text_muted)).child(u))
            })
            .into_any_element()
    }
}

/// TradingView's checkbox (section 2.10): an 18 px box (radius 3; checked:
/// `text` fill with a check in `bg`), 8 px to its 14 px label; the whole
/// row clicks.
#[derive(IntoElement)]
pub struct CheckBox {
    id: ElementId,
    label: SharedString,
    checked: bool,
    on_change: Option<CheckHandler>,
}

impl CheckBox {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, checked: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked,
            on_change: None,
        }
    }

    /// The new state after a click.
    pub fn on_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for CheckBox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        let on = self.checked;
        let f = self.on_change;
        h_flex()
            .id(self.id)
            .gap(px(C_GAP))
            .items_center()
            .cursor_pointer()
            .text_size(px(TEXT_BODY))
            .text_color(hsla(t.text))
            .child(
                div()
                    .flex_none()
                    .size(px(C_CHECKBOX))
                    .rounded(px(3.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(on, |d| {
                        d.bg(hsla(t.text))
                            .child(glyph(Glyph::CHECK, ICON_XS).color(hsla(t.bg)))
                    })
                    .when(!on, |d| {
                        d.border_1()
                            .border_color(crate::picker::toggle_border(t))
                            .bg(hsla(t.bg))
                    }),
            )
            .when(!self.label.is_empty(), |d| {
                d.child(div().whitespace_nowrap().child(self.label))
            })
            .when_some(f, |d, f| d.on_click(move |_, w, cx| f(!on, w, cx)))
    }
}

/// A plain input on the comfortable density: `C_W_FIELD` x `C_CONTROL`,
/// radius `C_R_CONTROL`, text 14 (numbers in dialogs, Visibility min / max).
#[derive(IntoElement)]
pub struct Field {
    input: Entity<InputState>,
    width: f32,
}

impl Field {
    pub fn new(input: &Entity<InputState>) -> Self {
        Self {
            input: input.clone(),
            width: C_W_FIELD,
        }
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
}

impl RenderOnce for Field {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().flex_none().w(px(self.width)).child(
            Input::new(&self.input)
                // `Input::h` sizes multi-line inputs only; the style height
                // wins over the size class (it is refined last).
                .map(|i| Styled::h(i, px(C_CONTROL)))
                .rounded(px(C_R_CONTROL)),
        )
    }
}

/// A number input, `W_FIELD_NUMBER` wide, `H_FIELD` tall.
#[derive(IntoElement)]
pub struct NumberField {
    input: Entity<InputState>,
    width: f32,
}

impl NumberField {
    pub fn new(input: &Entity<InputState>) -> Self {
        Self {
            input: input.clone(),
            width: W_FIELD_NUMBER,
        }
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
}

impl RenderOnce for NumberField {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .flex_none()
            .w(px(self.width))
            .child(NumberInput::new(&self.input).small().h(px(H_FIELD)))
    }
}

/// A dropdown of fixed options: outline trigger (h 28, `W_FIELD_SELECT`)
/// and a kit [`Menu`] with ✓ on the current option.
#[derive(IntoElement)]
pub struct Select<T: PartialEq + Clone + 'static> {
    id: ElementId,
    options: Vec<(T, SharedString)>,
    current: T,
    width: f32,
    comfortable: bool,
    on_change: Option<ChangeHandler<T>>,
}

impl<T: PartialEq + Clone + 'static> Select<T> {
    pub fn new(
        id: impl Into<ElementId>,
        options: impl IntoIterator<Item = (T, impl Into<SharedString>)>,
        current: T,
    ) -> Self {
        Self {
            id: id.into(),
            options: options.into_iter().map(|(v, l)| (v, l.into())).collect(),
            current,
            width: W_FIELD_SELECT,
            comfortable: false,
            on_change: None,
        }
    }

    /// Section 2.10: `C_W_FIELD` x `C_CONTROL`, radius `C_R_CONTROL`,
    /// chevron 18.
    pub fn comfortable(mut self) -> Self {
        self.comfortable = true;
        self.width = C_W_FIELD;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    pub fn on_change(mut self, f: impl Fn(T, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl<T: PartialEq + Clone + 'static> RenderOnce for Select<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let id = self.id.clone();
        let nav = MenuNav::keyed(child_id(&id, "select-nav"), window, cx);
        let focus = nav.read(cx).focus.clone();
        let label = self
            .options
            .iter()
            .find(|(v, _)| *v == self.current)
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        let (h, chevron) = if self.comfortable {
            (C_CONTROL, ICON_MD)
        } else {
            (H_FIELD, ICON_XS)
        };
        let trigger = Button::new(child_id(&id, "select-trigger"))
            .outline()
            .with_size(Size::Size(px(h)))
            .h(px(h))
            .w(px(self.width))
            .px(px(S2))
            .py_0()
            .when(self.comfortable, |b| b.rounded(px(C_R_CONTROL)).pl(px(S3)))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap(px(S1_5))
                    .text_size(px(TEXT_BODY))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(label),
                    )
                    .child(glyph(Glyph::CHEVRON_DOWN, chevron).color(hsla(t.text_muted))),
            );
        let options = Rc::new(self.options);
        let current = self.current;
        let on_change = self.on_change;
        let width = self.width;
        Popover::new(child_id(&id, "select-pop"))
            .anchor(Anchor::TopLeft)
            .offset(px(S1))
            .appearance(false)
            .track_focus(&focus)
            .trigger(trigger)
            .content(move |_, _, cx| {
                let pop = cx.entity();
                let mut menu = Menu::new()
                    .width(if width > W_MENU_SM {
                        MenuWidth::Md
                    } else {
                        MenuWidth::Sm
                    })
                    .nav(nav.clone())
                    .on_dismiss(move |w, cx| pop.update(cx, |p, cx| p.dismiss(w, cx)));
                for (i, (value, label)) in options.iter().enumerate() {
                    let (value, f) = (value.clone(), on_change.clone());
                    menu = menu.row(
                        MenuRow::new(("select-opt", i), label.clone())
                            .checked(value == current)
                            .on_select(move |w, cx| {
                                if let Some(f) = &f {
                                    f(value.clone(), w, cx)
                                }
                            }),
                    );
                }
                menu
            })
    }
}

/// A color swatch trigger (16 swatch in a 24 control, `R_SM`, 1 px border)
/// with gpui-component's palette popover (featured [`PALETTE`], hue /
/// saturation / lightness / **opacity** sliders, hex input).
#[derive(IntoElement)]
pub struct ColorField {
    state: Entity<ColorPickerState>,
    compact: bool,
}

impl ColorField {
    pub fn new(state: &Entity<ColorPickerState>) -> Self {
        Self {
            state: state.clone(),
            compact: false,
        }
    }

    /// Floating-toolbar variant (20 px trigger, same popover).
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}

impl RenderOnce for ColorField {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        ColorPicker::new(&self.state)
            .featured_colors(palette())
            .map(|p| if self.compact { p.xsmall() } else { p.small() })
    }
}
