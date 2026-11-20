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

    /// A 1-px dotted line (1 px on, 3 px off, TradingView's grid style).
    /// Backends may draw it as one textured quad instead of many dots.
    fn dotted_line(&mut self, a: Pos2, b: Pos2, color: Color32) {
        let px = 1.0 / self.pixels_per_point();
        self.dashed_line(a, b, px, color, px, 3.0 * px);
    }

    /// Fills the area between `points` (an x-sorted polyline) and `baseline_y`
    /// with a fixed vertical gradient: `top` at `gradient_top_y`, `bottom` at
    /// `baseline_y` (independent of the line's shape, so there is no banding).
    fn fill_area(
        &mut self,
        points: &[Pos2],
        baseline_y: f32,
        gradient_top_y: f32,
        top: Color32,
        bottom: Color32,
    );

    /// Fills the band between two polylines sampled at the same x positions
    /// (`top[i].x == bottom[i].x`).
    fn fill_between(&mut self, top: &[Pos2], bottom: &[Pos2], color: Color32);

    fn fill_circle(&mut self, center: Pos2, radius: f32, color: Color32);

    /// Draws text in the UI font (`bold` uses the semibold face when the
    /// backend has one); returns its bounding rect.
    fn text_styled(
        &mut self,
        pos: Pos2,
        anchor: Align2,
        text: &str,
        size: f32,
        color: Color32,
        bold: bool,
    ) -> Rect;

    /// Size of `text` at `size` without drawing it.
    fn text_size_styled(&mut self, text: &str, size: f32, bold: bool) -> Vec2;

    /// Draws an icon glyph (from the icon font, if the backend has one)
    /// centered on `center`; returns its bounding rect.
    fn icon(&mut self, center: Pos2, glyph: char, size: f32, color: Color32) -> Rect {
        let _ = (glyph, color);
        Rect::from_center_size(center, Vec2::splat(size))
    }

    /// Regular-weight text; see [`Canvas::text_styled`].
    fn text(&mut self, pos: Pos2, anchor: Align2, text: &str, size: f32, color: Color32) -> Rect {
        self.text_styled(pos, anchor, text, size, color, false)
    }

    fn text_size(&mut self, text: &str, size: f32) -> Vec2 {
        self.text_size_styled(text, size, false)
    }
}
