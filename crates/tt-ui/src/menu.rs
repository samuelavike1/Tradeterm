//! One menu system for every dropdown (§3.4): kit-owned 32 px rows on a
//! gpui-component `Popover` (anchoring, outside-click and Escape dismissal).
//!
//! Keyboard: ↑/↓ move, Home/End, PageUp/PageDown, Enter selects, Esc
//! closes, typing a letter jumps to the next row starting with it. Pointer
//! hover moves the same highlight, so there is only ever one highlighted row.
//!
//! A row click closes the menu (dismiss first, then the handler, so a dialog
//! opened by the handler keeps its focus); `keep_open()` opts out (stars,
//! collapse toggles).
//!
//! The current item of a pick-one menu is either `checked` (✓ column and
//! `selection` background) or, TradingView style, `current` (inverted row,
//! no ✓). `Menu::large()` gives TradingView's 40 px rows with 28 px icons.

use crate::button::{ControlSize, IconButton, child_id};
use crate::divider::Divider;
use crate::glyph::{Glyph, glyph};
use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use crate::tooltip::{format_keys, tip};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;
use tt_theme::{Theme, with_alpha};

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Menu widths (§2.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MenuWidth {
    /// 200
    Sm,
    /// 240
    #[default]
    Md,
    /// 280, only with a shortcut column or large rows.
    Lg,
}

impl MenuWidth {
    pub fn px(self) -> f32 {
        match self {
            MenuWidth::Sm => W_MENU_SM,
            MenuWidth::Md => W_MENU_MD,
            MenuWidth::Lg => W_MENU_LG,
        }
    }
}

/// Keyboard focus, highlighted row and scroll position of one menu. Kept in
/// keyed element state by the menu's owner (trigger), so it survives the
/// menu being rebuilt every frame.
pub struct MenuNav {
    pub focus: FocusHandle,
    highlighted: Option<usize>,
    scroll: ScrollHandle,
}

impl MenuNav {
    /// The nav state stored under `id` in the current element.
    pub fn keyed(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Entity<MenuNav> {
        window.use_keyed_state(id.into(), cx, |_, cx| MenuNav {
            focus: cx.focus_handle(),
            highlighted: None,
            scroll: ScrollHandle::new(),
        })
    }

    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }
}

/// One menu row (§3.4): h 32, `R_MD`, text 14, icon 16.
#[derive(IntoElement)]
pub struct MenuRow {
    id: ElementId,
    label: SharedString,
    glyph: Option<Glyph>,
    leading: Option<Rc<dyn Fn() -> AnyElement>>,
    description: Option<SharedString>,
    shortcut: Option<SharedString>,
    checked: Option<bool>,
    /// `checked` is an on/off setting: ✓ only, no `selection` background.
    toggle: bool,
    /// The current item, TradingView style: inverted row, no ✓.
    current: bool,
    favorite: Option<(bool, Handler)>,
    removable: Option<(SharedString, Handler)>,
    danger: bool,
    disabled: bool,
    keep_open: bool,
    on_select: Option<Handler>,
}

impl MenuRow {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            glyph: None,
            leading: None,
            description: None,
            shortcut: None,
            checked: None,
            toggle: false,
            current: false,
            favorite: None,
            removable: None,
            danger: false,
            disabled: false,
            keep_open: false,
            on_select: None,
        }
    }

    /// A destructive row ("Remove 3 drawings"): `danger` fg and hover tint.
    pub fn danger(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        let mut r = Self::new(id, label);
        r.danger = true;
        r
    }

    /// Leading icon (16, muted unless highlighted).
    pub fn icon(mut self, g: impl Into<Glyph>) -> Self {
        self.glyph = Some(g.into());
        self
    }

    /// A leading element before the label (line previews in the floating
    /// toolbar's width / style menus).
    pub fn leading_element(mut self, f: impl Fn() -> AnyElement + 'static) -> Self {
        self.leading = Some(Rc::new(f));
        self
    }

    /// The current item: leading ✓ (the menu reserves the column when any
    /// row is checkable) and `selection` background.
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = Some(on);
        self
    }

    /// The current item of a pick-one menu, TradingView style: the row is
    /// inverted (`text` fill, content in the surface color; a dark row in
    /// light theme, a light row in dark theme) and stays so on hover. No ✓
    /// and no check column. Used by the chart-type and interval menus and
    /// the drawing flyouts.
    pub fn current(mut self, on: bool) -> Self {
        self.current = on;
        self
    }

    /// An on/off setting among others ("Invert scale"): a leading ✓ when
    /// on, in the same column as [`MenuRow::checked`], but no `selection`
    /// background (several can be on at once).
    pub fn toggle(mut self, on: bool) -> Self {
        self.checked = Some(on);
        self.toggle = true;
        self
    }

    /// Right-aligned shortcut hint, any form (`"alt-t"`, `"Alt + T"`),
    /// shown as `Alt+T`.
    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    /// Shortcut text from the key bound to `action` (in `context`).
    pub fn shortcut_action(
        mut self,
        action: &dyn Action,
        context: Option<&str>,
        window: &Window,
    ) -> Self {
        let binding = match context {
            Some(c) => KeyContext::parse(c).ok().and_then(|ctx| {
                window.highest_precedence_binding_for_action_in_context(action, ctx)
            }),
            None => window.highest_precedence_binding_for_action(action),
        };
        if let Some(k) =
            binding.and_then(|b| b.keystrokes().first().map(|k| k.as_keystroke().clone()))
        {
            self.shortcut = Some(gpui_kit::component::kbd::Kbd::format(&k).into());
        }
        self
    }

    /// Tooltip after the standard delay.
    pub fn description(mut self, d: impl Into<SharedString>) -> Self {
        self.description = Some(d.into());
        self
    }

    /// Trailing star: visible on hover or when on. Clicking it keeps the
    /// menu open.
    pub fn favorite(mut self, on: bool, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.favorite = Some((on, Rc::new(f)));
        self
    }

    /// Trailing × with tooltip `tip` (custom intervals).
    pub fn removable(
        mut self,
        tip: impl Into<SharedString>,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.removable = Some((tip.into(), Rc::new(f)));
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Clicking the row does not close the menu.
    pub fn keep_open(mut self) -> Self {
        self.keep_open = true;
        self
    }

    pub fn on_select(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    fn render_row(
        self,
        index: usize,
        nav: Option<&Entity<MenuNav>>,
        highlighted: bool,
        (check_col, large): (bool, bool),
        dismiss: Option<Handler>,
        t: &Theme,
    ) -> Stateful<Div> {
        // TradingView's current row: inverted, also while hovered.
        let inverted = self.current && !self.disabled;
        let fg = if self.disabled {
            t.text_disabled
        } else if inverted {
            t.overlay_bg
        } else if self.danger {
            t.danger
        } else {
            t.text
        };
        let muted = if inverted {
            with_alpha(t.overlay_bg, 0.7)
        } else {
            t.text_muted
        };
        let hover_bg = if inverted {
            t.text
        } else if self.danger {
            with_alpha(t.danger, 0.12)
        } else {
            t.hover
        };
        let current = self.checked == Some(true);
        let current_bg = current && !self.toggle;
        let group = SharedString::from(format!("menu-row-{index}"));
        let interactive = !self.disabled;
        let keep_open = self.keep_open;
        let on_select = self.on_select.clone();
        let id = self.id.clone();
        // Large rows: 40 tall, the icon 4 px from the edge, the label 4 px
        // after it (TradingView-measured).
        let (row_h, pad, gap, icon) = if large {
            (H_MENU_ROW_LG, S1, S1, ICON_LG)
        } else {
            (H_MENU_ROW, S2, S2, ICON_SM)
        };
        h_flex()
            .id(self.id)
            .group(group.clone())
            .flex_none()
            .h(px(row_h))
            .px(px(pad))
            .gap(px(gap))
            .items_center()
            .rounded(px(R_MD))
            .text_size(px(TEXT_BODY))
            .text_color(hsla(fg))
            .when(current_bg, |d| d.bg(hsla(t.selection)))
            .when(inverted, |d| d.bg(hsla(t.text)))
            .when(interactive && highlighted, |d| d.bg(hsla(hover_bg)))
            .when(interactive && nav.is_none(), |d| {
                d.hover(|s| s.bg(hsla(hover_bg)))
            })
            .when(check_col, |d| {
                d.child(
                    div()
                        .flex_none()
                        .size(px(ICON_SM))
                        .when(current, |d| d.child(glyph(Glyph::CHECK, ICON_SM))),
                )
            })
            .when_some(self.glyph, |d, g| {
                // SVG icons keep the label color (TradingView); font glyphs
                // are muted unless highlighted.
                let dim = !highlighted && interactive && !inverted && !g.is_svg();
                d.child(glyph(g, g.svg_size(icon)).when(dim, |g| {
                    g.color(hsla(if self.danger { t.danger } else { t.text_muted }))
                }))
            })
            .when_some(self.leading, |d, f| d.child(div().flex_none().child(f())))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(self.label),
            )
            .when_some(self.shortcut, |d, k| {
                d.child(
                    div()
                        .flex_none()
                        .text_size(px(TEXT_CAPTION))
                        .text_color(hsla(muted))
                        .child(format_keys(&k)),
                )
            })
            .when_some(self.removable.filter(|_| !inverted), |d, (tip_text, f)| {
                d.child(
                    IconButton::new(child_id(&id, "remove"), Glyph::X)
                        .size(ControlSize::Xs)
                        .tooltip(tip_text)
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            f(window, cx);
                            window.refresh();
                        }),
                )
            })
            .when_some(self.favorite, |d, (on, f)| {
                let star = if on { Glyph::StarFilled } else { Glyph::STAR };
                d.child(
                    div()
                        .id(child_id(&id, "star"))
                        .flex_none()
                        .size(px(CONTROL_XS))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(R_SM))
                        .cursor_pointer()
                        .when(on, |d| d.text_color(hsla(t.favorite)))
                        .when(!on, |d| {
                            d.invisible()
                                .group_hover(group.clone(), |s| s.visible())
                                .text_color(hsla(muted))
                                .hover(|s| s.text_color(hsla(fg)))
                        })
                        .child(glyph(star, ICON_SM))
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            f(window, cx);
                            window.refresh();
                        }),
                )
            })
            .when_some(self.description, |d, desc| d.tooltip(tip(desc, None)))
            .when_some(nav.cloned(), |d, nav| {
                d.on_hover(move |hovered, _, cx| {
                    nav.update(cx, |n, cx| {
                        let next = if *hovered {
                            Some(index)
                        } else if n.highlighted == Some(index) {
                            None
                        } else {
                            n.highlighted
                        };
                        if next != n.highlighted {
                            n.highlighted = next;
                            cx.notify();
                        }
                    })
                })
            })
            .when(interactive, |d| {
                d.cursor_pointer().on_click(move |_, window, cx| {
                    activate(on_select.as_ref(), keep_open, dismiss.as_ref(), window, cx)
                })
            })
    }
}

fn activate(
    on_select: Option<&Handler>,
    keep_open: bool,
    dismiss: Option<&Handler>,
    window: &mut Window,
    cx: &mut App,
) {
    if !keep_open && let Some(d) = dismiss {
        d(window, cx);
    }
    if let Some(f) = on_select {
        f(window, cx);
    }
    if keep_open {
        window.refresh();
    }
}

impl RenderOnce for MenuRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // Standalone row (outside a Menu): CSS hover, no keyboard state.
        let check_col = self.checked.is_some();
        let t = cx.ui().clone();
        self.render_row(0, None, false, (check_col, false), None, &t)
    }
}

/// Uppercase caption over a group of rows (§3.4): h 28, 11 px, `text_muted`,
/// optional collapse chevron.
#[derive(IntoElement)]
pub struct SectionHeader {
    id: ElementId,
    title: SharedString,
    collapsible: Option<(bool, Handler)>,
}

impl SectionHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        let title: SharedString = title.into();
        Self {
            id: ElementId::Name(SharedString::from(format!("section-{title}"))),
            title,
            collapsible: None,
        }
    }

    /// Clickable header with a chevron (up when open).
    pub fn collapsible(
        mut self,
        open: bool,
        on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.collapsible = Some((open, Rc::new(on_toggle)));
        self
    }
}

impl RenderOnce for SectionHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        let base = h_flex()
            .id(self.id)
            .flex_none()
            .h(px(H_SECTION))
            .px(px(S2))
            .items_center()
            .rounded(px(R_MD))
            .text_size(px(TEXT_CAPTION))
            .font_weight(FontWeight::MEDIUM)
            .text_color(hsla(t.text_muted))
            .child(div().flex_1().child(self.title.to_uppercase()));
        match self.collapsible {
            None => base,
            Some((open, f)) => base
                .cursor_pointer()
                .hover(|s| s.bg(hsla(t.hover)))
                .child(glyph(
                    if open {
                        Glyph::CHEVRON_UP
                    } else {
                        Glyph::CHEVRON_DOWN
                    },
                    ICON_XS,
                ))
                .on_click(move |_, window, cx| {
                    f(window, cx);
                    window.refresh();
                }),
        }
    }
}

enum Item {
    Row(MenuRow),
    Section(SectionHeader),
    Separator,
    Element(AnyElement),
}

/// A menu surface with rows (§3.4). Usually built inside
/// `ToolbarButton::menu` / `IconButton::with_flyout` / `Select`, which supply
/// [`Menu::nav`] and [`Menu::on_dismiss`]; rendered on its own (gallery) it
/// keeps its own nav state.
#[derive(IntoElement)]
pub struct Menu {
    id: ElementId,
    items: Vec<Item>,
    width: MenuWidth,
    max_h: Option<Pixels>,
    nav: Option<Entity<MenuNav>>,
    dismiss: Option<Handler>,
    surface: bool,
    large: bool,
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

impl Menu {
    pub fn new() -> Self {
        Self {
            id: "tt-menu".into(),
            items: Vec::new(),
            width: MenuWidth::Md,
            max_h: None,
            nav: None,
            dismiss: None,
            surface: true,
            large: false,
        }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    pub fn width(mut self, w: MenuWidth) -> Self {
        self.width = w;
        self
    }

    /// Scrolls past `h`; the keyboard keeps the highlighted row in view.
    pub fn max_h(mut self, h: Pixels) -> Self {
        self.max_h = Some(h);
        self
    }

    pub fn row(mut self, row: MenuRow) -> Self {
        self.items.push(Item::Row(row));
        self
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = MenuRow>) -> Self {
        self.items.extend(rows.into_iter().map(Item::Row));
        self
    }

    pub fn section(mut self, title: impl Into<SharedString>) -> Self {
        self.items.push(Item::Section(SectionHeader::new(title)));
        self
    }

    pub fn collapsible_section(
        mut self,
        title: impl Into<SharedString>,
        open: bool,
        on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.items.push(Item::Section(
            SectionHeader::new(title).collapsible(open, on_toggle),
        ));
        self
    }

    pub fn separator(mut self) -> Self {
        self.items.push(Item::Separator);
        self
    }

    /// Any element (a custom row, an empty-state line).
    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.items.push(Item::Element(el.into_any_element()));
        self
    }

    /// Keyboard / highlight state (owned by the trigger).
    pub fn nav(mut self, nav: Entity<MenuNav>) -> Self {
        self.nav = Some(nav);
        self
    }

    /// How a row click closes the menu (popover dismiss).
    pub fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.dismiss = Some(Rc::new(f));
        self
    }

    /// TradingView's large rows: 40 tall, icons 28 (SVG) / 20 (Lucide),
    /// 4 px icon inset and gap, 13 px group separators. The chart-type menu
    /// and the drawing flyouts.
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }

    /// Draw without the surface (bg, border, shadow); for embedding.
    pub fn bare(mut self) -> Self {
        self.surface = false;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Moves the highlight `step` rows among `rows` (selectable item indices).
fn step_highlight(rows: &[usize], current: Option<usize>, step: isize) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    let n = rows.len() as isize;
    let pos = current.and_then(|c| rows.iter().position(|r| *r == c));
    let next = match pos {
        None if step >= 0 => 0,
        None => n - 1,
        Some(p) => {
            let p = p as isize + step;
            if step.abs() == 1 {
                p.rem_euclid(n)
            } else {
                p.clamp(0, n - 1)
            }
        }
    };
    Some(rows[next as usize])
}

impl RenderOnce for Menu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let nav = self
            .nav
            .clone()
            .unwrap_or_else(|| MenuNav::keyed(child_id(&self.id, "nav"), window, cx));
        let (focus, highlighted, scroll) = {
            let n = nav.read(cx);
            (n.focus.clone(), n.highlighted, n.scroll.clone())
        };
        let check_col = self
            .items
            .iter()
            .any(|i| matches!(i, Item::Row(r) if r.checked.is_some()));
        let large = self.large;

        // Keyboard targets: (item index, label, handler, keep_open).
        let mut targets: Vec<(usize, SharedString, Option<Handler>, bool)> = Vec::new();
        let mut list = v_flex()
            .id(child_id(&self.id, "list"))
            .track_scroll(&scroll)
            .when_some(self.max_h, |d, h| d.max_h(h).overflow_y_scroll());
        for (i, item) in self.items.into_iter().enumerate() {
            list = match item {
                Item::Row(r) => {
                    if !r.disabled {
                        targets.push((i, r.label.clone(), r.on_select.clone(), r.keep_open));
                    }
                    let hl = highlighted == Some(i);
                    let dismiss = self.dismiss.clone();
                    list.child(r.render_row(i, Some(&nav), hl, (check_col, large), dismiss, &t))
                }
                Item::Section(s) => list.child(s),
                // Large menus: 6 + 1 + 6 between groups (TradingView).
                Item::Separator if large => {
                    list.child(div().py(px(S1_5 - S1)).child(Divider::horizontal()))
                }
                Item::Separator => list.child(Divider::horizontal()),
                Item::Element(e) => list.child(e),
            };
        }
        let rows: Vec<usize> = targets.iter().map(|t| t.0).collect();
        let dismiss = self.dismiss.clone();
        let key_nav = nav.clone();
        v_flex()
            .id(self.id)
            .key_context("TtMenu")
            .track_focus(&focus)
            .w(px(self.width.px()))
            .p(px(if large { S1_5 } else { S1 }))
            .text_color(hsla(t.text))
            .when(self.surface, |d| {
                d.bg(hsla(t.overlay_bg))
                    .border_1()
                    .border_color(hsla(t.border))
                    .rounded(px(R_LG))
                    .shadow_md()
            })
            .on_key_down(move |e: &KeyDownEvent, window, cx| {
                let k = &e.keystroke;
                if k.modifiers.control || k.modifiers.alt || k.modifiers.platform {
                    return;
                }
                let current = key_nav.read(cx).highlighted;
                let next = match k.key.as_str() {
                    "up" => step_highlight(&rows, current, -1),
                    "down" => step_highlight(&rows, current, 1),
                    "home" => rows.first().copied(),
                    "end" => rows.last().copied(),
                    "pageup" => step_highlight(&rows, current, -8),
                    "pagedown" => step_highlight(&rows, current, 8),
                    "enter" => {
                        if let Some(target) = targets.iter().find(|t| Some(t.0) == current) {
                            cx.stop_propagation();
                            activate(target.2.as_ref(), target.3, dismiss.as_ref(), window, cx);
                        }
                        return;
                    }
                    _ => {
                        // Type-ahead: the next row starting with the letter.
                        let Some(ch) = k
                            .key_char
                            .as_deref()
                            .filter(|s| s.chars().count() == 1)
                            .and_then(|s| s.chars().next())
                            .filter(|c| c.is_alphanumeric())
                        else {
                            return;
                        };
                        let lower = ch.to_lowercase().to_string();
                        let start = current
                            .and_then(|c| targets.iter().position(|t| t.0 == c))
                            .map_or(0, |p| p + 1);
                        let n = targets.len();
                        (0..n)
                            .map(|o| &targets[(start + o) % n])
                            .find(|t| t.1.to_lowercase().starts_with(&lower))
                            .map(|t| t.0)
                    }
                };
                if let Some(ix) = next {
                    cx.stop_propagation();
                    key_nav.update(cx, |n, cx| {
                        n.highlighted = Some(ix);
                        n.scroll.scroll_to_item(ix);
                        cx.notify();
                    });
                }
            })
            .child(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn highlight_wraps_and_clamps() {
        let rows = [1, 2, 4, 7];
        assert_eq!(step_highlight(&rows, None, 1), Some(1));
        assert_eq!(step_highlight(&rows, None, -1), Some(7));
        assert_eq!(step_highlight(&rows, Some(7), 1), Some(1));
        assert_eq!(step_highlight(&rows, Some(1), -1), Some(7));
        assert_eq!(step_highlight(&rows, Some(2), 8), Some(7));
        assert_eq!(step_highlight(&rows, Some(4), -8), Some(1));
        assert_eq!(step_highlight(&[], None, 1), None);
    }
}
