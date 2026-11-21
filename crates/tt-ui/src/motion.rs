//! UI motion (§2.9): short, interruptible (transitions retarget from the
//! current value), EaseOut, and never on data. gpui-kit snaps to the end
//! state when the OS asks for reduced motion.

use crate::tokens::{MOTION_FAST, MOTION_NORMAL, ms};
use gpui_kit::base::{Easing, Transition, TransitionId, transition};
use gpui_kit::*;

/// Standard snappy transition of `ms` milliseconds.
pub fn snappy(duration_ms: u64) -> Transition {
    Transition::new(ms(duration_ms)).easing(Easing::EaseOut)
}

/// `FAST` (90 ms) EaseOut: in-chart control fades.
pub fn fast() -> Transition {
    snappy(MOTION_FAST)
}

/// `NORMAL` (130 ms) EaseOut: menus, tab fill.
pub fn normal() -> Transition {
    snappy(MOTION_NORMAL)
}

/// Opacity easing toward 1 (`shown`) or 0, in both directions. Render the
/// element until this returns 0 (`.invisible()` below 0.01), so the fade
/// out is not cut.
pub fn fade_fast(
    id: impl Into<TransitionId>,
    shown: bool,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    transition(id, if shown { 1.0f32 } else { 0.0 }, fast(), window, cx)
}
