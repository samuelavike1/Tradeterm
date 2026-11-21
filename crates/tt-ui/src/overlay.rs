//! Floating surfaces (§2.6): `overlay_bg`, 1 px `border`, `R_LG`,
//! `shadow_md`. [`Overlay::floating`] is the floating drawing toolbar frame
//! (drag handle on the left, `S0_5` padding); the default is the perf
//! overlay / panel frame (`S3` padding).

use crate::glyph::{Glyph, glyph};
use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::StyledExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

#[derive(IntoElement)]
pub struct Overlay {
    id: Option<ElementId>,
    style: StyleRefinement,
    children: Vec<AnyElement>,
    floating: bool,
}

impl Default for Overlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Overlay {
    pub fn new() -> Self {
        Self {
            id: None,
            style: StyleRefinement::default(),
            children: Vec::new(),
            floating: false,
        }
    }

    /// Floating toolbar: a row with a grip handle (`GripVertical` 14) first,
    /// `S0_5` padding and gap. The caller owns dragging (the handle is the
    /// first child; wire `on_drag` on the overlay itself).
    pub fn floating(mut self) -> Self {
        self.floating = true;
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }
}

impl ParentElement for Overlay {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Overlay {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Overlay {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        let base = div()
            .bg(hsla(t.overlay_bg))
            .border_1()
            .border_color(hsla(t.border))
            .rounded(px(R_LG))
            .shadow_md()
            .text_size(px(TEXT_SMALL))
            .text_color(hsla(t.text))
            .occlude();
        let base = if self.floating {
            base.flex()
                .flex_row()
                .items_center()
                .p(px(S0_5))
                .gap(px(S0_5))
                .child(
                    div()
                        .flex_none()
                        .w(px(CONTROL_SM / 2. + S0_5))
                        .h(px(CONTROL_SM))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_grab()
                        .text_color(hsla(t.text_muted))
                        .child(glyph(Glyph::GRIP_VERTICAL, ICON_XS)),
                )
        } else {
            base.flex().flex_col().p(px(S3)).gap(px(S0_5))
        };
        base.children(self.children)
            .refine_style(&self.style)
            .map(|d| match self.id {
                Some(id) => d.id(id).into_any_element(),
                None => d.into_any_element(),
            })
    }
}

/// The chart's top-right pane-controls gutter (§2.7).
pub struct PaneControls;

impl PaneControls {
    /// Width of a row of `buttons` `IconButton::Sm` with `S0_5` gaps and
    /// padding. The floating toolbar never enters this gutter.
    pub fn width(buttons: usize) -> Pixels {
        px(Self::width_f32(buttons))
    }

    pub fn width_f32(buttons: usize) -> f32 {
        if buttons == 0 {
            return 0.0;
        }
        buttons as f32 * CONTROL_SM + (buttons as f32 - 1.0) * S0_5 + 2.0 * S0_5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn pane_controls_width() {
        assert_eq!(PaneControls::width_f32(0), 0.0);
        assert_eq!(PaneControls::width_f32(1), 28.0);
        assert_eq!(PaneControls::width_f32(5), 5.0 * 24.0 + 4.0 * 2.0 + 4.0);
    }
}
