//! Design tokens as gpui values: `tt_theme::metrics` re-exported, plus
//! [`PxExt`] to turn them into [`Pixels`] (`tokens::H_TOOLBAR.px()`).

use gpui_kit::{Pixels, px};
use std::time::Duration;

pub use tt_theme::metrics::*;

/// `f32` token → gpui [`Pixels`].
pub trait PxExt {
    fn px(self) -> Pixels;
}

impl PxExt for f32 {
    #[inline]
    fn px(self) -> Pixels {
        px(self)
    }
}

/// A motion token (milliseconds) as a [`Duration`].
#[inline]
pub const fn ms(v: u64) -> Duration {
    Duration::from_millis(v)
}
