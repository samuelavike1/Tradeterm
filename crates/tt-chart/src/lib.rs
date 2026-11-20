//! Interactive price chart widget.
//!
//! Module boundaries are chosen so the drawing backend can be swapped later:
//! - [`viewport`] and [`scale`]: pure math (bar ↔ x, price ↔ y in linear, log
//!   and percent modes, tick picking, label formatting). Unit tested.
//! - `draw`: builds the chart from primitives of the [`render::Canvas`] trait.
//! - [`render`]: the backend boundary. [`render::EguiCanvas`] implements
//!   `Canvas` with `egui::Painter`; a custom wgpu renderer implements the
//!   same trait.
//! - [`ChartView`]: input handling (pan/zoom/axis drags/crosshair) and layout.

mod draw;
mod kind;
pub mod render;
pub mod scale;
mod style;
mod view;
pub mod viewport;

pub use kind::{ChartType, ScaleMode};
pub use style::ChartStyle;
pub use view::{ChartData, ChartOutput, ChartStats, ChartView};
