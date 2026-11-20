use super::Canvas;
use egui::epaint::{Mesh, PathShape, PathStroke, RectShape};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2};

/// [`Canvas`] on top of `egui::Painter`.
///
/// Consecutive filled rectangles and axis-aligned lines (the bulk of a
/// candlestick chart) are merged into a single mesh, so thousands of bars cost
/// one shape instead of thousands. Call [`EguiCanvas::finish`] to submit.
pub struct EguiCanvas {
    painter: Painter,
    ppp: f32,
    clip: Rect,
    mesh: Mesh,
    /// Shapes grouped by clip rect, in draw order.
    batches: Vec<(Rect, Vec<Shape>)>,
}

impl EguiCanvas {
    pub fn new(painter: Painter) -> Self {
        let clip = painter.clip_rect();
        let ppp = painter.pixels_per_point();
        Self {
            painter,
            ppp,
            clip,
            mesh: Mesh::default(),
            batches: vec![(clip, Vec::new())],
        }
    }

    fn flush_mesh(&mut self) {
        if !self.mesh.is_empty() {
            let mesh = std::mem::take(&mut self.mesh);
            self.push_shape(Shape::mesh(mesh));
        }
    }

    fn push_shape(&mut self, shape: Shape) {
        match self.batches.last_mut() {
            Some((clip, shapes)) if *clip == self.clip => shapes.push(shape),
            _ => self.batches.push((self.clip, vec![shape])),
        }
    }

    fn add(&mut self, shape: Shape) {
        self.flush_mesh();
        self.push_shape(shape);
    }

    /// Submits everything to the painter.
    pub fn finish(mut self) {
        self.flush_mesh();
        for (clip, shapes) in self.batches {
            if !shapes.is_empty() {
                self.painter.with_clip_rect(clip).extend(shapes);
            }
        }
    }
}

impl Canvas for EguiCanvas {
    fn pixels_per_point(&self) -> f32 {
        self.ppp
    }

    fn set_clip(&mut self, rect: Rect) {
        let rect = rect.intersect(self.painter.clip_rect());
        if rect != self.clip {
            self.flush_mesh();
            self.clip = rect;
        }
    }

    fn fill_rect(&mut self, rect: Rect, color: Color32) {
        if color.a() == 0 || !rect.is_positive() {
            return;
        }
        // Cheap CPU-side culling keeps the mesh small when zoomed in.
        if !rect.intersects(self.clip) {
            return;
        }
        self.mesh.add_colored_rect(rect, color);
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color32) {
        self.add(Shape::Rect(RectShape::filled(rect, radius, color)));
    }

    fn stroke_rect(&mut self, rect: Rect, width: f32, color: Color32) {
        let w = width
            .min(rect.width() / 2.0)
            .min(rect.height() / 2.0)
            .max(0.0);
        if w <= 0.0 {
            self.fill_rect(rect, color);
            return;
        }
        let (l, r, t, b) = (rect.left(), rect.right(), rect.top(), rect.bottom());
        self.fill_rect(Rect::from_min_max(pos2(l, t), pos2(r, t + w)), color);
        self.fill_rect(Rect::from_min_max(pos2(l, b - w), pos2(r, b)), color);
        self.fill_rect(
            Rect::from_min_max(pos2(l, t + w), pos2(l + w, b - w)),
            color,
        );
        self.fill_rect(
            Rect::from_min_max(pos2(r - w, t + w), pos2(r, b - w)),
            color,
        );
    }

    fn line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32) {
        let h = width / 2.0;
        if a.x == b.x {
            let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
            self.fill_rect(
                Rect::from_min_max(pos2(a.x - h, y0), pos2(a.x + h, y1)),
                color,
            );
        } else if a.y == b.y {
            let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
            self.fill_rect(
                Rect::from_min_max(pos2(x0, a.y - h), pos2(x1, a.y + h)),
                color,
            );
        } else {
            self.add(Shape::line_segment([a, b], Stroke::new(width, color)));
        }
    }

    fn dashed_line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32, dash: f32, gap: f32) {
        let len = a.distance(b);
        if len <= 0.0 {
            return;
        }
        let dir = (b - a) / len;
        let mut t = 0.0;
        while t < len {
            let e = (t + dash).min(len);
            self.line(a + dir * t, a + dir * e, width, color);
            t += dash + gap;
        }
    }

    fn polyline(&mut self, points: &[Pos2], width: f32, color: Color32) {
        if points.len() < 2 {
            return;
        }
        self.add(Shape::Path(PathShape::line(
            points.to_vec(),
            PathStroke::new(width, color),
        )));
    }

    fn fill_area(&mut self, points: &[Pos2], baseline_y: f32, top: Color32, bottom: Color32) {
        if points.len() < 2 {
            return;
        }
        self.flush_mesh();
        let mut mesh = Mesh::default();
        mesh.reserve_vertices(points.len() * 2);
        mesh.reserve_triangles((points.len() - 1) * 2);
        for p in points {
            mesh.colored_vertex(*p, top);
            mesh.colored_vertex(pos2(p.x, baseline_y), bottom);
        }
        for i in 0..(points.len() as u32 - 1) {
            let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(c, b, d);
        }
        self.push_shape(Shape::mesh(mesh));
    }

    fn text(&mut self, pos: Pos2, anchor: Align2, text: &str, size: f32, color: Color32) -> Rect {
        let galley = self
            .painter
            .layout_no_wrap(text.to_owned(), FontId::monospace(size), color);
        let rect = anchor.anchor_size(pos, galley.size());
        self.add(Shape::galley(rect.min, galley, color));
        rect
    }

    fn text_size(&mut self, text: &str, size: f32) -> Vec2 {
        self.painter
            .layout_no_wrap(
                text.to_owned(),
                FontId::monospace(size),
                Color32::PLACEHOLDER,
            )
            .size()
    }
}
