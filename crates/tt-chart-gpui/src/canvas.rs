//! [`tt_chart::render::Canvas`] on gpui.
//!
//! Primitives are recorded in draw order, grouped by clip rect, and replayed
//! in [`GpuiCanvas::finish`] inside `with_content_mask` (gpui clips with
//! scoped masks):
//! - rects → `paint_quad` (GPU-instanced);
//! - lines, polylines and fills → triangle `Path`s; consecutive triangles of
//!   the same color share one path, so a polyline is one primitive;
//! - axis-aligned dotted lines (the grid) → one dashed-border quad each;
//! - text and icons → `shape_line`.

use ecolor::Color32;
use emath::{Align2, Pos2, Rect, Vec2};
use gpui_kit::{
    App, Background, BorderStyle, Bounds, ContentMask, Corners, Edges, Font, FontStyle, FontWeight,
    Hsla, PaintQuad, Path, Pixels, Rgba, ShapedLine, SharedString, TextAlign, TextRun, Window,
    point, px, size,
};
use tt_chart::render::{Canvas, TextOpts};

/// Line height as a multiple of the font size (the text height the chart's
/// label anchoring was tuned with).
const LINE_HEIGHT: f32 = 1.25;

/// Font families used for chart text.
#[derive(Debug, Clone)]
pub struct CanvasFonts {
    pub ui: SharedString,
    pub icons: SharedString,
}

impl Default for CanvasFonts {
    fn default() -> Self {
        Self {
            ui: if cfg!(target_os = "windows") {
                "Segoe UI".into()
            } else {
                ".SystemUIFont".into()
            },
            icons: crate::ICON_FONT.into(),
        }
    }
}

enum Item {
    Quad(PaintQuad),
    /// Boxed: `ShapedLine` carries a large inline SmallVec; unboxed, every
    /// item (quads too) would be ~1.4 KB.
    Text(Box<(ShapedLine, Pos2, f32)>),
    Path(Path<Pixels>, Background, Option<[u8; 4]>),
}

const _: () = assert!(std::mem::size_of::<Item>() <= 256);

/// Records chart primitives; call [`Self::finish`] to paint them.
pub struct GpuiCanvas<'a> {
    window: &'a mut Window,
    fonts: &'a CanvasFonts,
    ppp: f32,
    /// Area of the whole canvas.
    full: Rect,
    clip: Rect,
    batches: Vec<(Rect, Vec<Item>)>,
    /// Vertex indices for polygon triangulation (allocated on first use).
    scratch: Vec<usize>,
    /// Primitives recorded (for performance overlays).
    pub primitives: usize,
}

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
    /// A canvas drawing within `clip` (normally the element's bounds).
    pub fn new(window: &'a mut Window, fonts: &'a CanvasFonts, clip: Rect) -> Self {
        let ppp = window.scale_factor();
        Self {
            window,
            fonts,
            ppp,
            full: clip,
            clip,
            batches: vec![(clip, Vec::new())],
            scratch: Vec::new(),
            primitives: 0,
        }
    }

    fn items(&mut self) -> &mut Vec<Item> {
        &mut self.batches.last_mut().expect("batch").1
    }

    fn push(&mut self, item: Item) {
        self.primitives += 1;
        self.items().push(item);
    }

    /// The mesh to append solid `color` triangles to: the previous item if
    /// it is a mesh of that color (keeps draw order), else a new one.
    fn mesh(&mut self, color: Color32, seed: Pos2) -> &mut Path<Pixels> {
        let key = color.to_srgba_unmultiplied();
        let reuse = matches!(self.items().last(), Some(Item::Path(_, _, Some(k))) if *k == key);
        if !reuse {
            self.push(Item::Path(
                Path::new(pt(seed)),
                Background::from(hsla(color)),
                Some(key),
            ));
        }
        match self.items().last_mut() {
            Some(Item::Path(p, _, _)) => p,
            _ => unreachable!("just pushed a path"),
        }
    }

    fn shape(
        &mut self,
        text: &str,
        size: f32,
        color: Color32,
        family: SharedString,
        bold: bool,
        italic: bool,
    ) -> ShapedLine {
        let font = Font {
            family,
            weight: if bold {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            },
            style: if italic {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
            ..gpui_kit::font(self.fonts.ui.clone())
        };
        let run = TextRun {
            len: text.len(),
            font,
            color: hsla(color),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        self.window.text_system().shape_line(
            SharedString::from(text.to_owned()),
            px(size),
            &[run],
            None,
        )
    }

    /// Paints everything recorded, in three passes, each clipped like it
    /// was recorded:
    ///
    /// 1. quads, inside one gpui layer: every primitive shares the layer's
    ///    draw order, so gpui skips its per-primitive bounds-tree insertion
    ///    (otherwise the bulk of a frame, also when a cached view replays);
    /// 2. paths (the few indicator lines/fills), each with its own order:
    ///    gpui composites same-order paths as if disjoint, which would
    ///    double-blend overlapping translucent fills. Painting them right
    ///    after the quad layer keeps them in one GPU batch (each path batch
    ///    costs a full-window MSAA clear + resolve);
    /// 3. text, inside a second layer, on top of both.
    ///
    /// The stacking (quads < lines < text) is the right one for a chart.
    pub fn finish(mut self, cx: &mut App) {
        let batches = std::mem::take(&mut self.batches);
        let window = &mut *self.window;
        let full = bounds(self.full);
        let mut paths = Vec::new();
        let mut texts = Vec::new();
        window.paint_layer(full, |window| {
            for (clip, items) in batches {
                let mask = ContentMask {
                    bounds: bounds(clip),
                };
                let (mut clip_paths, mut clip_texts) = (Vec::new(), Vec::new());
                window.with_content_mask(Some(mask), |window| {
                    for item in items {
                        match item {
                            Item::Quad(q) => window.paint_quad(q),
                            Item::Path(path, bg, _) => clip_paths.push((path, bg)),
                            Item::Text(t) => clip_texts.push(t),
                        }
                    }
                });
                if !clip_paths.is_empty() {
                    paths.push((mask, clip_paths));
                }
                if !clip_texts.is_empty() {
                    texts.push((mask, clip_texts));
                }
            }
        });
        for (mask, clip_paths) in paths {
            window.with_content_mask(Some(mask), |window| {
                for (path, bg) in clip_paths {
                    window.paint_path(path, bg);
                }
            });
        }
        window.paint_layer(full, |window| {
            for (mask, clip_texts) in texts {
                window.with_content_mask(Some(mask), |window| {
                    for t in clip_texts {
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
                });
            }
        });
    }
}

impl Canvas for GpuiCanvas<'_> {
    fn pixels_per_point(&self) -> f32 {
        self.ppp
    }

    fn set_clip(&mut self, rect: Rect) {
        if rect != self.clip {
            self.clip = rect;
            self.batches.push((rect, Vec::new()));
        }
    }

    fn fill_rect(&mut self, rect: Rect, color: Color32) {
        if color.a() == 0 || !self.clip.intersects(rect) {
            return;
        }
        self.push(Item::Quad(gpui_kit::fill(bounds(rect), hsla(color))));
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color32) {
        if color.a() == 0 {
            return;
        }
        self.push(Item::Quad(
            gpui_kit::fill(bounds(rect), hsla(color)).corner_radii(Corners::all(px(radius))),
        ));
    }

    fn stroke_rect(&mut self, rect: Rect, width: f32, color: Color32) {
        self.push(Item::Quad(PaintQuad {
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
            let grow = if a.x == b.x {
                Vec2::new(h, 0.0)
            } else {
                Vec2::new(0.0, h)
            };
            self.fill_rect(Rect::from_two_pos(a, b).expand2(grow), color);
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
        let h = width.max(1.0 / self.ppp) / 2.0;
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
    }

    fn dotted_line(&mut self, a: Pos2, b: Pos2, color: Color32) {
        let px1 = 1.0 / self.ppp;
        if a.x == b.x || a.y == b.y {
            // One quad with a dashed border edge. gpui's dash pattern is
            // fixed (2 px on, 1 px off for a 1 px border).
            let horizontal = a.y == b.y;
            let (r, edges) = if horizontal {
                (
                    Rect::from_min_max(
                        Pos2::new(a.x.min(b.x), a.y - px1 / 2.0),
                        Pos2::new(a.x.max(b.x), a.y + 2.5 * px1),
                    ),
                    Edges {
                        top: px(px1),
                        ..Default::default()
                    },
                )
            } else {
                (
                    Rect::from_min_max(
                        Pos2::new(a.x - px1 / 2.0, a.y.min(b.y)),
                        Pos2::new(a.x + 2.5 * px1, a.y.max(b.y)),
                    ),
                    Edges {
                        left: px(px1),
                        ..Default::default()
                    },
                )
            };
            self.push(Item::Quad(PaintQuad {
                bounds: bounds(r),
                corner_radii: Corners::default(),
                background: Background::from(Hsla::transparent_black()),
                border_widths: edges,
                border_color: hsla(color),
                border_style: BorderStyle::Dashed,
            }));
            return;
        }
        self.dashed_line(a, b, px1, color, px1, 3.0 * px1);
    }

    fn fill_area(
        &mut self,
        points: &[Pos2],
        baseline_y: f32,
        _gradient_top_y: f32,
        top: Color32,
        bottom: Color32,
    ) {
        if points.len() < 2 {
            return;
        }
        let mut path = Path::new(pt(points[0]));
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (a0, b0) = (Pos2::new(a.x, baseline_y), Pos2::new(b.x, baseline_y));
            push_quad(&mut path, a, b, b0, a0);
        }
        // gpui gradients span the path's bounds (not a fixed top y).
        let bg = gpui_kit::linear_gradient(
            180.,
            gpui_kit::linear_color_stop(hsla(top), 0.),
            gpui_kit::linear_color_stop(hsla(bottom), 1.),
        );
        self.push(Item::Path(path, bg, None));
    }

    fn fill_between(&mut self, top: &[Pos2], bottom: &[Pos2], color: Color32) {
        let n = top.len().min(bottom.len());
        if n < 2 || color.a() == 0 {
            return;
        }
        let mesh = self.mesh(color, top[0]);
        for i in 1..n {
            push_quad(mesh, top[i - 1], top[i], bottom[i], bottom[i - 1]);
        }
    }

    fn fill_circle(&mut self, center: Pos2, radius: f32, color: Color32) {
        let r = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
        self.push(Item::Quad(
            gpui_kit::fill(bounds(r), hsla(color)).corner_radii(Corners::all(px(radius))),
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
        let line = self.shape(text, size, color, self.fonts.ui.clone(), bold, false);
        let rect = anchor.anchor_size(pos, Vec2::new(f32::from(line.width), size * LINE_HEIGHT));
        self.push(Item::Text(Box::new((line, rect.min, size))));
        rect
    }

    fn text_size_styled(&mut self, text: &str, size: f32, bold: bool) -> Vec2 {
        let line = self.shape(
            text,
            size,
            Color32::WHITE,
            self.fonts.ui.clone(),
            bold,
            false,
        );
        Vec2::new(f32::from(line.width), size * LINE_HEIGHT)
    }

    fn text_rich(
        &mut self,
        pos: Pos2,
        anchor: Align2,
        text: &str,
        size: f32,
        color: Color32,
        opts: TextOpts,
    ) -> Rect {
        if opts.rotation != 0.0 {
            // gpui has no rotated text: upright glyphs along the baseline.
            return tt_chart::render::text_along(self, pos, anchor, text, size, color, opts);
        }
        let line = self.shape(
            text,
            size,
            color,
            self.fonts.ui.clone(),
            opts.bold,
            opts.italic,
        );
        let rect = anchor.anchor_size(pos, Vec2::new(f32::from(line.width), size * LINE_HEIGHT));
        self.push(Item::Text(Box::new((line, rect.min, size))));
        rect
    }

    fn fill_polygon(&mut self, points: &[Pos2], color: Color32) {
        if points.len() < 3 || color.a() == 0 {
            return;
        }
        let mut scratch = std::mem::take(&mut self.scratch);
        let st = (point(0., 1.), point(0., 1.), point(0., 1.));
        let mesh = self.mesh(color, points[0]);
        tt_chart::render::triangulate(points, &mut scratch, |a, b, c| {
            mesh.push_triangle((pt(a), pt(b), pt(c)), st);
        });
        self.scratch = scratch;
    }

    fn icon(&mut self, center: Pos2, glyph: char, size: f32, color: Color32) -> Rect {
        let line = self.shape(
            &glyph.to_string(),
            size,
            color,
            self.fonts.icons.clone(),
            false,
            false,
        );
        let rect = Align2::CENTER_CENTER
            .anchor_size(center, Vec2::new(f32::from(line.width), size * LINE_HEIGHT));
        self.push(Item::Text(Box::new((line, rect.min, size))));
        rect
    }
}
