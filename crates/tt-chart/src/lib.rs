//! Candlestick chart widget.
//!
//! Module boundaries are chosen so the drawing backend can be swapped later:
//! - [`viewport`] and [`scale`]: pure math (bar ↔ x, price ↔ y, tick picking,
//!   label formatting). No egui types; unit tested.
//! - `paint`: the **only** module that talks to `egui::Painter`. A future
//!   wgpu renderer replaces this module (e.g. via an `egui_wgpu::Callback`)
//!   while consuming the same [`viewport::Viewport`].
//! - [`ChartView`]: input handling (pan/zoom/crosshair) and per-frame layout.

mod paint;
pub mod scale;
mod style;
mod view;
pub mod viewport;

pub use style::ChartStyle;
pub use view::ChartView;
