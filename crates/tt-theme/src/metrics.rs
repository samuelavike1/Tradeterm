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
/// Left drawing toolbar Lucide glyphs, in-panel list icons.
pub const ICON_LG: f32 = 20.0;
/// Our 1-stroke SVG icons (chart types, drawing tools) where a Lucide glyph
/// would be 18–20: TradingView draws its icons in a 28 box with their own
/// inner margin. Smaller slots use 24 (16) and 20 (14), see
/// `Glyph::svg_size`.
pub const ICON_SVG: f32 = 28.0;

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
/// Large menu rows with 28 px icons: the chart-type menu and the drawing
/// flyouts (TradingView-measured, 2026-11: 40 tall, radius 6, icon at x 4,
/// label at x 36, text 14; groups split by a 13 px separator 6 + 1 + 6).
pub const H_MENU_ROW_LG: f32 = 40.0;
/// Left drawing toolbar buttons: 34 square hover / toggled background,
/// 4 px apart (38 px pitch), TradingView-measured.
pub const CONTROL_DRAWING: f32 = 34.0;
pub const H_SECTION: f32 = 28.0;
/// Nav tabs, indicator list, form rows.
pub const H_LIST_ROW: f32 = 36.0;
/// Symbol search results.
pub const H_LIST_ROW_TALL: f32 = 48.0;

// ---- widths (§2.4) ----------------------------------------------------------

/// TradingView's left toolbar: 52 wide, 34 px buttons 9 px from each edge.
pub const W_DRAWING_TOOLBAR: f32 = 52.0;
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

// ---- comfortable density: dialogs and floating toolbars (§2.10) -------------
//
// Measured on TradingView (2026-11, CSS px): the drawing settings dialog and
// the drawing floating toolbar. The app chrome (top toolbar, menus, lists)
// keeps the compact tokens above; see docs/specs/ui-guidelines.md §2.10.

/// Dialog title: 20 px, weight 600, line height 28.
pub const C_TEXT_TITLE: f32 = 20.0;
pub const C_TITLE_LINE: f32 = 28.0;
/// Dialog tab labels: 16 px, weight 600.
pub const C_TEXT_TAB: f32 = 16.0;
/// Footer Cancel / Ok labels.
pub const C_TEXT_BUTTON: f32 = 16.0;
/// Popover section titles ("Opacity", "Thickness"): 12 px, `text_muted`.
pub const C_TEXT_SECTION: f32 = 12.0;
/// Dialog header (title row) height; the title sits 17 px from the top.
pub const C_H_HEADER: f32 = 68.0;
/// Header buttons (rename ✎, close ×): 34 square, icon 18.
pub const C_HEADER_BUTTON: f32 = 34.0;
/// Tab strip: 32 tall (labels + 8 px bottom padding + underline).
pub const C_H_TABS: f32 = 32.0;
/// Gap between tab labels.
pub const C_TAB_GAP: f32 = 24.0;
/// Active tab underline (radius 2).
pub const C_TAB_UNDERLINE: f32 = 4.0;
/// Inputs, selects, footer buttons, toggle buttons inside dialogs.
pub const C_CONTROL: f32 = 34.0;
/// A form row: 8 + control + 8.
pub const C_ROW: f32 = 50.0;
/// Between controls in a row.
pub const C_GAP: f32 = 8.0;
/// Dialog horizontal padding (header, tabs, body, footer).
pub const C_PAD_X: f32 = 20.0;
/// Dialog body vertical padding.
pub const C_PAD_Y: f32 = 16.0;
/// Label column: label + this gap to the controls.
pub const C_LABEL_GAP: f32 = 20.0;
/// Footer: 16 + 34 + 16.
pub const C_H_FOOTER: f32 = 66.0;
/// Minimum dialog width (the dialog grows with its widest tab).
pub const C_W_DIALOG: f32 = 380.0;
/// Number fields and selects.
pub const C_W_FIELD: f32 = 100.0;
/// Multiline text area height.
pub const C_H_TEXTAREA: f32 = 100.0;
/// Checkbox box (radius 3); 8 px to its label.
pub const C_CHECKBOX: f32 = 18.0;
/// Dialog surface radius.
pub const C_R_DIALOG: f32 = 20.0;
/// Inputs, selects, footer buttons.
pub const C_R_CONTROL: f32 = 8.0;
/// Square swatch / toggle buttons (line picker, color, B, I).
pub const C_R_TOGGLE: f32 = 6.0;
/// Swatch inside a 34 px swatch button (radius 3).
pub const C_SWATCH: f32 = 24.0;
/// Two-handle range slider: track 10 tall (radius 5), handles 12.
pub const C_SLIDER_TRACK: f32 = 10.0;
pub const C_SLIDER_HANDLE: f32 = 12.0;
pub const C_W_SLIDER: f32 = 100.0;
/// Palette popover: 10 columns of 17 px swatches on a 22.6 px pitch.
pub const C_PALETTE_SWATCH: f32 = 17.0;
pub const C_PALETTE_GAP: f32 = 5.0;
/// Popover width (palette, line picker).
pub const C_W_POPOVER: f32 = 248.0;
/// Segmented options (thickness, line style) in the line picker.
pub const C_H_SEGMENT: f32 = 32.0;

/// Floating drawing toolbar: 38 tall, 38 square buttons with no gaps,
/// a 24 px grip, radius 6.
pub const FT_HEIGHT: f32 = 38.0;
pub const FT_BUTTON: f32 = 38.0;
pub const FT_GRIP: f32 = 24.0;
pub const FT_RADIUS: f32 = 6.0;
/// Icons in the floating toolbar (TradingView draws them in a 28 box).
pub const FT_ICON: f32 = 20.0;
/// Color-bar buttons: a 16 icon 7 px from the top and a 22 × 4 bar
/// (radius 2) 27 px from the top.
pub const FT_BAR_ICON: f32 = 16.0;
pub const FT_BAR_W: f32 = 22.0;
pub const FT_BAR_H: f32 = 4.0;
/// "— 1px" line-width button.
pub const FT_W_WIDTH_BUTTON: f32 = 68.0;

// ---- search dialogs: symbol search, indicator picker (§2.10) ----------------
//
// Measured on TradingView (2026-11, CSS px, computed styles): the symbol
// search and "Indicators, metrics, and strategies". Screenshots of
// TradingView taken in a zoomed browser (devicePixelRatio 1.22) look 22 %
// bigger; these are the real sizes at 100 %.

/// Symbol search: 840 × 680, radius `C_R_DIALOG`, no title row.
pub const SS_W_DIALOG: f32 = 840.0;
pub const SS_H_DIALOG: f32 = 680.0;
/// Search header: 16 top / 4 bottom padding around a 40 px field row.
pub const SS_PAD_TOP: f32 = 16.0;
pub const SS_PAD_BOTTOM: f32 = 4.0;
/// A search field row (header or boxed field).
pub const SEARCH_H_FIELD: f32 = 40.0;
/// Query text in search fields.
pub const SEARCH_TEXT: f32 = 16.0;
/// Magnifier: a 28 box with a 20 icon.
pub const SEARCH_ICON_BOX: f32 = 28.0;
pub const SEARCH_ICON: f32 = 20.0;
/// "Clear" text button: 28 tall, 7 px side padding, radius 8.
pub const SEARCH_H_CLEAR: f32 = 28.0;
/// Filter pills: 28 tall, fully round, 14 px, 8 apart; 16 px to the list.
pub const PILL_H: f32 = 28.0;
pub const PILL_PAD_X: f32 = 12.0;
pub const PILL_GAP: f32 = 8.0;
pub const SS_PILLS_BOTTOM: f32 = 16.0;
/// Result rows: 58 tall (8 + 42 + 8), hover fill inset 12 from the dialog
/// edge with radius 8, content 20 in.
pub const H_LIST_ROW_LG: f32 = 58.0;
pub const LIST_LG_INSET: f32 = 12.0;
/// Symbol logo in a result row; logo to text 16.
pub const LOGO_LG: f32 = 36.0;
/// Row title 16 / 24, second line 13 / 18.
pub const TEXT_ROW_LG: f32 = 16.0;
pub const LINE_ROW_LG: f32 = 24.0;
pub const TEXT_ROW_LG_SECONDARY: f32 = 13.0;
pub const LINE_ROW_LG_SECONDARY: f32 = 18.0;
/// Type badge ("SPOT CRYPTO"): 8 px bold uppercase, 12 tall, radius 3.
pub const TEXT_BADGE: f32 = 8.0;
pub const H_BADGE: f32 = 12.0;
pub const R_BADGE: f32 = 3.0;
/// Exchange logo at the right of a result row; 24 px from the text column.
pub const LOGO_EXCHANGE: f32 = 24.0;
/// Indicator picker surface radius (smaller than other dialogs').
pub const PICKER_R_DIALOG: f32 = 12.0;
/// Text on chart drawings: gap kept on each side of text that sits on a
/// line (the line breaks behind it).
pub const TEXT_LINE_GAP: f32 = 6.0;

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
        // 34 px buttons with 9 px gutters (TradingView), holding a 28 icon.
        assert_eq!(W_DRAWING_TOOLBAR - CONTROL_DRAWING, 18.0);
        assert_eq!(CONTROL_DRAWING - ICON_SVG, 2.0 * 3.0);
    }
}

// ---- alerts: "Create alert" dialog and the Alerts panel (docs/specs/alerts.md)
//
// Measured on TradingView (2026-11, CSS px, computed styles).

/// "Create alert" dialog: 480 wide, radius `C_R_DIALOG`, 20 px padding.
pub const AL_W_DIALOG: f32 = 480.0;
/// Label column ("Condition", "Trigger", …) and the control column.
pub const AL_W_LABEL: f32 = 131.0;
pub const AL_W_CONTROL: f32 = 305.0;
/// Half of the control column: "Value" select | number field, 8 apart.
pub const AL_W_HALF: f32 = 148.0;
/// Summary rows (Trigger, Expiration, Message, Notifications): 30 tall,
/// label 14 muted, value 14 with a chevron.
pub const AL_H_ROW: f32 = 30.0;
/// Alerts panel: 300 wide; content 16 from its edges.
pub const AL_W_PANEL: f32 = 300.0;
pub const AL_PAD_X: f32 = 16.0;
/// "Alerts | Log" segmented control: 34 tall, radius 8, 3 px inset; the
/// selected segment 28 tall, radius 5, weight 500.
pub const AL_H_SEGMENT: f32 = 34.0;
pub const AL_R_SEGMENT: f32 = 8.0;
pub const AL_H_SEGMENT_BUTTON: f32 = 28.0;
pub const AL_R_SEGMENT_BUTTON: f32 = 5.0;
/// Panel toolbar: 38 px buttons with 28 px icons, 8 from the panel edge.
pub const AL_TOOL_BUTTON: f32 = 38.0;
/// Empty state: 120 px illustration, 16 px text in a 260 column, a 34 tall
/// radius 8 dark button 20 below.
pub const AL_EMPTY_ICON: f32 = 120.0;
pub const AL_EMPTY_TEXT: f32 = 16.0;
pub const AL_W_EMPTY: f32 = 260.0;
