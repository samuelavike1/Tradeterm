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

use std::borrow::Cow;

/// Font family of the Lucide icon font registered by [`init`].
pub const ICON_FONT: &str = "lucide";

/// Registers the fonts charts use (call once at startup).
pub fn init(cx: &mut gpui_kit::App) {
    cx.text_system()
        .add_fonts(vec![Cow::Borrowed(lucide_icons::LUCIDE_FONT_BYTES)])
        .expect("register the Lucide icon font");
}
