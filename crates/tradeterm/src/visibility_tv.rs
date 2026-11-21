//! TradingView's Visibility tab rows, shared by the drawing and indicator
//! settings dialogs (docs/specs/drawings-tv-ref.md, indicators-tv-ref.md):
//! Ticks, then per unit `[x] Label [min] ——o==o—— [max]`, then Ranges.

use gpui_kit::component::input::InputState;
use gpui_kit::*;
use tt_chart::drawings::form::unit_label;
use tt_core::TimeUnit;
use tt_ui::prelude::*;

/// Unit rows' label column (TradingView: label + 20).
pub const W_VIS_LABEL: f32 = 96.0;

/// The dialog width TradingView uses while the Visibility tab is shown.
pub const W_VIS_DIALOG: f32 = 454.0;

/// A lone checkbox row (Ticks, Ranges).
pub fn check_row(
    id: impl Into<ElementId>,
    label: &'static str,
    on: bool,
    f: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> FormRow {
    FormRow::check(id, label, on, f).comfortable()
}

/// One unit: its checkbox, then `[min] slider [max]` editing `value` inside
/// `range` (dimmed while unchecked, still editable).
#[allow(clippy::too_many_arguments)]
pub fn unit_row(
    prefix: &str,
    u: TimeUnit,
    on: bool,
    lo: &Entity<InputState>,
    hi: &Entity<InputState>,
    range: (f32, f32),
    value: (f32, f32),
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    on_change: impl Fn(u16, u16, &mut Window, &mut App) + 'static,
) -> FormRow {
    let label = unit_label(u);
    FormRow::check(
        SharedString::from(format!("{prefix}-vis-{label}")),
        label,
        on,
        on_toggle,
    )
    .comfortable()
    .label_width(W_VIS_LABEL)
    .control(
        RangeField::new(
            SharedString::from(format!("{prefix}-vis-range-{label}")),
            lo,
            hi,
            range,
            value,
        )
        .disabled(!on)
        .on_change(move |a, b, w, cx| {
            let (a, b) = (a.round() as u16, b.round() as u16);
            on_change(a.min(b), b.max(a), w, cx)
        }),
    )
}
