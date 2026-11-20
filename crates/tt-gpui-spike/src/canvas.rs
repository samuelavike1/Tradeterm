//! `tt_chart::render::Canvas` on gpui.
//!
//! Primitives are recorded per clip rect and replayed in `finish` inside
//! `with_content_mask` (gpui clips with scoped masks). Rects become
//! `paint_quad`s (GPU-instanced); lines, polylines and fills become `Path`
//! triangle meshes, merged into one path per color and clip so a frame has
//! few draw primitives.

use egui::{Align2, Color32, Pos2, Rect, Vec2};
use gpui_kit::{
    App, Background, BorderStyle, Bounds, ContentMask, Corners, Edges, Font, FontWeight, Hsla,
    PaintQuad, Path, Pixels, Rgba, ShapedLine, SharedString, TextAlign, TextRun, Window, point, px,
    size,
};
use std::collections::HashMap;
use tt_chart::render::Canvas;

/// Line height as a multiple of the font size (matches egui's galley height
/// closely enough for anchoring).
const LINE_HEIGHT: f32 = 1.25;

enum Prim {
    Quad(PaintQuad),
    /// Boxed: `ShapedLine` carries a large inline SmallVec, which would make
    /// every `Prim` (also quads) ~1.4 KB.
    Text(Box<(ShapedLine, Pos2, f32)>),
}

#[derive(Default)]
struct Batch {
    prims: Vec<Prim>,
    /// Triangle meshes by color (RGBA), merged per clip.
    meshes: HashMap<[u8; 4], Path<Pixels>>,
    /// Gradient fills keep their own paths.
    gradients: Vec<(Path<Pixels>, Background)>,
}

pub struct GpuiCanvas<'a> {
    window: &'a mut Window,
    ppp: f32,
    clip: Rect,
    batches: Vec<(Rect, Batch)>,
    font: SharedString,
    bold_font: SharedString,
    icon_font: SharedString,
    pub prim_count: usize,
    /// Time spent shaping text (profiling).
    pub shape_time: std::time::Duration,
    pub shape_calls: usize,
    pub tris: usize,
    /// Time per primitive kind (profiling): quads, meshes.
    pub quad_time: std::time::Duration,
    pub mesh_time: std::time::Duration,
}

const _: () = assert!(std::mem::size_of::<Prim>() <= 256);

pub fn hsla(c: Color32) -> Hsla {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Hsla::from(Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
    })
}

fn bounds(r: Rect) -> Bounds<Pixels> {
    Bounds::new(
        point(px(r.min.x), px(r.min.y)),
        size(px(r.width()), px(r.height())),
    )
}

fn pt(p: Pos2) -> gpui_kit::Point<Pixels> {
    point(px(p.x), px(p.y))
}

/// Two solid triangles (st = (0, 1) marks a filled, non-curve triangle).
fn push_quad(mesh: &mut Path<Pixels>, a: Pos2, b: Pos2, c: Pos2, d: Pos2) {
    let st = (point(0., 1.), point(0., 1.), point(0., 1.));
    mesh.push_triangle((pt(a), pt(b), pt(c)), st);
    mesh.push_triangle((pt(a), pt(c), pt(d)), st);
}

impl<'a> GpuiCanvas<'a> {
    pub fn new(window: &'a mut Window, clip: Rect) -> Self {
        let ppp = window.scale_factor();
        Self {
            window,
            ppp,
            clip,
            batches: vec![(clip, Batch::default())],
            font: "Segoe UI".into(),
            bold_font: "Segoe UI Semibold".into(),
            icon_font: "lucide".into(),
            prim_count: 0,
            shape_time: std::time::Duration::ZERO,
            shape_calls: 0,
            tris: 0,
            quad_time: std::time::Duration::ZERO,
            mesh_time: std::time::Duration::ZERO,
        }
    }

    fn batch(&mut self) -> &mut Batch {
        &mut self.batches.last_mut().expect("batch").1
    }

    /// The merged mesh for `color` in the current clip.
    fn mesh(&mut self, color: Color32, seed: Pos2) -> &mut Path<Pixels> {
        let key = color.to_srgba_unmultiplied();
        self.batches
            .last_mut()
            .expect("batch")
            .1
            .meshes
            .entry(key)
            .or_insert_with(|| Path::new(pt(seed)))
    }

    fn quad_tris(&mut self, a: Pos2, b: Pos2, c: Pos2, d: Pos2, color: Color32) {
        self.tris += 2;
        push_quad(self.mesh(color, a), a, b, c, d);
    }

    fn shape(
        &mut self,
        text: &str,
        size: f32,
        color: Color32,
        family: SharedString,
        bold: bool,
    ) -> ShapedLine {
        let font = Font {
            family,
            weight: if bold {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            },
            ..gpui_kit::font("Segoe UI")
        };
        let run = TextRun {
            len: text.len(),
            font,
            color: hsla(color),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let t = std::time::Instant::now();
        let line = self.window.text_system().shape_line(
            SharedString::from(text.to_owned()),
            px(size),
            &[run],
            None,
        );
        self.shape_time += t.elapsed();
        self.shape_calls += 1;
        line
    }

    /// Replays everything into the window's scene.
    pub fn finish(mut self, cx: &mut App) {
        let batches = std::mem::take(&mut self.batches);
        for (clip, batch) in batches {
            let mask = ContentMask {
                bounds: bounds(clip),
            };
            let window = &mut *self.window;
            window.with_content_mask(Some(mask), |window| {
                // Order inside a batch: quads/text in record order, but meshes
                // per color are drawn first. Good enough for the spike: fills
                // and lines rarely interleave with text within one clip.
                for (path, bg) in batch.gradients {
                    window.paint_path(path, bg);
                }
                for ([r, g, b, a], path) in batch.meshes {
                    window.paint_path(path, hsla(Color32::from_rgba_unmultiplied(r, g, b, a)));
                }
                for p in batch.prims {
                    match p {
                        Prim::Quad(q) => window.paint_quad(q),
                        Prim::Text(t) => {
                            let (line, pos, size) = *t;
                            let _ = line.paint(
                                pt(pos),
                                px(size * LINE_HEIGHT),
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        }
                    }
                }
            });
        }
    }
}

impl Canvas for GpuiCanvas<'_> {
    fn pixels_per_point(&self) -> f32 {
        self.ppp
    }

    fn set_clip(&mut self, rect: Rect) {
        if rect != self.clip {
            self.clip = rect;
            self.batches.push((rect, Batch::default()));
        }
    }

    fn fill_rect(&mut self, rect: Rect, color: Color32) {
        if color.a() == 0 || !self.clip.intersects(rect) {
            return;
        }
        let t = std::time::Instant::now();
        self.prim_count += 1;
        self.batch()
            .prims
            .push(Prim::Quad(gpui_kit::fill(bounds(rect), hsla(color))));
        self.quad_time += t.elapsed();
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color32) {
        self.prim_count += 1;
        self.batch().prims.push(Prim::Quad(
            gpui_kit::fill(bounds(rect), hsla(color)).corner_radii(Corners::all(px(radius))),
        ));
    }

    fn stroke_rect(&mut self, rect: Rect, width: f32, color: Color32) {
        self.prim_count += 1;
        self.batch().prims.push(Prim::Quad(PaintQuad {
            bounds: bounds(rect),
            corner_radii: Corners::default(),
            background: Background::from(Hsla::transparent_black()),
            border_widths: Edges::all(px(width)),
            border_color: hsla(color),
            border_style: BorderStyle::Solid,
        }));
    }

    fn line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32) {
        // Axis-aligned lines are quads (crisp); others are thin meshes.
        if a.x == b.x || a.y == b.y {
            let h = width / 2.0;
            let r = Rect::from_two_pos(a, b).expand2(if a.x == b.x {
                Vec2::new(h, 0.0)
            } else {
                Vec2::new(0.0, h)
            });
            self.fill_rect(r, color);
        } else {
            self.polyline(&[a, b], width, color);
        }
    }

    fn dashed_line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32, dash: f32, gap: f32) {
        let len = a.distance(b);
        if len <= 0.0 || dash <= 0.0 {
            return;
        }
        let dir = (b - a) / len;
        let mut t = 0.0;
        while t < len {
            let e = (t + dash).min(len);
            self.line(a + dir * t, a + dir * e, width, color);
            t = e + gap;
        }
    }

    fn polyline(&mut self, points: &[Pos2], width: f32, color: Color32) {
        if points.len() < 2 || color.a() == 0 {
            return;
        }
        let t = std::time::Instant::now();
        self.prim_count += 1;
        let h = width.max(1.0 / self.ppp) / 2.0;
        self.tris += 2 * (points.len() - 1);
        let mesh = self.mesh(color, points[0]);
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let d = b - a;
            let l = d.length();
            if l <= 0.0 {
                continue;
            }
            let n = Vec2::new(-d.y, d.x) / l * h;
            // Extend each segment by half the width to cover the joins.
            let e = d / l * h;
            push_quad(mesh, a - e + n, b + e + n, b + e - n, a - e - n);
        }
        self.mesh_time += t.elapsed();
    }

    fn dotted_line(&mut self, a: Pos2, b: Pos2, color: Color32) {
        let px1 = 1.0 / self.ppp;
        // Axis-aligned (grid): one quad with a dashed border edge. gpui's dash
        // pattern is fixed (2 px on, 1 px off for a 1 px border).
        if a.x == b.x || a.y == b.y {
            let horizontal = a.y == b.y;
            let r = if horizontal {
                Rect::from_min_max(
                    Pos2::new(a.x.min(b.x), a.y - px1 / 2.0),
                    Pos2::new(a.x.max(b.x), a.y + px1 / 2.0),
                )
            } else {
                Rect::from_min_max(
                    Pos2::new(a.x - px1 / 2.0, a.y.min(b.y)),
                    Pos2::new(a.x + px1 / 2.0, a.y.max(b.y)),
                )
            };
            let mut edges = Edges::default();
            if horizontal {
                edges.top = px(px1);
            } else {
                edges.left = px(px1);
            }
            self.prim_count += 1;
            self.batch().prims.push(Prim::Quad(PaintQuad {
                bounds: bounds(r),
                corner_radii: Corners::default(),
                background: Background::from(Hsla::transparent_black()),
                border_widths: edges,
                border_color: hsla(color),
                border_style: BorderStyle::Dashed,
            }));
            return;
        }
        // Otherwise a mesh of 1-px dots.
        let len = a.distance(b);
        if len <= 0.0 {
            return;
        }
        let dir = (b - a) / len;
        let n = Vec2::new(-dir.y, dir.x) * (px1 / 2.0);
        let mut t = 0.0;
        while t < len {
            let p = a + dir * t;
            let q = a + dir * (t + px1);
            self.quad_tris(p + n, q + n, q - n, p - n, color);
            t += 4.0 * px1;
        }
    }

    fn fill_area(
        &mut self,
        points: &[Pos2],
        baseline_y: f32,
        gradient_top_y: f32,
        top: Color32,
        bottom: Color32,
    ) {
        if points.len() < 2 {
            return;
        }
        let mut path = Path::new(pt(points[0]));
        let st = (point(0., 1.), point(0., 1.), point(0., 1.));
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (a0, b0) = (Pos2::new(a.x, baseline_y), Pos2::new(b.x, baseline_y));
            path.push_triangle((pt(a), pt(b), pt(b0)), st);
            path.push_triangle((pt(a), pt(b0), pt(a0)), st);
        }
        let _ = gradient_top_y;
        let bg = gpui_kit::linear_gradient(
            180.,
            gpui_kit::linear_color_stop(hsla(top), 0.),
            gpui_kit::linear_color_stop(hsla(bottom), 1.),
        );
        self.prim_count += 1;
        self.batch().gradients.push((path, bg));
    }

    fn fill_between(&mut self, top: &[Pos2], bottom: &[Pos2], color: Color32) {
        let n = top.len().min(bottom.len());
        self.prim_count += 1;
        self.tris += 2 * n.saturating_sub(1);
        if n < 2 {
            return;
        }
        let mesh = self.mesh(color, top[0]);
        for i in 1..n {
            push_quad(mesh, top[i - 1], top[i], bottom[i], bottom[i - 1]);
        }
    }

    fn fill_circle(&mut self, center: Pos2, radius: f32, color: Color32) {
        self.prim_count += 1;
        self.batch().prims.push(Prim::Quad(
            gpui_kit::fill(
                bounds(Rect::from_center_size(center, Vec2::splat(radius * 2.0))),
                hsla(color),
            )
            .corner_radii(Corners::all(px(radius))),
        ));
    }

    fn text_styled(
        &mut self,
        pos: Pos2,
        anchor: Align2,
        text: &str,
        size: f32,
        color: Color32,
        bold: bool,
    ) -> Rect {
        let family = if bold {
            self.bold_font.clone()
        } else {
            self.font.clone()
        };
        let line = self.shape(text, size, color, family, false);
        let sz = Vec2::new(f32::from(line.width), size * LINE_HEIGHT);
        let rect = anchor.anchor_size(pos, sz);
        self.prim_count += 1;
        self.batch()
            .prims
            .push(Prim::Text(Box::new((line, rect.min, size))));
        rect
    }

    fn text_size_styled(&mut self, text: &str, size: f32, bold: bool) -> Vec2 {
        let family = if bold {
            self.bold_font.clone()
        } else {
            self.font.clone()
        };
        let line = self.shape(text, size, Color32::WHITE, family, false);
        Vec2::new(f32::from(line.width), size * LINE_HEIGHT)
    }

    fn icon(&mut self, center: Pos2, glyph: char, size: f32, color: Color32) -> Rect {
        let fam = self.icon_font.clone();
        let line = self.shape(&glyph.to_string(), size, color, fam, false);
        let sz = Vec2::new(f32::from(line.width), size * LINE_HEIGHT);
        let rect = Align2::CENTER_CENTER.anchor_size(center, sz);
        self.batch()
            .prims
            .push(Prim::Text(Box::new((line, rect.min, size))));
        rect
    }
}
