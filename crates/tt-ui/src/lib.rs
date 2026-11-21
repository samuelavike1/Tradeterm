//! tradeterm's shared UI kit (docs/specs/ui-guidelines.md §3).
//!
//! Every control of one kind looks and behaves the same everywhere: sizes
//! come from [`tokens`] (`tt_theme::metrics`), colors from the global
//! [`UiTheme`] (`tt_theme::Theme`), which [`theme::sync`] also projects into
//! gpui-component's theme so both palettes are one.
//!
//! Dependency graph: `tt-theme` ← `tt-ui` ← `tt-chart-gpui` ← `tradeterm`.
//!
//! Start with [`init`] (registers the Lucide icon font), then call
//! [`theme::sync`] whenever the palette changes.

pub mod button;
pub mod dialog;
pub mod divider;
pub mod form;
pub mod glyph;
pub mod list;
pub mod menu;
pub mod motion;
pub mod overlay;
pub mod picker;
pub mod slider;
pub mod theme;
pub mod tokens;
pub mod tooltip;

pub use button::{Chip, ControlSize, IconButton, ToolbarButton};
pub use dialog::{
    DialogFooter, DialogFrame, DialogHeader, DialogOpts, DialogSize, NavTabs, TemplateMenu,
    open_dialog,
};
pub use divider::Divider;
pub use form::{CheckBox, CheckSelect, ColorField, Field, FormRow, NumberField, PALETTE, Select};
pub use glyph::{Glyph, GlyphEl, ICON_FONT, glyph, register_icon_font};
pub use list::{EmptyState, ListKey, ListNav, ListRow, Tag};
pub use menu::{Menu, MenuNav, MenuRow, MenuWidth, SectionHeader};
pub use overlay::{Overlay, PaneControls};
pub use picker::{
    ColorBarButton, ColorPalette, ColorSwatchButton, LineDash, LineEndSelect, LinePicker,
    LineStyleButton, LineValue, LineWidthButton, TV_PALETTE, TextStyleBar, TextStyleValue,
    ToggleSquare, line_preview,
};
pub use slider::{RangeField, RangeSlider};
pub use theme::{ActiveUiTheme, UiTheme, hsla};
pub use tooltip::{format_keys, kbd, tip, tip_action};

/// Registers what the kit needs at startup (the Lucide icon font). Call
/// once, after `gpui_kit::init`.
pub fn init(cx: &mut gpui_kit::App) {
    glyph::register_icon_font(cx);
}

/// Everything a screen usually needs.
pub mod prelude {
    pub use crate::button::{Chip, ControlSize, IconButton, ToolbarButton};
    pub use crate::dialog::{DialogFooter, DialogFrame, DialogOpts, DialogSize, NavTabs};
    pub use crate::divider::Divider;
    pub use crate::form::{CheckBox, CheckSelect, ColorField, Field, FormRow, NumberField, Select};
    pub use crate::glyph::{Glyph, glyph};
    pub use crate::list::{EmptyState, ListKey, ListNav, ListRow, Tag};
    pub use crate::menu::{Menu, MenuRow, MenuWidth, SectionHeader};
    pub use crate::overlay::{Overlay, PaneControls};
    pub use crate::picker::{
        ColorBarButton, ColorSwatchButton, LineDash, LinePicker, LineStyleButton, LineValue,
        LineWidthButton, TextStyleBar, TextStyleValue, ToggleSquare,
    };
    pub use crate::slider::{RangeField, RangeSlider};
    pub use crate::theme::{ActiveUiTheme, hsla};
    pub use crate::tokens::{self, PxExt};
    pub use crate::tooltip::{tip, tip_action};
}
