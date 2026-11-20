//! Interactive price chart widget.
//!
//! Module boundaries are chosen so the drawing backend can be swapped later:
//! - [`viewport`] and [`scale`]: pure math (bar ↔ x, price ↔ y in linear, log
//!   and percent modes, tick picking, label formatting). Unit tested.
//! - [`time_axis`]: index-based time ticks (gaps take no space).
//! - [`lod`]: min/max pyramid; zoomed far out, each pixel column is built
//!   from a few precomputed blocks instead of thousands of bars.
//! - `draw`: builds the chart from primitives of the [`render::Canvas`] trait.
//! - [`render`]: the backend boundary. [`render::EguiCanvas`] implements
//!   `Canvas` with `egui::Painter`; a custom wgpu renderer implements the
//!   same trait.
//! - [`ChartView`]: input handling (pan/zoom/axis drags/crosshair) and layout.

pub mod bars;
mod derived;
mod draw;
pub mod indicators;
mod kind;
pub mod lod;
pub mod render;
pub mod scale;
mod style;
pub mod time_axis;
mod view;
pub mod viewport;

pub use indicators::Indicator;
pub use kind::{ChartType, ScaleMode};
pub use style::ChartStyle;
pub use view::{BACKFILL_MARGIN_BARS, ChartData, ChartOutput, ChartStats, ChartView};
