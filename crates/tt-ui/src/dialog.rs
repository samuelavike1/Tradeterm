//! Dialog scaffold (§3.6): sizes, left nav or top tabs, fixed body height
//! (switching tabs does not jump), footer with optional "Template ▾" menu and
//! Cancel / Ok.
//!
//! Rules: Esc and × mean Cancel; Enter means Ok (wired to gpui-component's
//! `Confirm` through `Dialog::on_ok`); titles in sentence case; picker
//! dialogs (search, indicators) have no footer.

use crate::button::{ToolbarButton, child_id};
use crate::glyph::{Glyph, glyph};
use crate::menu::{Menu, MenuRow};
use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{Disableable as _, Sizable as _, Size, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Pointer and dialog offset when a header drag started.
type DragStart = Option<(Point<Pixels>, Point<Pixels>)>;

/// Moves a dialog by its header, like TradingView: the offset from the
/// default (centered) position, applied as a relative offset every render.
#[derive(Clone, Default)]
pub(crate) struct DialogDrag {
    offset: Rc<Cell<Point<Pixels>>>,
    /// Pointer and offset when the press started (`None`: not dragging).
    start: Rc<Cell<DragStart>>,
}

/// The value gpui carries while a dialog header is dragged.
struct DialogDragValue;

/// Nothing follows the pointer: the dialog itself moves.
struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl DialogDrag {
    /// The offset to apply, kept so the header stays reachable in `view`.
    fn offset_in(&self, view: gpui_kit::Size<Pixels>, width: Pixels) -> Point<Pixels> {
        let o = self.offset.get();
        let max_x = ((view.width + width) / 2. - px(80.)).max(px(0.));
        // The dialog starts a tenth of the window down; keep its header in
        // the window.
        let min_y = -(view.height / 10.);
        let max_y = (view.height - view.height / 10. - px(C_H_HEADER)).max(px(0.));
        point(o.x.clamp(-max_x, max_x), o.y.clamp(min_y, max_y))
    }

    /// Makes `el` (the title area) the drag handle.
    fn handle(&self, el: Div) -> Stateful<Div> {
        let (down, up, moving) = (self.clone(), self.clone(), self.clone());
        el.id("dialog-drag")
            .on_mouse_down(MouseButton::Left, move |e, _, _| {
                down.start.set(Some((e.position, down.offset.get())));
            })
            .on_mouse_up(MouseButton::Left, move |_, _, _| up.start.set(None))
            .on_drag(DialogDragValue, |_, _, _, cx| cx.new(|_| DragGhost))
            .on_drag_move::<DialogDragValue>(move |e, window, _| {
                if let Some((p0, o0)) = moving.start.get() {
                    moving.offset.set(o0 + (e.event.position - p0));
                    window.refresh();
                }
            })
    }
}

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

type RenameHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// What [`open_dialog`] needs besides the panel.
pub struct DialogOpts {
    title: SharedString,
    size: DialogSize,
    footer: Option<DialogFooter>,
    on_open: Option<OnceHandler>,
    comfortable: bool,
    width: Option<f32>,
    rename: Option<(SharedString, RenameHandler)>,
}

impl DialogOpts {
    pub fn new(title: impl Into<SharedString>, size: DialogSize) -> Self {
        Self {
            title: title.into(),
            size,
            footer: None,
            on_open: None,
            comfortable: false,
            width: None,
            rename: None,
        }
    }

    /// TradingView-measured density (section 2.10): 20 px semibold title
    /// with its own close button, radius `C_R_DIALOG`, no outer padding
    /// (the [`DialogFrame::comfortable`] body and the comfortable footer pad
    /// themselves), height fits the content. Width `C_W_DIALOG` unless
    /// [`DialogOpts::width`] says otherwise.
    pub fn comfortable(mut self) -> Self {
        self.comfortable = true;
        self
    }

    /// Fixed width (overrides the size's).
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// A pencil after the title renames: it turns the title into an input
    /// (Enter or leaving it commits, Escape cancels) and calls `on_rename`
    /// with the new name (empty: back to the default `title`). `current` is
    /// the name shown now (empty: the default title).
    pub fn rename(
        mut self,
        current: impl Into<SharedString>,
        on_rename: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.rename = Some((current.into(), Rc::new(on_rename)));
        self
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
        comfortable,
        width,
        rename,
    } = opts;
    let on_open = RefCell::new(on_open);
    let drag = DialogDrag::default();
    let header = comfortable.then(|| {
        let (shown, on_rename) = match rename {
            Some((cur, f)) => (Some(cur), Some(f)),
            None => (None, None),
        };
        let default = title.clone();
        let drag = drag.clone();
        cx.new(|cx| DialogHeader::new(default, shown, on_rename, drag, window, cx))
    });
    let width = width.unwrap_or(if comfortable {
        C_W_DIALOG
    } else {
        size.width()
    });
    window.open_dialog(cx, move |dialog, window, cx| {
        if let Some(f) = on_open.borrow_mut().take() {
            f(window, cx);
        }
        let dialog = match &header {
            Some(h) => dialog
                .title(h.clone())
                .close_button(false)
                .p(px(0.))
                .rounded(px(C_R_DIALOG)),
            None => dialog.title(drag.handle(div().w_full().child(title.clone()))),
        };
        // Dragged by its header: shift the (centered) surface.
        let o = drag.offset_in(window.viewport_size(), px(width));
        let dialog = dialog
            .relative()
            .left(o.x)
            .top(o.y)
            .w(px(width))
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
    comfortable: bool,
    min_body: f32,
}

impl DialogFrame {
    pub fn new(size: DialogSize) -> Self {
        Self {
            id: "dialog-frame".into(),
            size,
            nav: None,
            body: None,
            comfortable: false,
            min_body: 0.0,
        }
    }

    /// Section 2.10: top tabs (16 px semibold, 4 px underline) and a body
    /// padded `C_PAD_Y` / `C_PAD_X` whose height fits its content (at
    /// least `min_body`), like TradingView: the dialog grows or shrinks
    /// with the tab.
    pub fn comfortable(mut self) -> Self {
        self.comfortable = true;
        self
    }

    /// Smallest body height (comfortable frames), so short tabs keep a
    /// steady footer.
    pub fn min_body(mut self, h: f32) -> Self {
        self.min_body = h;
        self
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
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        if self.comfortable {
            // Tall tabs (Fib levels) scroll inside the window, like
            // TradingView's.
            let chrome = C_H_HEADER + C_H_TABS + C_H_FOOTER + 2.0 * C_PAD_Y + 4.0 * S6;
            let max = (f32::from(window.viewport_size().height) - chrome).max(self.min_body);
            return v_flex()
                .id(self.id.clone())
                .w_full()
                .when_some(self.nav, |d, nav| d.child(nav.comfortable()))
                .child(
                    v_flex()
                        .id(child_id(&self.id, "body"))
                        .px(px(C_PAD_X))
                        .py(px(C_PAD_Y))
                        .min_h(px(self.min_body))
                        .max_h(px(max))
                        .overflow_y_scroll()
                        .children(self.body),
                )
                .into_any_element();
        }
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
    comfortable: bool,
    items: Vec<(&'static str, SharedString, Option<Glyph>)>,
    selected: Option<&'static str>,
    on_change: Option<NavHandler>,
}

impl NavTabs {
    pub fn vertical() -> Self {
        Self {
            vertical: true,
            comfortable: false,
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

    /// Section 2.10 top tabs: 16 px semibold labels `C_TAB_GAP` apart, a
    /// `C_TAB_UNDERLINE` underline under the current one over a faint track
    /// (set by [`DialogFrame::comfortable`]).
    pub fn comfortable(mut self) -> Self {
        self.comfortable = true;
        self.vertical = false;
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
        if self.comfortable {
            let track = if t.dark {
                tt_theme::mix(t.bg, t.text, 0.1)
            } else {
                tt_theme::mix(t.bg, t.text, 0.05)
            };
            let mut row = h_flex().gap(px(C_TAB_GAP)).items_end().h_full();
            for (id, label, _) in self.items {
                let selected = self.selected == Some(id);
                let f = self.on_change.clone();
                row = row.child(
                    div()
                        .id(SharedString::from(format!("tab-{id}")))
                        .relative()
                        .h_full()
                        .pb(px(S2))
                        .flex()
                        .items_end()
                        .cursor_pointer()
                        .text_size(px(C_TEXT_TAB))
                        .line_height(px(C_H_TABS - S2))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(hsla(t.text))
                        .when(!selected, |d| d.hover(|s| s.text_color(hsla(t.text_muted))))
                        .child(label)
                        .when(selected, |d| {
                            d.child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .right_0()
                                    .bottom_0()
                                    .h(px(C_TAB_UNDERLINE))
                                    .rounded(px(2.0))
                                    .bg(hsla(t.text)),
                            )
                        })
                        .on_click(move |_, w, cx| {
                            if let Some(f) = &f {
                                f(id, w, cx)
                            }
                        }),
                );
            }
            return div()
                .relative()
                .mx(px(C_PAD_X))
                .h(px(C_H_TABS))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(C_TAB_UNDERLINE))
                        .rounded(px(2.0))
                        .bg(hsla(track)),
                )
                .child(row)
                .into_any_element();
        }
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

type MenuBuilder = Rc<dyn Fn(Menu, &mut Window, &mut App) -> Menu>;

/// "Template ▾": Apply defaults, or any rows built when the menu opens
/// ([`TemplateMenu::build`]: saved templates, Save as…).
#[derive(Clone)]
pub struct TemplateMenu {
    rows: Vec<(SharedString, Handler)>,
    build: Option<MenuBuilder>,
    label: Option<SharedString>,
}

impl TemplateMenu {
    /// One row: "Apply defaults".
    pub fn defaults(on_apply: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        Self {
            rows: vec![("Apply defaults".into(), Rc::new(on_apply))],
            build: None,
            label: None,
        }
    }

    /// Rows built each time the menu opens (after any fixed rows), so they
    /// can list state that changes while the dialog is open.
    pub fn build(f: impl Fn(Menu, &mut Window, &mut App) -> Menu + 'static) -> Self {
        Self {
            rows: Vec::new(),
            build: Some(Rc::new(f)),
            label: None,
        }
    }

    /// Trigger label (default "Template"; indicator settings use
    /// TradingView's "Defaults").
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
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
    comfortable: bool,
    template: Option<TemplateMenu>,
    actions: Vec<(SharedString, Handler)>,
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
            comfortable: false,
            template: None,
            actions: Vec::new(),
            cancel: None,
            ok: None,
            ok_disabled: false,
            on_ok: None,
            on_cancel: None,
        }
    }

    /// Section 2.10: 34 px buttons, 16 px labels, radius `C_R_CONTROL`,
    /// padded `C_PAD_X` (with a [`DialogOpts::comfortable`] dialog).
    pub fn comfortable(mut self) -> Self {
        self.comfortable = true;
        self
    }

    pub fn template(mut self, menu: TemplateMenu) -> Self {
        self.template = Some(menu);
        self
    }

    /// An outline button on the left, after the template menu ("Apply to
    /// all tabs"). It does not close the dialog by itself.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.actions.push((label.into(), Rc::new(f)));
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
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let comfortable = self.comfortable;
        // TradingView's Cancel: a 1 px `text` outline.
        let cancel_border = hsla(cx.ui().text);
        let footer_button = move |b: Button| {
            if comfortable {
                b.with_size(Size::Size(px(C_CONTROL)))
                    .h(px(C_CONTROL))
                    .px(px(11.0))
                    .py_0()
                    .rounded(px(C_R_CONTROL))
                    .text_size(px(C_TEXT_BUTTON))
            } else {
                b.with_size(Size::Size(px(CONTROL_MD)))
                    .h(px(CONTROL_MD))
                    .min_w(px(W_DIALOG_BUTTON))
                    .px(px(S4))
                    .py_0()
            }
        };
        let left = self.template.map(|tm| {
            let (rows, build) = (tm.rows, tm.build);
            let label = tm.label.unwrap_or_else(|| "Template".into());
            ToolbarButton::text("dialog-template", label)
                .caret()
                .outlined(comfortable)
                .menu(move |menu, w, cx| {
                    let mut menu = menu;
                    for (i, (label, f)) in rows.iter().enumerate() {
                        let f = f.clone();
                        menu = menu.row(
                            MenuRow::new(("template", i), label.clone())
                                .on_select(move |w, cx| f(w, cx)),
                        );
                    }
                    match &build {
                        Some(b) => b(menu, w, cx),
                        None => menu,
                    }
                })
        });
        let actions = self.actions.into_iter().enumerate().map(|(i, (label, f))| {
            footer_button(Button::new(("dialog-action", i)).outline().label(label))
                .on_click(move |_, window, cx| f(window, cx))
        });
        let cancel = self.cancel.map(|label| {
            let f = self.on_cancel.clone();
            footer_button(Button::new("dialog-cancel").outline().label(label))
                .when(comfortable, |b| b.border_color(cancel_border))
                .on_click(move |_, window, cx| {
                    if let Some(f) = &f {
                        f(window, cx);
                    }
                    window.close_dialog(cx);
                })
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
        let gap = if comfortable { S3 } else { S2 };
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .when(comfortable, |d| {
                d.px(px(C_PAD_X)).pt(px(C_PAD_Y - S2)).pb(px(C_PAD_Y))
            })
            .child(h_flex().gap(px(S2)).children(left).children(actions))
            .child(h_flex().gap(px(gap)).children(cancel).children(ok))
    }
}

/// The comfortable dialog's title row: 20 px semibold title, an optional
/// rename pencil (the title becomes an input), and the close button.
pub struct DialogHeader {
    default: SharedString,
    /// The custom name (`None`: not renamable).
    name: Option<SharedString>,
    editing: bool,
    input: Entity<InputState>,
    on_rename: Option<RenameHandler>,
    drag: DialogDrag,
    _subs: Vec<Subscription>,
}

impl DialogHeader {
    fn new(
        default: SharedString,
        name: Option<SharedString>,
        on_rename: Option<RenameHandler>,
        drag: DialogDrag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        let sub = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, w, cx| match ev {
            InputEvent::PressEnter { .. } | InputEvent::Blur => this.finish(true, w, cx),
            _ => {}
        });
        Self {
            default,
            name,
            editing: false,
            input,
            on_rename,
            drag,
            _subs: vec![sub],
        }
    }

    /// The title shown: the custom name, else the default.
    pub fn shown(&self) -> SharedString {
        match &self.name {
            Some(n) if !n.trim().is_empty() => n.clone(),
            _ => self.default.clone(),
        }
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = true;
        let s = self.shown();
        self.input.update(cx, |i, cx| {
            i.set_value(s, window, cx);
            i.focus(window, cx);
        });
        cx.notify();
    }

    fn finish(&mut self, commit: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        self.editing = false;
        if commit {
            let v: SharedString = self.input.read(cx).value().trim().to_owned().into();
            // The default title typed back means "no custom name".
            let v: SharedString = if v == self.default { "".into() } else { v };
            if Some(&v) != self.name.as_ref() {
                self.name = Some(v.clone());
                if let Some(f) = self.on_rename.clone() {
                    f(v, window, cx);
                }
            }
        }
        cx.notify();
    }
}

impl Render for DialogHeader {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let this = cx.entity().downgrade();
        let title: AnyElement = if self.editing {
            let esc = this.clone();
            div()
                .flex_1()
                .min_w_0()
                // Enter / Escape end the rename only: captured before the
                // input and the dialog (whose Enter is Ok, Escape Cancel).
                .capture_key_down(move |e: &KeyDownEvent, w, cx| {
                    let commit = match e.keystroke.key.as_str() {
                        "enter" => true,
                        "escape" => false,
                        _ => return,
                    };
                    cx.stop_propagation();
                    w.prevent_default();
                    esc.update(cx, |h, cx| h.finish(commit, w, cx)).ok();
                })
                .child(
                    Input::new(&self.input)
                        .map(|i| Styled::h(i, px(C_HEADER_BUTTON)))
                        .rounded(px(C_R_CONTROL))
                        .text_size(px(C_TEXT_TITLE))
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .into_any_element()
        } else {
            h_flex()
                .flex_1()
                .min_w_0()
                .gap(px(5.0))
                .items_center()
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(C_TEXT_TITLE))
                        .line_height(px(C_TITLE_LINE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(hsla(t.text))
                        .child(self.shown()),
                )
                .when(self.on_rename.is_some(), |d| {
                    let start = this.clone();
                    d.child(
                        Button::new("dialog-rename")
                            .custom(crate::button::variant(&t, hsla(t.text), cx))
                            .with_size(Size::Size(px(C_HEADER_BUTTON)))
                            .rounded(px(R_XS))
                            .w(px(C_HEADER_BUTTON))
                            .h(px(C_HEADER_BUTTON))
                            .px_0()
                            .py_0()
                            .child(glyph(Glyph::PENCIL, ICON_MD + 2.0))
                            .tooltip("Rename")
                            .on_click(move |_, w, cx| {
                                start.update(cx, |h, cx| h.start(w, cx)).ok();
                            }),
                    )
                })
                .into_any_element()
        };
        self.drag
            .handle(h_flex())
            .w_full()
            .h(px(C_H_HEADER - S2))
            .pt(px((C_H_HEADER - C_HEADER_BUTTON) / 2.0))
            .px(px(C_PAD_X))
            .gap(px(S2))
            .items_start()
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .h(px(C_HEADER_BUTTON))
                    .child(title),
            )
            .child(
                Button::new("dialog-close")
                    .custom(crate::button::variant(&t, hsla(t.text), cx))
                    .with_size(Size::Size(px(C_HEADER_BUTTON)))
                    .rounded(px(C_R_CONTROL))
                    .w(px(C_HEADER_BUTTON))
                    .h(px(C_HEADER_BUTTON))
                    .px_0()
                    .py_0()
                    .child(glyph(Glyph::X, ICON_MD))
                    .tooltip("Close")
                    .on_click(|_, w, cx| w.close_dialog(cx)),
            )
    }
}
