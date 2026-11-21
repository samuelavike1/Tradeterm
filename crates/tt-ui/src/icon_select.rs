//! TradingView's plot type control (indicator settings, Style tab): a
//! bordered `C_CONTROL` square showing the current option's icon; its menu
//! has an optional switch row on top ("Price line"), a separator and the
//! options with their icons (docs/specs/indicators-tv-ref.md).
//!
//! Stateless: the caller passes the current option / switch and gets every
//! change through the callbacks.

use crate::button::child_id;
use crate::glyph::{Glyph, glyph};
use crate::menu::{MenuRow, MenuWidth};
use crate::picker::{boxed_button, menu_with};
use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;

type IndexHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type BoolHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// Width of the switch track (TradingView: 38 × 20, knob 14).
const SWITCH_W: f32 = 38.0;
const SWITCH_H: f32 = 20.0;
const SWITCH_KNOB: f32 = 14.0;

/// An on / off switch (the plot type menu's "Price line").
pub fn switch(on: bool, cx: &App) -> Div {
    let t = cx.ui();
    let track = if on {
        hsla(t.text)
    } else {
        hsla(t.text_disabled)
    };
    let knob = hsla(t.overlay_bg);
    h_flex()
        .flex_none()
        .w(px(SWITCH_W))
        .h(px(SWITCH_H))
        .rounded(px(SWITCH_H / 2.0))
        .bg(track)
        .px(px((SWITCH_H - SWITCH_KNOB) / 2.0))
        .items_center()
        .when(on, |d| d.justify_end())
        .child(
            div()
                .size(px(SWITCH_KNOB))
                .rounded(px(SWITCH_KNOB / 2.0))
                .bg(knob),
        )
}

/// A `C_CONTROL` square select whose options are icons with labels.
#[derive(IntoElement)]
pub struct IconSelect {
    id: ElementId,
    options: Vec<(Glyph, SharedString)>,
    selected: usize,
    switch: Option<(SharedString, bool, BoolHandler)>,
    on_change: Option<IndexHandler>,
}

impl IconSelect {
    pub fn new(
        id: impl Into<ElementId>,
        options: impl IntoIterator<Item = (Glyph, impl Into<SharedString>)>,
        selected: usize,
    ) -> Self {
        Self {
            id: id.into(),
            options: options.into_iter().map(|(g, l)| (g, l.into())).collect(),
            selected,
            switch: None,
            on_change: None,
        }
    }

    /// A switch row on top of the menu (stays open when flipped).
    pub fn switch(
        mut self,
        label: impl Into<SharedString>,
        on: bool,
        f: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.switch = Some((label.into(), on, Rc::new(f)));
        self
    }

    /// The picked option's index.
    pub fn on_change(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for IconSelect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui().clone();
        let (g, label) = self
            .options
            .get(self.selected)
            .cloned()
            .unwrap_or((Glyph::CHECK, SharedString::default()));
        let trigger = boxed_button(child_id(&self.id, "btn"), C_CONTROL, &t, cx)
            .child(glyph(g, ICON_MD).color(hsla(t.text)))
            .tooltip(label);
        let (options, now, f, sw) = (self.options, self.selected, self.on_change, self.switch);
        let row_id = self.id.clone();
        menu_with(self.id, trigger, window, cx, move |mut menu, cx| {
            menu = menu.width(MenuWidth::Md);
            if let Some((label, on, f)) = &sw {
                let (on, f) = (*on, f.clone());
                let text = hsla(cx.ui().text);
                menu = menu
                    .child(
                        h_flex()
                            .id(child_id(&row_id, "switch"))
                            .h(px(C_H_SEGMENT))
                            .px(px(S3))
                            .items_center()
                            .justify_between()
                            .cursor_pointer()
                            .text_size(px(TEXT_BODY))
                            .text_color(text)
                            .child(label.clone())
                            .child(switch(on, cx))
                            .on_click(move |_, w, cx| f(!on, w, cx)),
                    )
                    .separator();
            }
            for (i, (g, label)) in options.iter().enumerate() {
                let f = f.clone();
                menu = menu.row(
                    MenuRow::new(("icon-select-opt", i), label.clone())
                        .icon(*g)
                        .checked(i == now)
                        .on_select(move |w, cx| {
                            if let Some(f) = &f {
                                f(i, w, cx)
                            }
                        }),
                );
            }
            menu
        })
    }
}
