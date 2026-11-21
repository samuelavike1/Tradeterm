//! List rows, tags and empty states (§3.8).

use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;

/// A list row: leading, primary (+ secondary) text, trailing. Height
/// `H_LIST_ROW` (36) or [`ListRow::tall`] `H_LIST_ROW_TALL` (48).
#[derive(IntoElement)]
pub struct ListRow {
    id: ElementId,
    height: f32,
    leading: Option<AnyElement>,
    primary: Option<SharedString>,
    secondary: Option<SharedString>,
    trailing: Vec<AnyElement>,
    selected: bool,
    on_click: Option<crate::button::ClickHandler>,
}

impl ListRow {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            height: H_LIST_ROW,
            leading: None,
            primary: None,
            secondary: None,
            trailing: Vec::new(),
            selected: false,
            on_click: None,
        }
    }

    /// 48 px rows (symbol search results).
    pub fn tall(mut self) -> Self {
        self.height = H_LIST_ROW_TALL;
        self
    }

    pub fn leading(mut self, el: impl IntoElement) -> Self {
        self.leading = Some(el.into_any_element());
        self
    }

    /// Primary text, 14.
    pub fn primary(mut self, s: impl Into<SharedString>) -> Self {
        self.primary = Some(s.into());
        self
    }

    /// Secondary line, 12 muted.
    pub fn secondary(mut self, s: impl Into<SharedString>) -> Self {
        self.secondary = Some(s.into());
        self
    }

    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing.push(el.into_any_element());
        self
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ListRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        let group = SharedString::from(format!("list-row-{}", self.id));
        h_flex()
            .id(self.id)
            .group(group)
            .flex_none()
            .h(px(self.height))
            .px(px(S2))
            .gap(px(S2))
            .items_center()
            .rounded(px(R_MD))
            .cursor_pointer()
            .text_size(px(TEXT_BODY))
            .text_color(hsla(t.text))
            .when(self.selected, |d| d.bg(hsla(t.selection)))
            .when(!self.selected, |d| d.hover(|s| s.bg(hsla(t.hover))))
            .children(self.leading)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .when_some(self.primary, |d, p| {
                        d.child(div().whitespace_nowrap().text_ellipsis().child(p))
                    })
                    .when_some(self.secondary, |d, s| {
                        d.child(
                            div()
                                .text_size(px(TEXT_SMALL))
                                .text_color(hsla(t.text_muted))
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(s),
                        )
                    }),
            )
            .children(self.trailing)
            .when_some(self.on_click, |d, f| {
                d.on_click(move |e, w, cx| f(e, w, cx))
            })
    }
}

/// A small tag ("SPOT"): 11 px, `R_SM`, `panel_alt` bg, px 4.
#[derive(IntoElement)]
pub struct Tag(SharedString);

impl Tag {
    pub fn new(s: impl Into<SharedString>) -> Self {
        Self(s.into())
    }
}

impl RenderOnce for Tag {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        div()
            .flex_none()
            .px(px(S1))
            .rounded(px(R_SM))
            .bg(hsla(t.panel_alt))
            .text_size(px(TEXT_CAPTION))
            .font_weight(FontWeight::MEDIUM)
            .text_color(hsla(t.text_muted))
            .child(self.0)
    }
}

/// Centered muted text with `S6` padding ("This indicator was removed.").
#[derive(IntoElement)]
pub struct EmptyState(SharedString);

impl EmptyState {
    pub fn new(s: impl Into<SharedString>) -> Self {
        Self(s.into())
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .w_full()
            .p(px(S6))
            .flex()
            .justify_center()
            .text_size(px(TEXT_BODY))
            .text_color(hsla(cx.ui().text_muted))
            .child(self.0)
    }
}
