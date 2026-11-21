//! Buttons (§3.3): [`IconButton`], [`ToolbarButton`], [`Chip`].
//!
//! All are gpui-component `Button`s with a token-built custom variant
//! (transparent at rest, `hover` on hover, `selection` pressed / selected),
//! so they get its managed tooltips (show delay, warm hand-off, hidden when
//! the trigger moves or is clicked), focus handling and accessibility.
//!
//! gpui-component lays out any button with a child as a *text* button
//! (height from `Size`, `px = size × 0.2`). The kit passes `Size::Size(h)`
//! and an explicit `w/h/px` instance style, which gpui-component applies last
//! (`refine_style(&instance_style)`), so icon buttons are exact squares.

use crate::glyph::{Glyph, glyph};
use crate::menu::{Menu, MenuNav};
use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Disableable as _, Placement, Selectable, Sizable as _, Size, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;
use tt_theme::Theme;

pub(crate) type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
pub(crate) type MenuBuilder = Rc<dyn Fn(Menu, &mut Window, &mut App) -> Menu>;
/// A boxed action and the key context its binding is looked up in.
type BoundAction = (Rc<Box<dyn Action>>, Option<SharedString>);

/// Control sizes (§2.4): control = icon + 2 × inset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ControlSize {
    /// 20 px, icon 14: A/L/%, row actions, swatches.
    Xs,
    /// 24 px, icon 16: in-chart buttons, floating toolbar, title bar.
    Sm,
    /// 32 px, icon 18: top toolbar, dialog footer.
    #[default]
    Md,
    /// 36 px, icon 20: left drawing toolbar.
    Lg,
    /// 38 px, icon 20: the floating drawing toolbar (section 2.10,
    /// TradingView-measured).
    Xl,
    /// 34 px: the left drawing toolbar (TradingView-measured). SVG icons
    /// 28, Lucide glyphs 20.
    Drawing,
    /// 44 px: the right sidebar strip (TradingView-measured). SVG icons
    /// 28.
    Sidebar,
}

impl ControlSize {
    /// Height (and width, for square buttons).
    pub fn px(self) -> f32 {
        match self {
            ControlSize::Xs => CONTROL_XS,
            ControlSize::Sm => CONTROL_SM,
            ControlSize::Md => CONTROL_MD,
            ControlSize::Lg => CONTROL_LG,
            ControlSize::Xl => FT_BUTTON,
            ControlSize::Drawing => CONTROL_DRAWING,
            ControlSize::Sidebar => SIDEBAR_BUTTON,
        }
    }

    /// Lucide glyph size; an SVG glyph is drawn at
    /// [`Glyph::svg_size`] of this (28 from Md up).
    pub fn icon(self) -> f32 {
        match self {
            ControlSize::Xs => ICON_XS,
            ControlSize::Sm => ICON_SM,
            ControlSize::Md => ICON_MD,
            ControlSize::Lg | ControlSize::Xl | ControlSize::Drawing | ControlSize::Sidebar => {
                FT_ICON
            }
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            ControlSize::Xs | ControlSize::Xl => R_SM,
            _ => R_MD,
        }
    }

    pub fn text(self) -> f32 {
        match self {
            ControlSize::Xs | ControlSize::Sm => TEXT_SMALL,
            _ => TEXT_BODY,
        }
    }
}

/// Token colors for a kit button with foreground `fg`.
pub(crate) fn variant(t: &Theme, fg: Hsla, cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(gpui_kit::transparent_black())
        .foreground(fg)
        .hover(hsla(t.hover))
        .active(hsla(t.selection))
}

pub(crate) fn child_id(id: &ElementId, name: &'static str) -> ElementId {
    ElementId::NamedChild(id.clone().into(), name.into())
}

#[derive(Clone)]
pub(crate) struct TooltipSpec {
    label: SharedString,
    action: Option<BoundAction>,
}

impl TooltipSpec {
    fn apply(self, b: Button) -> Button {
        match self.action {
            Some((a, ctx)) => {
                b.tooltip_with_action(self.label, a.as_ref().as_ref(), ctx.as_deref())
            }
            None => b.tooltip(self.label),
        }
    }
}

enum Content {
    Glyph(Glyph),
    Text(SharedString),
}

/// A square icon button (§3.3).
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    content: Content,
    size: ControlSize,
    toggled: Option<bool>,
    selected: bool,
    open: bool,
    danger: bool,
    disabled: bool,
    tooltip: Option<TooltipSpec>,
    tooltip_placement: Option<Placement>,
    flyout: Option<MenuBuilder>,
    menu: Option<(MenuBuilder, Anchor)>,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, glyph: impl Into<Glyph>) -> Self {
        Self::with_content(id.into(), Content::Glyph(glyph.into()))
    }

    /// Short text instead of an icon (A / L / %).
    pub fn text(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self::with_content(id.into(), Content::Text(text.into()))
    }

    fn with_content(id: ElementId, content: Content) -> Self {
        Self {
            id,
            content,
            size: ControlSize::Md,
            toggled: None,
            selected: false,
            open: false,
            danger: false,
            disabled: false,
            tooltip: None,
            tooltip_placement: None,
            flyout: None,
            menu: None,
            on_click: None,
        }
    }

    /// Default `Md` (32).
    pub fn size(mut self, s: ControlSize) -> Self {
        self.size = s;
        self
    }

    /// Binary tool / setting: `active` fg on `selection` bg, aria-pressed.
    pub fn toggled(mut self, on: bool) -> Self {
        self.toggled = Some(on);
        self
    }

    /// One of many (current item): `selection` bg.
    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn tooltip(mut self, label: impl Into<SharedString>) -> Self {
        self.tooltip = Some(TooltipSpec {
            label: label.into(),
            action: None,
        });
        self
    }

    /// Tooltip with the key bound to `action` as a Kbd chip ("Trend line
    /// Alt+T"), looked up in `context` (e.g. `Some("Tradeterm")`).
    pub fn tooltip_action(
        mut self,
        label: impl Into<SharedString>,
        action: &dyn Action,
        context: Option<&str>,
    ) -> Self {
        self.tooltip = Some(TooltipSpec {
            label: label.into(),
            action: Some((
                Rc::new(action.boxed_clone()),
                context.map(|c| SharedString::from(c.to_owned())),
            )),
        });
        self
    }

    /// Preferred tooltip side (§2.7: below for top bars, right for the left
    /// toolbar, above for in-chart bottom controls).
    pub fn tooltip_placement(mut self, p: Placement) -> Self {
        self.tooltip_placement = Some(p);
        self
    }

    /// TradingView group button: a corner caret in the bottom-right, a 10 px
    /// hit strip on the right edge and right-click open `menu`, to the right
    /// of the button, top-aligned.
    pub fn with_flyout(
        mut self,
        menu: impl Fn(Menu, &mut Window, &mut App) -> Menu + 'static,
    ) -> Self {
        self.flyout = Some(Rc::new(menu));
        self
    }

    /// A click opens a kit [`Menu`] anchored at `anchor` (the popover's own
    /// corner, as in gpui-component: `TopLeft` opens below, `BottomRight`
    /// opens above and right-aligned, for in-chart corner controls), offset
    /// `S1`. The menu is rebuilt on every render while open; the tooltip is
    /// hidden while it is open.
    pub fn menu(
        mut self,
        anchor: Anchor,
        build: impl Fn(Menu, &mut Window, &mut App) -> Menu + 'static,
    ) -> Self {
        self.menu = Some((Rc::new(build), anchor));
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    fn button(self, open: bool, cx: &App) -> Button {
        let open = open || self.open;
        let t = cx.ui();
        let s = self.size.px();
        let toggled = self.toggled.unwrap_or(false);
        let fg = if self.danger {
            t.danger
        } else if toggled {
            t.active
        } else {
            t.text
        };
        let content = match self.content {
            Content::Glyph(g) => glyph(g, g.svg_size(self.size.icon())).into_any_element(),
            Content::Text(text) => div()
                .text_size(px(self.size.text()))
                .font_weight(FontWeight::MEDIUM)
                .line_height(px(self.size.text()))
                .child(text)
                .into_any_element(),
        };
        let mut b = Button::new(self.id)
            .custom(variant(t, hsla(fg), cx))
            .with_size(Size::Size(px(s)))
            .rounded(px(self.size.radius()))
            .selected(self.selected || toggled || open)
            .disabled(self.disabled)
            .child(content)
            .w(px(s))
            .h(px(s))
            .px_0()
            .py_0();
        if let Some(on) = self.toggled {
            b = b.toggled(on);
        }
        // No tooltip while its menu / flyout is open: dropping the managed
        // tooltip hides one that is showing (it would cover the menu).
        if let Some(tip) = self.tooltip.filter(|_| !open) {
            b = tip.apply(b);
        }
        if let Some(p) = self.tooltip_placement {
            b = b.tooltip_placement(p);
        }
        if let Some(f) = self.on_click {
            b = b.on_click(move |e, w, cx| f(e, w, cx));
        }
        b
    }
}

impl Selectable for IconButton {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }

    fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    fn is_open(&self) -> bool {
        self.open
    }
}

impl RenderOnce for IconButton {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if let Some((build, anchor)) = self.menu.take() {
            return menu_popover(self.id.clone(), anchor, self, build, window, cx);
        }
        let Some(build) = self.flyout.take() else {
            return self.button(false, cx).into_any_element();
        };
        let id = self.id.clone();
        let s = self.size.px();
        let open_state = window.use_keyed_state(child_id(&id, "flyout-open"), cx, |_, _| false);
        let nav = MenuNav::keyed(child_id(&id, "flyout-nav"), window, cx);
        let focus = nav.read(cx).focus.clone();
        let open = *open_state.read(cx);
        let caret = hsla(cx.ui().text_muted);

        let set_open = {
            let st = open_state.clone();
            move |v: bool, cx: &mut App| {
                st.update(cx, |o, cx| {
                    if *o != v {
                        *o = v;
                        cx.notify();
                    }
                })
            }
        };
        let toggle_right = set_open.clone();
        let toggle_strip = set_open.clone();
        let on_change = set_open.clone();
        let dismiss = set_open.clone();

        let button = self
            .button(open, cx)
            .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                cx.stop_propagation();
                toggle_right(!open, cx);
            });
        let strip = div()
            .id(child_id(&id, "flyout-strip"))
            .absolute()
            .right_0()
            .bottom_0()
            .w(px(10.))
            .h(px(s / 2.))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                toggle_strip(!open, cx);
            })
            .child(
                canvas(
                    |_, _, _| {},
                    move |b: Bounds<Pixels>, _, window, _| {
                        // A 4 px triangle tucked into the corner.
                        let (r, bt) = (b.right() - px(3.), b.bottom() - px(3.));
                        let mut p = PathBuilder::fill();
                        p.move_to(point(r - px(4.), bt));
                        p.line_to(point(r, bt));
                        p.line_to(point(r, bt - px(4.)));
                        p.close();
                        if let Ok(path) = p.build() {
                            window.paint_path(path, caret);
                        }
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            );
        let popover = Popover::new(child_id(&id, "flyout-pop"))
            .anchor(Anchor::TopLeft)
            .offset(px(0.))
            .appearance(false)
            .open(open)
            .track_focus(&focus)
            .on_open_change(move |o, _, cx| on_change(*o, cx))
            .trigger(AnchorPoint)
            .content(move |_, window, cx| {
                let dismiss = dismiss.clone();
                build(Menu::new(), window, cx)
                    .nav(nav.clone())
                    .on_dismiss(move |_, cx| dismiss(false, cx))
            });
        div()
            .relative()
            .flex_none()
            .size(px(s))
            .child(button)
            .child(strip)
            // The anchor sits at the button's top-right (plus a gap), so the
            // menu opens to the right, top-aligned with the button.
            .child(div().absolute().top_0().left(px(s + S1)).child(popover))
            .into_any_element()
    }
}

/// Zero-size popover trigger used to place a menu at a point.
#[derive(IntoElement)]
struct AnchorPoint;

impl Selectable for AnchorPoint {
    fn selected(self, _: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }
}

impl RenderOnce for AnchorPoint {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().size_0()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolbarKind {
    Text,
    Icon,
    Field,
}

/// Top-toolbar button: text, icon or the ticker "field"; optional leading
/// icon, caret and dropdown [`Menu`] (§3.3).
#[derive(IntoElement)]
pub struct ToolbarButton {
    id: ElementId,
    kind: ToolbarKind,
    label: Option<SharedString>,
    glyph: Option<Glyph>,
    leading: Option<Glyph>,
    caret: bool,
    size: ControlSize,
    selected: bool,
    open: bool,
    disabled: bool,
    outlined: bool,
    tooltip: Option<TooltipSpec>,
    menu: Option<MenuBuilder>,
    /// Popover anchor of the menu (TopLeft = below, left-aligned).
    menu_anchor: Anchor,
    on_click: Option<ClickHandler>,
}

impl ToolbarButton {
    fn base(id: ElementId, kind: ToolbarKind) -> Self {
        Self {
            id,
            kind,
            label: None,
            glyph: None,
            leading: None,
            caret: false,
            size: ControlSize::Md,
            selected: false,
            open: false,
            disabled: false,
            outlined: false,
            tooltip: None,
            menu: None,
            menu_anchor: Anchor::TopLeft,
            on_click: None,
        }
    }

    /// h 32, px 8, text 14.
    pub fn text(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        let mut b = Self::base(id.into(), ToolbarKind::Text);
        b.label = Some(label.into());
        b
    }

    /// Square 32 (wider with a caret).
    pub fn icon(id: impl Into<ElementId>, glyph: impl Into<Glyph>) -> Self {
        let mut b = Self::base(id.into(), ToolbarKind::Icon);
        b.glyph = Some(glyph.into());
        b
    }

    /// The ticker pill: `panel_alt` background, leading search icon.
    pub fn field(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        let mut b = Self::base(id.into(), ToolbarKind::Field);
        b.label = Some(label.into());
        b.leading = Some(Glyph::SEARCH);
        b
    }

    /// Leading icon (18 at Md, 16 at Sm), gap 6.
    pub fn leading(mut self, g: impl Into<Glyph>) -> Self {
        self.leading = Some(g.into());
        self
    }

    /// Trailing ChevronDown 14 (ChevronUp while the menu is open).
    pub fn caret(mut self) -> Self {
        self.caret = true;
        self
    }

    /// `Sm` (24) for the floating toolbar; default `Md` (32).
    pub fn size(mut self, s: ControlSize) -> Self {
        self.size = s;
        self
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// A bordered field-like button (section 2.10): `C_CONTROL` tall,
    /// radius `C_R_CONTROL`, chevron 18 (the comfortable dialog's
    /// "Template").
    pub fn outlined(mut self, on: bool) -> Self {
        self.outlined = on;
        self
    }

    pub fn tooltip(mut self, label: impl Into<SharedString>) -> Self {
        self.tooltip = Some(TooltipSpec {
            label: label.into(),
            action: None,
        });
        self
    }

    pub fn tooltip_action(
        mut self,
        label: impl Into<SharedString>,
        action: &dyn Action,
        context: Option<&str>,
    ) -> Self {
        self.tooltip = Some(TooltipSpec {
            label: label.into(),
            action: Some((
                Rc::new(action.boxed_clone()),
                context.map(|c| SharedString::from(c.to_owned())),
            )),
        });
        self
    }

    /// Opens a kit [`Menu`] below the button (TopLeft, offset `S1`). The
    /// menu is rebuilt on every render while open.
    pub fn menu(mut self, build: impl Fn(Menu, &mut Window, &mut App) -> Menu + 'static) -> Self {
        self.menu = Some(Rc::new(build));
        self
    }

    /// Where the menu opens: e.g. `Anchor::BottomRight` opens it above the
    /// button, right-aligned (status bar).
    pub fn menu_anchor(mut self, a: Anchor) -> Self {
        self.menu_anchor = a;
        self
    }

    /// Menu below the button, right edges aligned (buttons near the right
    /// end of a bar, so the menu stays on screen).
    pub fn menu_right(self) -> Self {
        self.menu_anchor(Anchor::TopRight)
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    fn button(self, cx: &App) -> Button {
        let t = cx.ui();
        let s = if self.outlined {
            C_CONTROL
        } else {
            self.size.px()
        };
        let icon = if self.size == ControlSize::Md {
            ICON_MD
        } else {
            self.size.icon()
        };
        let square = self.kind == ToolbarKind::Icon && !self.caret && self.label.is_none();
        let caret = self.caret.then(|| {
            glyph(
                if self.open {
                    Glyph::CHEVRON_UP
                } else {
                    Glyph::CHEVRON_DOWN
                },
                if self.outlined { ICON_MD } else { ICON_XS },
            )
            .color(hsla(t.text_muted))
        });
        let content = h_flex()
            .items_center()
            .gap(px(S1_5))
            .when_some(self.leading, |d, g| {
                d.child(glyph(g, icon).when(self.kind == ToolbarKind::Field, |g| {
                    g.color(hsla(t.text_muted))
                }))
            })
            .when_some(self.glyph, |d, g| d.child(glyph(g, g.svg_size(icon))))
            .when_some(self.label, |d, l| {
                d.child(
                    div()
                        .text_size(px(self.size.text()))
                        .whitespace_nowrap()
                        .when(self.kind == ToolbarKind::Field, |d| {
                            d.font_weight(FontWeight::MEDIUM)
                        })
                        .child(l),
                )
            })
            .when_some(caret, |d, c| d.child(div().ml(px(S0_5 - S1_5)).child(c)));
        let mut b = Button::new(self.id)
            .custom(variant(t, hsla(t.text), cx))
            .with_size(Size::Size(px(s)))
            .rounded(px(R_MD))
            .selected(self.selected || self.open)
            .disabled(self.disabled)
            .child(content)
            .h(px(s))
            .py_0()
            .map(|b| {
                if square {
                    b.w(px(s)).px_0()
                } else if self.kind == ToolbarKind::Icon {
                    b.px(px(S1))
                } else {
                    b.px(px(S2))
                }
            })
            .when(self.kind == ToolbarKind::Field, |b| b.bg(hsla(t.panel_alt)))
            .when(self.outlined, |b| {
                b.border_1()
                    .border_color(hsla(t.border))
                    .rounded(px(C_R_CONTROL))
                    .pl(px(S3))
                    .pr(px(S1))
            });
        // No tooltip over its own open menu (see `IconButton::button`).
        if let Some(tip) = self.tooltip.filter(|_| !self.open) {
            b = tip.apply(b);
        }
        if let Some(f) = self.on_click {
            b = b.on_click(move |e, w, cx| f(e, w, cx));
        }
        b
    }
}

impl Selectable for ToolbarButton {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }

    fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    fn is_open(&self) -> bool {
        self.open
    }
}

impl RenderOnce for ToolbarButton {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Some(build) = self.menu.take() else {
            return self.button(cx).into_any_element();
        };
        let anchor = self.menu_anchor;
        menu_popover(self.id.clone(), anchor, self, build, window, cx)
    }
}

/// A kit [`Menu`] in a popover on `trigger`, offset `S1`, with keyboard nav
/// state kept under the trigger's id.
fn menu_popover<T: Selectable + IntoElement + 'static>(
    id: ElementId,
    anchor: Anchor,
    trigger: T,
    build: MenuBuilder,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let nav = MenuNav::keyed(child_id(&id, "menu-nav"), window, cx);
    let focus = nav.read(cx).focus.clone();
    Popover::new(child_id(&id, "menu-pop"))
        .anchor(anchor)
        .offset(px(S1))
        .appearance(false)
        .track_focus(&focus)
        .trigger(trigger)
        .content(move |_, window, cx| {
            let pop = cx.entity();
            build(Menu::new(), window, cx)
                .nav(nav.clone())
                .on_dismiss(move |window, cx| pop.update(cx, |p, cx| p.dismiss(window, cx)))
        })
        .into_any_element()
}

/// Filter chip: h 24, full radius, text 12; selected = `selection` bg +
/// `text`, otherwise `text_muted` (§1.4 S3).
#[derive(IntoElement)]
pub struct Chip {
    id: ElementId,
    label: SharedString,
    selected: bool,
    on_click: Option<ClickHandler>,
}

impl Chip {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected: false,
            on_click: None,
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
}

impl RenderOnce for Chip {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        let fg = if self.selected { t.text } else { t.text_muted };
        Button::new(self.id)
            .custom(variant(t, hsla(fg), cx))
            .with_size(Size::Size(px(CONTROL_SM)))
            .rounded(px(R_FULL))
            .selected(self.selected)
            .child(div().text_size(px(TEXT_SMALL)).child(self.label))
            .h(px(CONTROL_SM))
            .px(px(S3))
            .py_0()
            .when_some(self.on_click, |b, f| {
                b.on_click(move |e, w, cx| f(e, w, cx))
            })
    }
}
