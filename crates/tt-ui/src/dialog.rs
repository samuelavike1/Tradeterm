//! Dialog scaffold (§3.6): sizes, left nav or top tabs, fixed body height
//! (switching tabs does not jump), footer with optional "Template ▾" menu and
//! Cancel / Ok.
//!
//! Rules: Esc and × mean Cancel; Enter means Ok (wired to gpui-component's
//! `Confirm` through `Dialog::on_ok`); titles in sentence case; picker
//! dialogs (search, indicators) have no footer.

use crate::button::{ToolbarButton, child_id};
use crate::glyph::{Glyph, glyph};
use crate::menu::MenuRow;
use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{Disableable as _, Sizable as _, Size, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::RefCell;
use std::rc::Rc;

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type OnceHandler = Box<dyn FnOnce(&mut Window, &mut App)>;
type OkHandler = Rc<dyn Fn(&mut Window, &mut App) -> bool>;
type NavHandler = Rc<dyn Fn(&'static str, &mut Window, &mut App)>;

/// Dialog sizes: width 480 / 680 / 800, body height 280 / 460 / 480.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DialogSize {
    Sm,
    #[default]
    Md,
    Lg,
}

impl DialogSize {
    pub fn width(self) -> f32 {
        match self {
            DialogSize::Sm => W_DIALOG_SM,
            DialogSize::Md => W_DIALOG_MD,
            DialogSize::Lg => W_DIALOG_LG,
        }
    }

    pub fn body_height(self) -> f32 {
        match self {
            DialogSize::Sm => H_DIALOG_BODY_SM,
            DialogSize::Md => H_DIALOG_BODY_MD,
            DialogSize::Lg => H_DIALOG_BODY_LG,
        }
    }

    /// `FormRow` label column for this size.
    pub fn label_width(self) -> f32 {
        match self {
            DialogSize::Lg => W_LABEL_LG,
            _ => W_LABEL_SM,
        }
    }
}

/// What [`open_dialog`] needs besides the panel.
pub struct DialogOpts {
    title: SharedString,
    size: DialogSize,
    footer: Option<DialogFooter>,
    on_open: Option<OnceHandler>,
}

impl DialogOpts {
    pub fn new(title: impl Into<SharedString>, size: DialogSize) -> Self {
        Self {
            title: title.into(),
            size,
            footer: None,
            on_open: None,
        }
    }

    pub fn footer(mut self, f: DialogFooter) -> Self {
        self.footer = Some(f);
        self
    }

    /// Runs once, on the first render (focus the first input here).
    pub fn on_open(mut self, f: impl FnOnce(&mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Box::new(f));
        self
    }
}

/// Opens a dialog with the standard frame. `panel` renders the body (wrap it
/// in a [`DialogFrame`] for nav and the fixed body height).
pub fn open_dialog<P: Render>(
    window: &mut Window,
    cx: &mut App,
    opts: DialogOpts,
    panel: Entity<P>,
) {
    let DialogOpts {
        title,
        size,
        footer,
        on_open,
    } = opts;
    let on_open = RefCell::new(on_open);
    window.open_dialog(cx, move |dialog, window, cx| {
        if let Some(f) = on_open.borrow_mut().take() {
            f(window, cx);
        }
        let dialog = dialog
            .title(title.clone())
            .w(px(size.width()))
            .entrance(DialogEntrance::FadeSlide)
            .child(panel.clone());
        match footer.clone() {
            Some(f) => {
                let ok = f.on_ok.clone();
                let dialog = dialog.footer(f);
                match ok {
                    Some(ok) => dialog.on_ok(move |_, w, cx| ok(w, cx)),
                    None => dialog,
                }
            }
            None => dialog,
        }
    });
}

/// The dialog body: optional [`NavTabs`] (vertical: left, horizontal: top)
/// and a body that scrolls inside the fixed height of the size.
#[derive(IntoElement)]
pub struct DialogFrame {
    id: ElementId,
    size: DialogSize,
    nav: Option<NavTabs>,
    body: Option<AnyElement>,
}

impl DialogFrame {
    pub fn new(size: DialogSize) -> Self {
        Self {
            id: "dialog-frame".into(),
            size,
            nav: None,
            body: None,
        }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Left nav (≥ 4 sections) or top tabs (≤ 3).
    pub fn nav(mut self, nav: NavTabs) -> Self {
        self.nav = Some(nav);
        self
    }

    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }
}

impl RenderOnce for DialogFrame {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let h = self.size.body_height();
        let body = v_flex()
            .id(child_id(&self.id, "body"))
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .children(self.body);
        match self.nav {
            Some(nav) if nav.vertical => h_flex()
                .h(px(h))
                .gap(px(S4))
                .items_start()
                .child(nav)
                .child(body)
                .into_any_element(),
            Some(nav) => v_flex()
                .h(px(h))
                .gap(px(S3))
                .child(nav)
                .child(body)
                .into_any_element(),
            None => v_flex().h(px(h)).child(body).into_any_element(),
        }
    }
}

/// Dialog sections: vertical (left nav, `W_NAV`, rows 36, glyph 16) or
/// horizontal (gpui-component `TabBar::underline().small()`).
#[derive(IntoElement)]
pub struct NavTabs {
    vertical: bool,
    items: Vec<(&'static str, SharedString, Option<Glyph>)>,
    selected: Option<&'static str>,
    on_change: Option<NavHandler>,
}

impl NavTabs {
    pub fn vertical() -> Self {
        Self {
            vertical: true,
            items: Vec::new(),
            selected: None,
            on_change: None,
        }
    }

    pub fn horizontal() -> Self {
        Self {
            vertical: false,
            ..Self::vertical()
        }
    }

    pub fn item(
        mut self,
        id: &'static str,
        label: impl Into<SharedString>,
        glyph: Option<Glyph>,
    ) -> Self {
        self.items.push((id, label.into(), glyph));
        self
    }

    pub fn selected(mut self, id: &'static str) -> Self {
        self.selected = Some(id);
        self
    }

    pub fn on_change(mut self, f: impl Fn(&'static str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for NavTabs {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        if !self.vertical {
            let ids: Vec<&'static str> = self.items.iter().map(|i| i.0).collect();
            let ix = self
                .selected
                .and_then(|s| ids.iter().position(|i| *i == s))
                .unwrap_or(0);
            let f = self.on_change.clone();
            return TabBar::new("nav-tabs")
                .underline()
                .small()
                .selected_index(ix)
                .children(
                    self.items
                        .iter()
                        .map(|(_, l, _)| Tab::new().label(l.clone())),
                )
                .on_click(move |i: &usize, w, cx| {
                    if let (Some(f), Some(id)) = (&f, ids.get(*i)) {
                        f(id, w, cx)
                    }
                })
                .into_any_element();
        }
        let mut col = v_flex().flex_none().w(px(W_NAV)).gap(px(S0_5));
        for (id, label, g) in self.items {
            let selected = self.selected == Some(id);
            let f = self.on_change.clone();
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("nav-{id}")))
                    .h(px(H_LIST_ROW))
                    .px(px(S3))
                    .gap(px(S2))
                    .items_center()
                    .rounded(px(R_MD))
                    .cursor_pointer()
                    .text_size(px(TEXT_BODY))
                    .text_color(hsla(t.text))
                    .when(selected, |d| d.bg(hsla(t.selection)))
                    .when(!selected, |d| d.hover(|s| s.bg(hsla(t.hover))))
                    .when_some(g, |d, g| d.child(glyph(g, ICON_SM)))
                    .child(label)
                    .on_click(move |_, w, cx| {
                        if let Some(f) = &f {
                            f(id, w, cx)
                        }
                    }),
            );
        }
        col.into_any_element()
    }
}

/// "Template ▾": Apply defaults (later: Save as…).
#[derive(Clone)]
pub struct TemplateMenu {
    rows: Vec<(SharedString, Handler)>,
}

impl TemplateMenu {
    /// One row: "Apply defaults".
    pub fn defaults(on_apply: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        Self {
            rows: vec![("Apply defaults".into(), Rc::new(on_apply))],
        }
    }

    pub fn item(
        mut self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.rows.push((label.into(), Rc::new(f)));
        self
    }
}

/// Dialog footer: Template ▾ on the left, Cancel / Ok on the right
/// (32 px, min width 72, Ok rightmost and primary).
#[derive(IntoElement, Clone)]
pub struct DialogFooter {
    template: Option<TemplateMenu>,
    cancel: Option<SharedString>,
    ok: Option<SharedString>,
    ok_disabled: bool,
    on_ok: Option<OkHandler>,
    on_cancel: Option<Handler>,
}

impl Default for DialogFooter {
    fn default() -> Self {
        Self::new()
    }
}

impl DialogFooter {
    pub fn new() -> Self {
        Self {
            template: None,
            cancel: None,
            ok: None,
            ok_disabled: false,
            on_ok: None,
            on_cancel: None,
        }
    }

    pub fn template(mut self, menu: TemplateMenu) -> Self {
        self.template = Some(menu);
        self
    }

    /// A "Cancel" button (closes the dialog after `on_cancel`).
    pub fn cancel(mut self) -> Self {
        self.cancel = Some("Cancel".into());
        self
    }

    pub fn ok(mut self, label: impl Into<SharedString>) -> Self {
        self.ok = Some(label.into());
        self
    }

    /// Ok / Enter. Return `true` to close the dialog (`false` keeps it open,
    /// e.g. on a validation error).
    pub fn on_ok(mut self, f: impl Fn(&mut Window, &mut App) -> bool + 'static) -> Self {
        self.on_ok = Some(Rc::new(f));
        self
    }

    pub fn on_cancel(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(f));
        self
    }

    pub fn ok_disabled(mut self, d: bool) -> Self {
        self.ok_disabled = d;
        self
    }
}

impl RenderOnce for DialogFooter {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let footer_button = |b: Button| {
            b.with_size(Size::Size(px(CONTROL_MD)))
                .h(px(CONTROL_MD))
                .min_w(px(W_DIALOG_BUTTON))
                .px(px(S4))
                .py_0()
        };
        let left = self.template.map(|tm| {
            let rows = tm.rows;
            ToolbarButton::text("dialog-template", "Template")
                .caret()
                .menu(move |menu, _, _| {
                    let mut menu = menu;
                    for (i, (label, f)) in rows.iter().enumerate() {
                        let f = f.clone();
                        menu = menu.row(
                            MenuRow::new(("template", i), label.clone())
                                .on_select(move |w, cx| f(w, cx)),
                        );
                    }
                    menu
                })
        });
        let cancel = self.cancel.map(|label| {
            let f = self.on_cancel.clone();
            footer_button(Button::new("dialog-cancel").outline().label(label)).on_click(
                move |_, window, cx| {
                    if let Some(f) = &f {
                        f(window, cx);
                    }
                    window.close_dialog(cx);
                },
            )
        });
        let ok = self.ok.map(|label| {
            let f = self.on_ok.clone();
            footer_button(Button::new("dialog-ok").primary().label(label))
                .disabled(self.ok_disabled)
                .on_click(move |_, window, cx| {
                    let close = f.as_ref().is_none_or(|f| f(window, cx));
                    if close {
                        window.close_dialog(cx);
                    }
                })
        });
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .child(h_flex().children(left))
            .child(h_flex().gap(px(S2)).children(cancel).children(ok))
    }
}
