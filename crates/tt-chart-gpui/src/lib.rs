//! gpui backend for the tradeterm chart.
//!
//! - [`GpuiCanvas`]: `tt_chart::render::Canvas` on gpui's primitives.
//! - [`Chart`]: the chart as gpui views with a cached base layer and a cheap
//!   pointer overlay (see `chart.rs`).

mod canvas;
mod chart;
mod drawing_menu;
mod drawing_prefs;
mod drawing_toolbar;
pub mod motion;

pub use canvas::{CanvasFonts, GpuiCanvas, hsla};
pub use chart::{Chart, ChartEvent, ChartSource, ChartState, LayerStats, SeriesSource};
pub use drawing_menu::DrawingClipboard;
pub use drawing_prefs::{DrawingPrefs, TemplateTarget, prompt_template_name, template_menu};

/// Font family of the Lucide icon font: the kit's (ui-guidelines G1).
pub use tt_ui::ICON_FONT;

/// Registers the fonts charts use. Delegates to [`tt_ui::init`], which
/// registers the icon font exactly once, so calling both is harmless.
pub fn init(cx: &mut gpui_kit::App) {
    tt_ui::init(cx);
}
