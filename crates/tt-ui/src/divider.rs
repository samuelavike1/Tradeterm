//! 1 px `border` separators (§1.1 A10).

use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::*;

#[derive(IntoElement)]
pub struct Divider {
    vertical: bool,
    inset: f32,
    length: f32,
}

impl Divider {
    /// Toolbar separator: 1 × 20, margin `S1` on both sides.
    pub fn vertical() -> Self {
        Self {
            vertical: true,
            inset: S1,
            length: 20.0,
        }
    }

    /// Menu / list separator: full width, margin `S1` above and below.
    pub fn horizontal() -> Self {
        Self {
            vertical: false,
            inset: 0.0,
            length: 0.0,
        }
    }

    /// Horizontal: side inset; vertical: side margin.
    pub fn inset(mut self, px: f32) -> Self {
        self.inset = px;
        self
    }

    /// Vertical length (default 20).
    pub fn length(mut self, px: f32) -> Self {
        self.length = px;
        self
    }
}

impl RenderOnce for Divider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = hsla(cx.ui().border);
        if self.vertical {
            div()
                .flex_none()
                .w(px(1.))
                .h(px(self.length))
                .mx(px(self.inset))
                .bg(c)
        } else {
            div()
                .flex_none()
                .h(px(1.))
                .my(px(S1))
                .mx(px(self.inset))
                .bg(c)
        }
    }
}
