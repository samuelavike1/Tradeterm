//! Drawing backend boundary.
//!
//! Chart code draws exclusively through [`Canvas`], a handful of immediate
//! primitives in screen points. `tt-chart-gpui`'s `GpuiCanvas` implements it
//! on gpui; another backend only has to implement this trait. Geometry and
//! colors are `emath` / `ecolor` types (plain data, no UI runtime).

use crate::drawings::LineStyle;
use ecolor::Color32;
use emath::{Align2, Pos2, Rect, Vec2};

pub trait Canvas {
    /// Physical pixels per logical point, for pixel snapping.
    fn pixels_per_point(&self) -> f32;

    /// Restricts subsequent primitives to `rect`.
    fn set_clip(&mut self, rect: Rect);

    fn fill_rect(&mut self, rect: Rect, color: Color32);

    /// Fills `rect` with a vertical gradient, `top` to `bottom`. The default
    /// draws 64 horizontal bands; backends override it with a real gradient.
    fn fill_rect_gradient(&mut self, rect: Rect, top: Color32, bottom: Color32) {
        if top == bottom {
            self.fill_rect(rect, top);
            return;
        }
        const BANDS: usize = 64;
        let h = rect.height() / BANDS as f32;
        for i in 0..BANDS {
            let t = (i as f32 + 0.5) / BANDS as f32;
            let y0 = rect.top() + h * i as f32;
            let band = Rect::from_min_max(
                emath::pos2(rect.left(), y0),
                emath::pos2(rect.right(), (y0 + h + 0.5).min(rect.bottom())),
            );
            self.fill_rect(band, lerp_color(top, bottom, t));
        }
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color32);

    /// Rectangle outline drawn *inside* `rect`.
    fn stroke_rect(&mut self, rect: Rect, width: f32, color: Color32);

    fn line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32);

    fn dashed_line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32, dash: f32, gap: f32);

    fn polyline(&mut self, points: &[Pos2], width: f32, color: Color32);

    /// A dashed polyline: `dash` on, `gap` off, measured along the path so
    /// the pattern runs on across vertices. The default draws each dash
    /// piece with [`Canvas::line`]; backends override it to batch them.
    fn dashed_polyline(
        &mut self,
        points: &[Pos2],
        width: f32,
        color: Color32,
        dash: f32,
        gap: f32,
    ) {
        dash_pieces(points, dash, gap, |a, b| self.line(a, b, width, color));
    }

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

    /// Fills a simple polygon (convex or concave, either winding; the last
    /// point connects to the first). The default draws nothing: backends
    /// that can fill triangles override it (see [`triangulate`]).
    fn fill_polygon(&mut self, points: &[Pos2], color: Color32) {
        let _ = (points, color);
    }

    /// A polyline in a line style. The default draws each segment with
    /// [`styled_line`] (dashes restart at each vertex).
    fn stroke_polyline_styled(
        &mut self,
        points: &[Pos2],
        width: f32,
        color: Color32,
        style: LineStyle,
    ) {
        if style == LineStyle::Solid {
            self.polyline(points, width, color);
            return;
        }
        for w in points.windows(2) {
            styled_line(self, w[0], w[1], width, color, style);
        }
    }

    /// An axis-aligned ellipse inscribed in `rect`, filled and/or outlined.
    /// The default samples 64 segments on the stack (no allocation).
    fn ellipse(&mut self, rect: Rect, stroke: Option<LineSpec>, fill: Option<Color32>) {
        const N: usize = 64;
        let (c, rx, ry) = (rect.center(), rect.width() / 2.0, rect.height() / 2.0);
        let mut pts = [Pos2::ZERO; N + 1];
        for (i, p) in pts.iter_mut().enumerate() {
            let t = i as f32 / N as f32 * std::f32::consts::TAU;
            *p = Pos2::new(c.x + rx * t.cos(), c.y + ry * t.sin());
        }
        if let Some(f) = fill {
            self.fill_polygon(&pts[..N], f);
        }
        if let Some(s) = stroke {
            self.stroke_polyline_styled(&pts, s.width, s.color, s.style);
        }
    }

    /// Text with bold / italic and rotation. The default ignores italic and,
    /// when rotated, lays the glyphs upright along the rotated baseline
    /// ([`text_along`]); backends with real rotated text override it.
    fn text_rich(
        &mut self,
        pos: Pos2,
        anchor: Align2,
        text: &str,
        size: f32,
        color: Color32,
        opts: TextOpts,
    ) -> Rect {
        if opts.rotation == 0.0 {
            self.text_styled(pos, anchor, text, size, color, opts.bold)
        } else {
            text_along(self, pos, anchor, text, size, color, opts)
        }
    }

    /// An open arrow head (two strokes) at `tip`, pointing along `dir`
    /// (normalized), `size` points long and about half as wide.
    fn arrow_head(&mut self, tip: Pos2, dir: Vec2, size: f32, width: f32, color: Color32) {
        let side = Vec2::new(-dir.y, dir.x) * (size * 4.0 / 7.0);
        let back = tip - dir * size;
        self.line(tip, back + side, width, color);
        self.line(tip, back - side, width, color);
    }
}

/// Linear blend of two colors (premultiplied channels), `t` in `0..=1`.
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}

/// A stroke for [`Canvas::ellipse`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineSpec {
    pub width: f32,
    pub color: Color32,
    pub style: LineStyle,
}

/// Options of [`Canvas::text_rich`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextOpts {
    pub bold: bool,
    pub italic: bool,
    /// Clockwise on screen, in radians (`-FRAC_PI_2`: reads bottom to top).
    pub rotation: f32,
}

/// A straight line in a line style (dash lengths scale with the width).
pub fn styled_line<C: Canvas + ?Sized>(
    c: &mut C,
    a: Pos2,
    b: Pos2,
    width: f32,
    color: Color32,
    style: LineStyle,
) {
    match style {
        LineStyle::Solid => c.line(a, b, width, color),
        LineStyle::Dashed => c.dashed_line(
            a,
            b,
            width,
            color,
            6.0 * width.max(1.0),
            4.0 * width.max(1.0),
        ),
        LineStyle::Dotted => {
            c.dashed_line(a, b, width, color, width.max(1.0), 2.5 * width.max(1.0))
        }
    }
}

/// Splits a polyline into its dash pieces (`dash` on, `gap` off, along the
/// path); a dash that crosses a vertex yields one piece per segment.
pub fn dash_pieces(points: &[Pos2], dash: f32, gap: f32, mut piece: impl FnMut(Pos2, Pos2)) {
    if points.len() < 2 || dash.is_nan() || dash <= 0.0 || gap.is_nan() || gap < 0.0 {
        return;
    }
    // Position in the pattern: `on` while drawing, `left` until it flips.
    let (mut on, mut left) = (true, dash);
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.distance(b);
        if len.is_nan() || len <= 0.0 {
            continue;
        }
        let dir = (b - a) / len;
        let mut t = 0.0;
        while t < len {
            let step = left.min(len - t);
            if on {
                piece(a + dir * t, a + dir * (t + step));
            }
            t += step;
            left -= step;
            if left <= 0.0 {
                on = !on;
                left = if on { dash } else { gap };
                if left <= 0.0 {
                    // Zero gap: solid.
                    on = true;
                    left = dash;
                }
            }
        }
    }
}

/// Rotated text for backends without it: each glyph upright, centered on
/// the rotated baseline (no allocation beyond the backend's own text calls).
/// `anchor` is relative to the unrotated text box, which turns about `pos`.
/// Returns the bounding rect of the rotated box.
pub fn text_along<C: Canvas + ?Sized>(
    c: &mut C,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    size: f32,
    color: Color32,
    opts: TextOpts,
) -> Rect {
    let glyph = TextOpts {
        rotation: 0.0,
        ..opts
    };
    let total = c.text_size_styled(text, size, opts.bold);
    let (u, n) = {
        let (s, k) = opts.rotation.sin_cos();
        (Vec2::new(k, s), Vec2::new(-s, k))
    };
    let frac = |a: emath::Align| match a {
        emath::Align::Min => 0.0,
        emath::Align::Center => 0.5,
        emath::Align::Max => 1.0,
    };
    let (lx, ly) = (frac(anchor.x()) * total.x, frac(anchor.y()) * total.y);
    let origin = pos - u * lx - n * ly;
    let mut x = 0.0;
    let mut buf = [0u8; 4];
    for ch in text.chars() {
        let s = ch.encode_utf8(&mut buf);
        let w = c.text_size_styled(s, size, opts.bold).x;
        let center = origin + u * (x + w / 2.0) + n * (total.y / 2.0);
        c.text_rich(center, Align2::CENTER_CENTER, s, size, color, glyph);
        x += w;
    }
    let corners = [
        origin,
        origin + u * total.x,
        origin + n * total.y,
        origin + u * total.x + n * total.y,
    ];
    Rect::from_points(&corners)
}

/// Triangulates a simple polygon (either winding; last point joins the
/// first), calling `tri` per triangle. Convex polygons are fanned; concave
/// ones are ear-clipped with `scratch` (vertex indices, reused between
/// calls). A self-intersecting remainder is fanned.
pub fn triangulate(
    points: &[Pos2],
    scratch: &mut Vec<usize>,
    mut tri: impl FnMut(Pos2, Pos2, Pos2),
) {
    let n = points.len();
    if n < 3 {
        return;
    }
    let cross = |a: Pos2, b: Pos2, c: Pos2| (b - a).x * (c - b).y - (b - a).y * (c - b).x;
    let area2: f32 = (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    let sign = if area2 >= 0.0 { 1.0 } else { -1.0 };
    let convex =
        (0..n).all(|i| cross(points[i], points[(i + 1) % n], points[(i + 2) % n]) * sign >= 0.0);
    if convex {
        for i in 1..n - 1 {
            tri(points[0], points[i], points[i + 1]);
        }
        return;
    }
    scratch.clear();
    scratch.extend(0..n);
    let inside = |p: Pos2, a: Pos2, b: Pos2, c: Pos2| {
        let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
        // On an edge counts as inside: a reflex vertex on the candidate's
        // diagonal would otherwise let it clip outside the polygon.
        d1 * sign >= 0.0 && d2 * sign >= 0.0 && d3 * sign >= 0.0
    };
    while scratch.len() > 3 {
        let m = scratch.len();
        let ear = (0..m).find(|&i| {
            let (ia, ib, ic) = (scratch[(i + m - 1) % m], scratch[i], scratch[(i + 1) % m]);
            let (a, b, c) = (points[ia], points[ib], points[ic]);
            cross(a, b, c) * sign > 0.0
                && !scratch
                    .iter()
                    .filter(|&&j| j != ia && j != ib && j != ic)
                    .any(|&j| inside(points[j], a, b, c))
        });
        let Some(i) = ear else {
            break;
        };
        let (ia, ib, ic) = (scratch[(i + m - 1) % m], scratch[i], scratch[(i + 1) % m]);
        tri(points[ia], points[ib], points[ic]);
        scratch.remove(i);
    }
    for k in 1..scratch.len().saturating_sub(1) {
        tri(
            points[scratch[0]],
            points[scratch[k]],
            points[scratch[k + 1]],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emath::pos2;

    fn area(points: &[Pos2]) -> f32 {
        let mut scratch = Vec::new();
        let mut total = 0.0;
        triangulate(points, &mut scratch, |a, b, c| {
            total += ((b - a).x * (c - a).y - (b - a).y * (c - a).x).abs() / 2.0;
        });
        total
    }

    #[test]
    fn dash_pattern_runs_across_vertices() {
        let mut pieces = Vec::new();
        // An L of 10 + 10 with dash 4 / gap 2: 0-4, 6-10 | 12-16, 18-20.
        let pts = [pos2(0., 0.), pos2(10., 0.), pos2(10., 10.)];
        dash_pieces(&pts, 4.0, 2.0, |a, b| pieces.push((a, b)));
        assert_eq!(
            pieces,
            [
                (pos2(0., 0.), pos2(4., 0.)),
                (pos2(6., 0.), pos2(10., 0.)),
                (pos2(10., 2.), pos2(10., 6.)),
                (pos2(10., 8.), pos2(10., 10.)),
            ]
        );
        // A dash crossing the vertex splits into two pieces.
        pieces.clear();
        dash_pieces(&pts, 12.0, 1.0, |a, b| pieces.push((a, b)));
        assert_eq!(
            pieces[..2],
            [
                (pos2(0., 0.), pos2(10., 0.)),
                (pos2(10., 0.), pos2(10., 2.))
            ]
        );
        pieces.clear();
        dash_pieces(&pts[..1], 1.0, 1.0, |a, b| pieces.push((a, b)));
        assert!(pieces.is_empty());
    }

    #[test]
    fn triangulates_convex_and_concave_polygons() {
        let square = [pos2(0., 0.), pos2(10., 0.), pos2(10., 10.), pos2(0., 10.)];
        assert!((area(&square) - 100.0).abs() < 1e-3);
        let mut rev = square;
        rev.reverse();
        assert!((area(&rev) - 100.0).abs() < 1e-3);
        // An L shape (concave): 10x10 minus a 5x5 corner.
        let l = [
            pos2(0., 0.),
            pos2(10., 0.),
            pos2(10., 5.),
            pos2(5., 5.),
            pos2(5., 10.),
            pos2(0., 10.),
        ];
        assert!((area(&l) - 75.0).abs() < 1e-3, "{}", area(&l));
        assert_eq!(area(&l[..2]), 0.0);
    }
}
