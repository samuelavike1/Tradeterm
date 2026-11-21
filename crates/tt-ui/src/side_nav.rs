//! TradingView's chart Settings frame on the comfortable density (§2.10),
//! measured on tradingview.com (docs/specs/chart-settings-tv-ref.md):
//!
//! - [`SideNavFrame`]: a 236 px left nav (40 px items with a 28 px icon
//!   box, radius 8) and a page body that fits its content, scrolling past
//!   the window's height. Use with `DialogOpts::comfortable().width(
//!   C_W_SETTINGS_DIALOG)` and `DialogFooter::comfortable()`.
//! - [`SectionTitle`]: the 11 px uppercase muted section title.
//! - [`sub_row`]: a 42 px follow-up row (TradingView's "Inputs" under
//!   "Titles", or a second select under a row's first).

use crate::button::child_id;
use crate::glyph::{Glyph, glyph};
use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;

/// TradingView's chart Settings dialog width.
pub const C_W_SETTINGS_DIALOG: f32 = 750.0;
/// The left nav column (padding 8 10 0 20 around 206 px items).
pub const C_W_SIDE_NAV: f32 = 236.0;
/// A nav item's height.
pub const C_H_SIDE_NAV_ITEM: f32 = 40.0;
/// The nav item's icon box.
pub const C_SIDE_NAV_ICON_BOX: f32 = 28.0;
/// A follow-up row's height.
pub const C_SUB_ROW: f32 = 42.0;
/// The indent of a follow-up checkbox (checkbox + gap).
pub const C_SUB_INDENT: f32 = C_CHECKBOX + C_GAP;
/// Section titles: 11 px uppercase (`C_TEXT_SECTION` is the popovers' 12).
pub const C_TEXT_SECTION_TITLE: f32 = 11.0;

const NAV_PAD_TOP: f32 = 8.0;
const NAV_PAD_LEFT: f32 = 20.0;
const NAV_PAD_RIGHT: f32 = 10.0;
/// Page body left padding (labels 10 px right of the nav).
const BODY_PAD_LEFT: f32 = 10.0;

type NavHandler = Rc<dyn Fn(&'static str, &mut Window, &mut App)>;

/// Left nav + page body (see the module docs).
#[derive(IntoElement)]
pub struct SideNavFrame {
    id: ElementId,
    items: Vec<(&'static str, SharedString, Option<Glyph>)>,
    selected: Option<&'static str>,
    on_change: Option<NavHandler>,
    body: Option<AnyElement>,
}

impl SideNavFrame {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            selected: None,
            on_change: None,
            body: None,
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

    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }
}

impl RenderOnce for SideNavFrame {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        // The dialog starts a tenth of the window down; keep its footer
        // in the window (like TradingView, which caps at the window).
        let view_h = f32::from(window.viewport_size().height);
        let nav_h = NAV_PAD_TOP + self.items.len() as f32 * C_H_SIDE_NAV_ITEM;
        let max = (view_h * 0.9 - C_H_HEADER - C_H_FOOTER - S6).max(nav_h);
        let mut nav = v_flex()
            .flex_none()
            .w(px(C_W_SIDE_NAV))
            .pt(px(NAV_PAD_TOP))
            .pl(px(NAV_PAD_LEFT))
            .pr(px(NAV_PAD_RIGHT));
        for (id, label, g) in self.items {
            let selected = self.selected == Some(id);
            let f = self.on_change.clone();
            nav = nav.child(
                h_flex()
                    .id(child_id(&self.id, id))
                    .w_full()
                    .h(px(C_H_SIDE_NAV_ITEM))
                    .pl(px(S3))
                    .pr(px(S4))
                    .gap(px(S1))
                    .items_center()
                    .rounded(px(C_R_CONTROL))
                    .cursor_pointer()
                    .text_size(px(TEXT_BODY))
                    .text_color(hsla(t.text))
                    .when(selected, |d| {
                        d.bg(hsla(t.selection)).font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!selected, |d| d.hover(|s| s.bg(hsla(t.hover))))
                    .child(
                        div()
                            .flex_none()
                            .size(px(C_SIDE_NAV_ICON_BOX))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when_some(g, |d, g| d.child(glyph(g, ICON_MD + 2.0))),
                    )
                    .child(div().whitespace_nowrap().child(label))
                    .on_click(move |_, w, cx| {
                        if let Some(f) = &f {
                            f(id, w, cx)
                        }
                    }),
            );
        }
        h_flex()
            .id(self.id.clone())
            .w_full()
            .items_start()
            .child(nav)
            .child(
                v_flex()
                    .id(child_id(&self.id, "body"))
                    .flex_1()
                    .min_w_0()
                    .min_h(px(nav_h))
                    .max_h(px(max))
                    .overflow_y_scroll()
                    .pl(px(BODY_PAD_LEFT))
                    .pr(px(C_PAD_X))
                    .py(px(C_PAD_Y))
                    .children(self.body),
            )
    }
}

/// A page's section title: 11 px uppercase `text_muted`, 25 px under the
/// previous row (9 under the body top when `first`), rows 10 px under it.
#[derive(IntoElement)]
pub struct SectionTitle {
    title: SharedString,
    first: bool,
}

impl SectionTitle {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            first: false,
        }
    }

    /// The page's first title (less space above).
    pub fn first(mut self, first: bool) -> Self {
        self.first = first;
        self
    }
}

impl RenderOnce for SectionTitle {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        div()
            .pt(px(if self.first { 9.0 } else { 25.0 }))
            .pb(px(10.0))
            .text_size(px(C_TEXT_SECTION_TITLE))
            .line_height(px(13.0))
            .text_color(hsla(t.text_muted))
            .child(self.title.to_uppercase())
    }
}

/// A `C_SUB_ROW` follow-up row: `el` starts `indent` px from the left.
pub fn sub_row(indent: f32, el: impl IntoElement) -> Div {
    h_flex()
        .h(px(C_SUB_ROW))
        .pl(px(indent))
        .items_center()
        .child(el)
}
