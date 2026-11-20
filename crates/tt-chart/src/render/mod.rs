//! Drawing backend boundary.
//!
//! Chart code draws exclusively through [`Canvas`], a handful of immediate
//! primitives in screen points. [`EguiCanvas`] implements it on top of
//! `egui::Painter`; a custom wgpu renderer only has to implement this trait
//! (geometry types are `emath`/`ecolor`, which carry no egui runtime).

mod egui_canvas;

pub use egui_canvas::EguiCanvas;

use egui::{Align2, Color32, Pos2, Rect, Vec2};

pub trait Canvas {
    /// Physical pixels per logical point, for pixel snapping.
    fn pixels_per_point(&self) -> f32;

    /// Restricts subsequent primitives to `rect`.
    fn set_clip(&mut self, rect: Rect);

    fn fill_rect(&mut self, rect: Rect, color: Color32);

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color32);

    /// Rectangle outline drawn *inside* `rect`.
    fn stroke_rect(&mut self, rect: Rect, width: f32, color: Color32);

    fn line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32);

    fn dashed_line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32, dash: f32, gap: f32);

    fn polyline(&mut self, points: &[Pos2], width: f32, color: Color32);

    /// Fills the area between `points` (an x-sorted polyline) and `baseline_y`
    /// with a vertical gradient from `top` (at the line) to `bottom` (at the baseline).
    fn fill_area(&mut self, points: &[Pos2], baseline_y: f32, top: Color32, bottom: Color32);

    /// Draws monospace text; returns its bounding rect.
    fn text(&mut self, pos: Pos2, anchor: Align2, text: &str, size: f32, color: Color32) -> Rect;

    /// Size of monospace `text` at `size` without drawing it.
    fn text_size(&mut self, text: &str, size: f32) -> Vec2;
}
