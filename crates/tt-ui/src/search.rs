//! Search-dialog pieces measured on TradingView (§2.10): the search field
//! (a borderless header or a boxed field) and filter pills.

use crate::glyph::{Glyph, glyph};
use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::h_flex;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;

/// A search field: magnifier (20 in a 28 box), 16 px query text.
///
/// - [`SearchField::header`]: TradingView's symbol search, no box, a
///   "Clear" text button at the right while there is text.
/// - [`SearchField::boxed`]: the indicator picker's, a 40 tall field with
///   a 1 px border and radius 8, no focus ring, a × to clear.
#[derive(IntoElement)]
pub struct SearchField {
    state: Entity<InputState>,
    boxed: bool,
}

impl SearchField {
    pub fn header(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            boxed: false,
        }
    }

    pub fn boxed(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            boxed: true,
        }
    }
}

impl RenderOnce for SearchField {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let muted = hsla(t.text_muted);
        let icon = div()
            .flex_none()
            .size(px(SEARCH_ICON_BOX))
            .flex()
            .items_center()
            .justify_center()
            .child(glyph(Glyph::SEARCH, SEARCH_ICON).color(muted));
        if self.boxed {
            return Input::new(&self.state)
                .prefix(icon)
                .cleanable(true)
                .focus_bordered(false)
                .map(|i| Styled::h(i, px(SEARCH_H_FIELD)))
                .pl(px(S2))
                .rounded(px(C_R_CONTROL))
                .border_color(hsla(t.border))
                .text_size(px(SEARCH_TEXT))
                .into_any_element();
        }
        let empty = self.state.read(cx).value().is_empty();
        let state = self.state.clone();
        let hover = hsla(t.hover);
        h_flex()
            .w_full()
            .h(px(SEARCH_H_FIELD))
            .items_center()
            .gap(px(S1))
            .child(icon)
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.state)
                        .appearance(false)
                        .text_size(px(SEARCH_TEXT))
                        .px(px(S1)),
                ),
            )
            .when(!empty, |d| {
                d.child(
                    div()
                        .id("search-clear")
                        .flex_none()
                        .h(px(SEARCH_H_CLEAR))
                        .px(px(7.0))
                        .flex()
                        .items_center()
                        .rounded(px(C_R_CONTROL))
                        .cursor_pointer()
                        .text_size(px(TEXT_BODY))
                        .text_color(muted)
                        .hover(move |s| s.bg(hover))
                        .child("Clear")
                        .on_click(move |_, window, cx| {
                            state.update(cx, |s, cx| {
                                s.set_value("", window, cx);
                                s.focus(window, cx);
                            });
                        }),
                )
            })
            .into_any_element()
    }
}

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A filter pill: 28 tall, fully round, 14 px. Off: `panel_alt`; on:
/// inverted (text-colored fill), optionally with a "| ×" part that clears
/// the filter, like TradingView's "Crypto ×".
#[derive(IntoElement)]
pub struct FilterPill {
    id: ElementId,
    label: SharedString,
    selected: bool,
    on_click: Option<Handler>,
    on_clear: Option<Handler>,
}

impl FilterPill {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected: false,
            on_click: None,
            on_clear: None,
        }
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    /// Shown while selected: a × after the label that clears the filter.
    pub fn on_clear(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for FilterPill {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        let (bg, fg, hover) = if self.selected {
            let mut h = hsla(t.text);
            h.l = if t.dark { h.l - 0.08 } else { h.l + 0.12 };
            (hsla(t.text), hsla(t.bg), h)
        } else {
            (hsla(t.panel_alt), hsla(t.text), hsla(t.selection))
        };
        let clear = self.on_clear.filter(|_| self.selected);
        let mut sep = fg;
        sep.a = 0.35;
        let label = div()
            .id(self.id.clone())
            .h_full()
            .pl(px(PILL_PAD_X))
            .pr(px(if clear.is_some() { S2 } else { PILL_PAD_X }))
            .flex()
            .items_center()
            .cursor_pointer()
            .rounded_l(px(R_FULL))
            .when(clear.is_none(), |d| d.rounded_r(px(R_FULL)))
            .hover(move |s| s.bg(hover))
            .child(self.label)
            .when_some(self.on_click, |d, f| {
                d.on_click(move |e, w, cx| f(e, w, cx))
            });
        h_flex()
            .flex_none()
            .h(px(PILL_H))
            .rounded(px(R_FULL))
            .bg(bg)
            .text_color(fg)
            .text_size(px(TEXT_BODY))
            .whitespace_nowrap()
            .child(label)
            .when_some(clear, |d, f| {
                let id = ElementId::NamedChild(self.id.clone().into(), "clear".into());
                d.child(div().w(px(1.0)).h(px(S4)).bg(sep)).child(
                    div()
                        .id(id)
                        .h_full()
                        .w(px(PILL_H))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .rounded_r(px(R_FULL))
                        .hover(move |s| s.bg(hover))
                        .child(glyph(Glyph::X, ICON_XS).color(fg))
                        .on_click(move |e, w, cx| f(e, w, cx)),
                )
            })
    }
}
