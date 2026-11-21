//! Numeric design tokens (docs/specs/ui-guidelines.md §2.1–2.4, §2.9).
//!
//! Plain `f32` pixels and `u64` milliseconds, no toolkit types, so the
//! canvas (`tt-chart`) and the gpui kit (`tt-ui`) read the same numbers.
//! Sizes in feature code come from here (through `tt_ui::tokens`), nowhere
//! else.

// ---- spacing: 4 px grid with a 2 px half step (§2.1) ------------------------

/// Icon-to-caret, gaps inside a tight cluster.
pub const S0_5: f32 = 2.0;
/// Between toolbar buttons, menu edge padding.
pub const S1: f32 = 4.0;
/// Icon to label.
pub const S1_5: f32 = 6.0;
/// Row horizontal padding, toolbar padding.
pub const S2: f32 = 8.0;
/// Between form controls in a row.
pub const S3: f32 = 12.0;
/// Between groups (status bar), section top margin.
pub const S4: f32 = 16.0;
/// Dialog horizontal padding.
pub const S5: f32 = 20.0;
/// Empty-state padding.
pub const S6: f32 = 24.0;

// ---- radii (§2.2) -----------------------------------------------------------

/// Canvas axis tags only.
pub const R_XS: f32 = 2.0;
/// Swatches, Kbd, tags, legend buttons, xs (20 px) controls.
pub const R_SM: f32 = 4.0;
/// Buttons, rows, inputs, tab pills, nav items (= gpui-component `radius`).
pub const R_MD: f32 = 6.0;
/// Menus / popovers, floating toolbars, overlays.
pub const R_LG: f32 = 8.0;
/// Dialogs (= gpui-component `radius_lg`).
pub const R_XL: f32 = 10.0;
/// Chips, status dot, avatar badges.
pub const R_FULL: f32 = 9999.0;

// ---- typography (§2.3) ------------------------------------------------------

/// 11 px, weight 500, uppercase: section headers, Kbd, tags, badge initials.
pub const TEXT_CAPTION: f32 = 11.0;
/// Status bar, secondary lines, floating-toolbar labels, A/L/%, errors.
pub const TEXT_SMALL: f32 = 12.0;
/// Tab strip, dense lists.
pub const TEXT_DENSE: f32 = 13.0;
/// Default: toolbar labels, menus, form labels/values, buttons.
pub const TEXT_BODY: f32 = 14.0;
/// Dialog titles (weight 600).
pub const TEXT_TITLE: f32 = 16.0;

// ---- icons (§2.4) -----------------------------------------------------------

/// Inside 20 px controls, carets, legend canvas buttons, tab close.
pub const ICON_XS: f32 = 14.0;
/// Menus, lists, nav tabs, in-chart 24 px buttons, input prefix.
pub const ICON_SM: f32 = 16.0;
/// Top toolbar (32 px buttons), title bar buttons.
pub const ICON_MD: f32 = 18.0;
/// Left drawing toolbar (36 px buttons).
pub const ICON_LG: f32 = 20.0;

// ---- control heights (§2.4) -------------------------------------------------

pub const H_TITLE_BAR: f32 = 34.0;
pub const H_TAB: f32 = 28.0;
pub const H_TOOLBAR: f32 = 40.0;
pub const H_STATUS_BAR: f32 = 28.0;
/// A/L/%, swatches in rows, row actions (star, ×), canvas legend buttons.
pub const CONTROL_XS: f32 = 20.0;
/// In-chart overlay buttons, chips, title bar buttons.
pub const CONTROL_SM: f32 = 24.0;
/// Inputs, selects, color fields inside forms.
pub const H_FIELD: f32 = 28.0;
/// Top toolbar buttons, dialog footer buttons.
pub const CONTROL_MD: f32 = 32.0;
/// Drawing toolbar buttons.
pub const CONTROL_LG: f32 = 36.0;
pub const H_MENU_ROW: f32 = 32.0;
pub const H_SECTION: f32 = 28.0;
/// Nav tabs, indicator list, form rows.
pub const H_LIST_ROW: f32 = 36.0;
/// Symbol search results.
pub const H_LIST_ROW_TALL: f32 = 48.0;

// ---- widths (§2.4) ----------------------------------------------------------

pub const W_DRAWING_TOOLBAR: f32 = 48.0;
pub const W_MENU_SM: f32 = 200.0;
pub const W_MENU_MD: f32 = 240.0;
/// Only with a shortcut column.
pub const W_MENU_LG: f32 = 280.0;
pub const W_DIALOG_SM: f32 = 480.0;
pub const W_DIALOG_MD: f32 = 680.0;
pub const W_DIALOG_LG: f32 = 800.0;
/// Dialog left nav.
pub const W_NAV: f32 = 200.0;
pub const W_LABEL_SM: f32 = 160.0;
pub const W_LABEL_LG: f32 = 220.0;
pub const W_FIELD_NUMBER: f32 = 120.0;
pub const W_FIELD_SELECT: f32 = 160.0;

// ---- dialog geometry (§2.6) -------------------------------------------------

pub const H_DIALOG_BODY_SM: f32 = 280.0;
pub const H_DIALOG_BODY_MD: f32 = 460.0;
pub const H_DIALOG_BODY_LG: f32 = 480.0;
pub const H_DIALOG_HEADER: f32 = 52.0;
pub const H_DIALOG_FOOTER: f32 = 60.0;
/// Minimum width of dialog footer buttons.
pub const W_DIALOG_BUTTON: f32 = 72.0;

// ---- motion (§2.9), milliseconds --------------------------------------------

/// Hover, pressed, selection changes.
pub const MOTION_INSTANT: u64 = 0;
/// In-chart control fades (both directions), caret rotation.
pub const MOTION_FAST: u64 = 90;
/// Menu / popover open, tab active fill.
pub const MOTION_NORMAL: u64 = 130;
/// Dialog entrance.
pub const MOTION_SLOW: u64 = 200;
/// Toggles and switches (spring).
pub const MOTION_SPRING_CONTROL: u64 = 140;
/// Moves (spring).
pub const MOTION_SPRING_MOVE: u64 = 200;
/// First tooltip; 0 while warm.
pub const TOOLTIP_DELAY: u64 = 450;

/// Control size = icon + 2 × inset; the insets per size (§2.4 rule).
pub const fn control_for_icon(icon: f32) -> f32 {
    if icon <= ICON_XS {
        icon + 2.0 * 3.0
    } else if icon <= ICON_SM {
        icon + 2.0 * 4.0
    } else if icon <= ICON_MD {
        icon + 2.0 * 7.0
    } else {
        icon + 2.0 * 8.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_sizes_follow_the_inset_rule() {
        assert_eq!(control_for_icon(ICON_XS), CONTROL_XS);
        assert_eq!(control_for_icon(ICON_SM), CONTROL_SM);
        assert_eq!(control_for_icon(ICON_MD), CONTROL_MD);
        assert_eq!(control_for_icon(ICON_LG), CONTROL_LG);
    }

    #[test]
    fn drawing_toolbar_gutters() {
        // 36 px buttons with 6 px gutters.
        assert_eq!(W_DRAWING_TOOLBAR - CONTROL_LG, 2.0 * S1_5);
    }
}
