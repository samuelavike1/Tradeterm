//! List rows, tags and empty states (§3.8).

use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;

/// Row text: a plain string, or a rich element (highlighted match).
enum Text {
    Plain(SharedString),
    Rich(AnyElement),
}

impl Text {
    fn into_el(self) -> AnyElement {
        match self {
            Text::Plain(s) => div()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(s)
                .into_any_element(),
            Text::Rich(e) => e,
        }
    }
}

/// A list row: leading, primary (+ secondary) text, trailing. Height
/// `H_LIST_ROW` (36) or [`ListRow::tall`] `H_LIST_ROW_TALL` (48).
#[derive(IntoElement)]
pub struct ListRow {
    id: ElementId,
    height: f32,
    leading: Option<AnyElement>,
    primary: Option<Text>,
    secondary: Option<Text>,
    trailing: Vec<AnyElement>,
    selected: bool,
    large: bool,
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
            large: false,
            on_click: None,
        }
    }

    /// 48 px rows.
    pub fn tall(mut self) -> Self {
        self.height = H_LIST_ROW_TALL;
        self
    }

    /// TradingView's symbol search result: 58 tall, radius 8, 16 from the
    /// leading logo (`LOGO_LG`) to the text, title 16 / 24, second line
    /// 13 / 18 muted. Put the rows in a container padded `LIST_LG_INSET`
    /// so the fill sits 12 and the content 20 from the dialog edge.
    pub fn large(mut self) -> Self {
        self.height = H_LIST_ROW_LG;
        self.large = true;
        self
    }

    pub fn leading(mut self, el: impl IntoElement) -> Self {
        self.leading = Some(el.into_any_element());
        self
    }

    /// Primary text, 14.
    pub fn primary(mut self, s: impl Into<SharedString>) -> Self {
        self.primary = Some(Text::Plain(s.into()));
        self
    }

    /// Rich primary line (e.g. a `StyledText` with the match highlighted).
    /// It inherits the row's text 14 / `text`; keep it on one line
    /// (`whitespace_nowrap`).
    pub fn primary_el(mut self, el: impl IntoElement) -> Self {
        self.primary = Some(Text::Rich(el.into_any_element()));
        self
    }

    /// Secondary line, 12 muted.
    pub fn secondary(mut self, s: impl Into<SharedString>) -> Self {
        self.secondary = Some(Text::Plain(s.into()));
        self
    }

    /// Rich secondary line; inherits 12 / `text_muted`.
    pub fn secondary_el(mut self, el: impl IntoElement) -> Self {
        self.secondary = Some(Text::Rich(el.into_any_element()));
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
        let large = self.large;
        let (secondary_size, primary_line, secondary_line) = if large {
            (
                TEXT_ROW_LG_SECONDARY,
                Some(px(LINE_ROW_LG)),
                Some(px(LINE_ROW_LG_SECONDARY)),
            )
        } else {
            (TEXT_SMALL, None, None)
        };
        h_flex()
            .id(self.id)
            .group(group)
            .flex_none()
            .h(px(self.height))
            .px(px(S2))
            .gap(px(if large { S4 } else { S2 }))
            .items_center()
            .rounded(px(if large { C_R_CONTROL } else { R_MD }))
            .cursor_pointer()
            .text_size(px(if large { TEXT_ROW_LG } else { TEXT_BODY }))
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
                        d.child(
                            div()
                                .when_some(primary_line, |d, l| d.line_height(l))
                                .child(p.into_el()),
                        )
                    })
                    .when_some(self.secondary, |d, s| {
                        d.child(
                            div()
                                .text_size(px(secondary_size))
                                .when_some(secondary_line, |d, l| d.line_height(l))
                                .text_color(hsla(t.text_muted))
                                .child(s.into_el()),
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
pub struct Tag(SharedString, bool);

impl Tag {
    pub fn new(s: impl Into<SharedString>) -> Self {
        Self(s.into(), false)
    }

    /// TradingView's type badge ("SPOT CRYPTO", "FUTURES"): 8 px bold
    /// uppercase on a 12 tall, radius 3 chip of the text color at 12 %.
    pub fn badge(s: impl Into<SharedString>) -> Self {
        Self(s.into(), true)
    }
}

impl RenderOnce for Tag {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        if self.1 {
            let mut bg = hsla(t.text);
            bg.a = 0.12;
            return div()
                .flex_none()
                .h(px(H_BADGE))
                .px(px(S1_5))
                .flex()
                .items_center()
                .rounded(px(R_BADGE))
                .bg(bg)
                .text_size(px(TEXT_BADGE))
                .line_height(px(H_BADGE))
                .font_weight(FontWeight::BOLD)
                .text_color(hsla(t.text))
                .child(SharedString::from(self.0.to_uppercase()));
        }
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

/// Keyboard selection over a list of `len` rows (§3.8), kept in view
/// through [`ListNav::scroll`]: track it on the list container, whose child
/// `i` must be row `i`.
///
/// Moves clamp at the ends. From no selection, Down / PageDown start at the
/// top and Up / PageUp at the bottom. Home / End act only once a row is
/// selected, so a search field next to the list keeps them for its caret.
pub struct ListNav {
    selected: Option<usize>,
    pub scroll: ScrollHandle,
    page: usize,
}

/// What [`ListNav::key`] did with a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKey {
    /// Not a navigation key: let it through.
    Ignored,
    /// A navigation key; the selection is now this (it may be unchanged).
    Moved {
        from: Option<usize>,
        to: Option<usize>,
    },
    /// Enter, with the selected row (if any).
    Activate(Option<usize>),
}

impl ListKey {
    /// The key was a navigation key (consume it), Enter excluded.
    pub fn is_nav(self) -> bool {
        matches!(self, ListKey::Moved { .. })
    }

    /// The selection changed.
    pub fn changed(self) -> bool {
        matches!(self, ListKey::Moved { from, to } if from != to)
    }
}

impl Default for ListNav {
    fn default() -> Self {
        Self::new()
    }
}

impl ListNav {
    /// PageUp / PageDown move about one screen of tall rows.
    pub const DEFAULT_PAGE: usize = 7;

    pub fn new() -> Self {
        Self {
            selected: None,
            scroll: ScrollHandle::new(),
            page: Self::DEFAULT_PAGE,
        }
    }

    /// Rows a PageUp / PageDown moves.
    pub fn page(mut self, rows: usize) -> Self {
        self.page = rows.max(1);
        self
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Selects `row` (or nothing) and scrolls it into view.
    pub fn select(&mut self, row: Option<usize>) {
        self.selected = row;
        if let Some(i) = row {
            self.scroll.scroll_to_item(i);
        }
    }

    /// Clears the selection (the rows changed).
    pub fn reset(&mut self) {
        self.selected = None;
    }

    /// Applies a key (`Keystroke::key`: "up", "down", "pageup", "pagedown",
    /// "home", "end", "enter") to a list of `len` rows.
    pub fn key(&mut self, key: &str, len: usize) -> ListKey {
        let from = self.selected;
        if key == "enter" {
            return ListKey::Activate(from.filter(|&i| i < len));
        }
        let last = len.saturating_sub(1);
        let next = match (key, from) {
            ("down" | "pagedown", None) => 0,
            ("up" | "pageup", None) => last,
            ("down", Some(i)) => (i + 1).min(last),
            ("up", Some(i)) => i.saturating_sub(1),
            ("pagedown", Some(i)) => (i + self.page).min(last),
            ("pageup", Some(i)) => i.saturating_sub(self.page),
            ("home", Some(_)) => 0,
            ("end", Some(_)) => last,
            _ => return ListKey::Ignored,
        };
        if len > 0 {
            self.select(Some(next));
        }
        ListKey::Moved {
            from,
            to: self.selected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn list_nav_moves_and_clamps() {
        let mut n = ListNav::new().page(3);
        // With nothing highlighted, Home / End stay with the text field
        // (they move its caret).
        assert_eq!(n.key("home", 10), ListKey::Ignored);
        assert_eq!(n.key("end", 10), ListKey::Ignored);
        assert_eq!(
            n.key("down", 10),
            ListKey::Moved {
                from: None,
                to: Some(0)
            }
        );
        assert!(n.key("pagedown", 10).changed());
        assert_eq!(n.selected(), Some(3));
        n.key("end", 10);
        assert_eq!(n.selected(), Some(9));
        assert!(!n.key("down", 10).changed());
        assert_eq!(n.key("enter", 10), ListKey::Activate(Some(9)));
        assert_eq!(n.key("a", 10), ListKey::Ignored);
        n.reset();
        n.key("up", 10);
        assert_eq!(n.selected(), Some(9));
        let mut e = ListNav::new();
        assert!(e.key("down", 0).is_nav());
        assert_eq!(e.selected(), None);
        assert_eq!(e.key("enter", 0), ListKey::Activate(None));
    }
}
