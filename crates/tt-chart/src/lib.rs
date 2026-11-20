//! Price chart, independent of any UI toolkit.
//!
//! The chart draws through the [`render::Canvas`] trait and takes input as
//! plain method calls, so any front end can host it (`tt-chart-gpui` does it
//! for gpui). Geometry and colors are `emath` / `ecolor` types.
//! - [`viewport`] and [`scale`]: pure math (bar ↔ x, price ↔ y in linear, log
//!   and percent modes, tick picking, label formatting). Unit tested.
//! - [`time_axis`]: index-based time ticks (gaps take no space).
//! - [`lod`]: min/max pyramid; zoomed far out, each pixel column is built
//!   from a few precomputed blocks instead of thousands of bars.
//! - `draw`: builds the chart from primitives of the [`render::Canvas`] trait.
//! - [`render`]: the backend boundary (the `Canvas` trait).
//! - [`ChartView`]: layout, caches, the two drawing layers
//!   ([`ChartView::render_base`], [`ChartView::render_overlay`]), hit testing
//!   and input (pan, zoom, axis and divider drags, legend buttons).

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

pub use draw::LegendButton;
pub use indicators::Indicator;
pub use kind::{ChartType, ScaleMode};
pub use style::ChartStyle;
pub use view::{
    BACKFILL_MARGIN_BARS, ChartData, ChartHit, ChartLayout, ChartStats, ChartView, RenderInput,
    RenderOutput,
};
