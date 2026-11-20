use super::Canvas;
use egui::epaint::{Mesh, PathShape, PathStroke, RectShape};
use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2};

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
    /// Font family for bold text, if the app registered one.
    bold_family: Option<FontFamily>,
    /// Font family for icon glyphs.
    icon_family: Option<FontFamily>,
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
            bold_family: None,
            icon_family: None,
            batches: vec![(clip, Vec::new())],
        }
    }

    /// Uses `family` for bold text (falls back to the regular face if `None`).
    pub fn with_bold_family(mut self, family: Option<FontFamily>) -> Self {
        self.bold_family = family;
        self
    }

    /// Uses `family` for [`Canvas::icon`] glyphs (icons are skipped if `None`).
    pub fn with_icon_family(mut self, family: Option<FontFamily>) -> Self {
        self.icon_family = family;
        self
    }

    fn font(&self, size: f32, bold: bool) -> FontId {
        match (&self.bold_family, bold) {
            (Some(f), true) => FontId::new(size, f.clone()),
            _ => FontId::proportional(size),
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

    /// A 4×4 texture whose only opaque texel is (0, 0): row 0 and column 0
    /// are the "1 on, 3 off" dot pattern; with repeat wrapping one quad draws
    /// a whole dotted line. Created once per egui context.
    fn dots_texture(&self) -> egui::TextureId {
        let ctx = self.painter.ctx();
        let id = egui::Id::unique("tt-chart-dots-texture");
        if let Some(h) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
            return h.id();
        }
        let mut image = egui::ColorImage::filled([4, 4], Color32::TRANSPARENT);
        image.pixels[0] = Color32::WHITE;
        let options = egui::TextureOptions {
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Nearest,
            wrap_mode: egui::TextureWrapMode::Repeat,
            mipmap_mode: None,
        };
        let handle = ctx.load_texture("tt-chart-dots", image, options);
        let tex = handle.id();
        ctx.data_mut(|d| d.insert_temp(id, handle));
        tex
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

    fn fill_between(&mut self, top: &[Pos2], bottom: &[Pos2], color: Color32) {
        let n = top.len().min(bottom.len());
        if n < 2 || color.a() == 0 {
            return;
        }
        self.flush_mesh();
        let mut mesh = Mesh::default();
        mesh.reserve_vertices(n * 2);
        mesh.reserve_triangles((n - 1) * 2);
        for i in 0..n {
            mesh.colored_vertex(top[i], color);
            mesh.colored_vertex(bottom[i], color);
        }
        for i in 0..(n as u32 - 1) {
            let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(c, b, d);
        }
        self.push_shape(Shape::mesh(mesh));
    }

    fn fill_circle(&mut self, center: Pos2, radius: f32, color: Color32) {
        if self.clip.expand(radius).contains(center) {
            self.add(Shape::circle_filled(center, radius, color));
        }
    }

    fn dotted_line(&mut self, a: Pos2, b: Pos2, color: Color32) {
        let ppp = self.ppp;
        let px = 1.0 / ppp;
        // Texel centers of row 0 / column 0 (v or u = 0.5 / 4).
        let (rect, uv) = if a.y == b.y {
            let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
            let len_px = (x1 - x0) * ppp;
            (
                Rect::from_min_max(pos2(x0, a.y - px / 2.0), pos2(x1, a.y + px / 2.0)),
                Rect::from_min_max(pos2(0.0, 0.125), pos2(len_px / 4.0, 0.125)),
            )
        } else if a.x == b.x {
            let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
            let len_px = (y1 - y0) * ppp;
            (
                Rect::from_min_max(pos2(a.x - px / 2.0, y0), pos2(a.x + px / 2.0, y1)),
                Rect::from_min_max(pos2(0.125, 0.0), pos2(0.125, len_px / 4.0)),
            )
        } else {
            self.dashed_line(a, b, px, color, px, 3.0 * px);
            return;
        };
        if !rect.intersects(self.clip) {
            return;
        }
        let mut mesh = Mesh::with_texture(self.dots_texture());
        mesh.add_rect_with_uv(rect, uv, color);
        self.add(Shape::mesh(mesh));
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
        self.flush_mesh();
        let span = (baseline_y - gradient_top_y).max(1.0);
        let color_at = |y: f32| {
            let t = ((y - gradient_top_y) / span).clamp(0.0, 1.0);
            tt_theme::mix(top, bottom, t)
        };
        let mut mesh = Mesh::default();
        mesh.reserve_vertices(points.len() * 2);
        mesh.reserve_triangles((points.len() - 1) * 2);
        for p in points {
            mesh.colored_vertex(*p, color_at(p.y));
            mesh.colored_vertex(pos2(p.x, baseline_y), bottom);
        }
        for i in 0..(points.len() as u32 - 1) {
            let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(c, b, d);
        }
        self.push_shape(Shape::mesh(mesh));
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
        let font = self.font(size, bold);
        let galley = self.painter.layout_no_wrap(text.to_owned(), font, color);
        let rect = anchor.anchor_size(pos, galley.size());
        self.add(Shape::galley(rect.min, galley, color));
        rect
    }

    fn icon(&mut self, center: Pos2, glyph: char, size: f32, color: Color32) -> Rect {
        let Some(family) = self.icon_family.clone() else {
            return Rect::from_center_size(center, egui::Vec2::splat(size));
        };
        let galley =
            self.painter
                .layout_no_wrap(glyph.to_string(), FontId::new(size, family), color);
        let rect = Align2::CENTER_CENTER.anchor_size(center, galley.size());
        self.add(Shape::galley(rect.min, galley, color));
        rect
    }

    fn text_size_styled(&mut self, text: &str, size: f32, bold: bool) -> Vec2 {
        let font = self.font(size, bold);
        self.painter
            .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER)
            .size()
    }
}
