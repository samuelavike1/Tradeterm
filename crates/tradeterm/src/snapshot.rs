//! "Take a snapshot" (camera on the app bar): the active chart rendered
//! off-screen to a PNG, TradingView-style: a header line (symbol, interval,
//! source, date) over the chart without the crosshair.
//!
//! The chart draws through `tt_chart::render::Canvas`; [`SkiaCanvas`]
//! implements it on tiny-skia (CPU, any platform), with text and icons from
//! TrueType outlines (ttf-parser): the UI font from the system, the icons
//! from the bundled Lucide font. Nothing here runs while the chart paints.

use emath::{Align2, Pos2, Rect, Vec2, pos2, vec2};
use std::sync::OnceLock;
use tiny_skia::{
    FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Mask, Paint, PathBuilder, Pixmap,
    Point, SpreadMode, Stroke, StrokeDash, Transform,
};
use tt_chart::render::{Canvas, triangulate};
use tt_chart_gpui::ChartSource;
use tt_theme::Theme;

use crate::app::TabChart;
use ecolor::Color32;
use gpui_kit::*;

/// Header strip above the chart, points.
const HEADER_H: f32 = 32.0;
/// Header text size, points.
const HEADER_TEXT: f32 = 13.0;
/// Text box height per point of font size (matches `GpuiCanvas`).
const LINE_HEIGHT: f32 = 1.25;

// ---- fonts ------------------------------------------------------------------

/// TrueType data of one face (owned; the face is parsed per use, which is
/// cheap: ttf-parser only reads table offsets).
struct FontData {
    bytes: Vec<u8>,
    index: u32,
}

impl FontData {
    fn face(&self) -> Option<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.bytes, self.index).ok()
    }
}

struct Fonts {
    regular: Option<FontData>,
    bold: Option<FontData>,
    icons: Option<FontData>,
}

/// System UI font files, most preferred first: (regular, semibold).
const UI_FONT_FILES: &[(&str, &str)] = &[
    (
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\seguisb.ttf",
    ),
    (
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\arialbd.ttf",
    ),
    (
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    ),
    (
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ),
    (
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
    ),
];

fn load(path: &str) -> Option<FontData> {
    let bytes = std::fs::read(path).ok()?;
    let f = FontData { bytes, index: 0 };
    f.face().is_some().then_some(f)
}

fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let (regular, bold) = UI_FONT_FILES
            .iter()
            .find_map(|(r, b)| load(r).map(|r| (Some(r), load(b))))
            .unwrap_or((None, None));
        let icons = FontData {
            bytes: lucide_icons::LUCIDE_FONT_BYTES.to_vec(),
            index: 0,
        };
        Fonts {
            regular,
            bold,
            icons: icons.face().is_some().then_some(icons),
        }
    })
}

/// Collects a glyph outline into a tiny-skia path (font units → points:
/// scaled, y flipped, moved to the pen position).
struct Outline<'a> {
    pb: &'a mut PathBuilder,
    x: f32,
    y: f32,
    scale: f32,
}

impl Outline<'_> {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.scale, self.y - y * self.scale)
    }
}

impl ttf_parser::OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.p(x, y);
        self.pb.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.p(x, y);
        self.pb.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.p(x1, y1);
        let (x, y) = self.p(x, y);
        self.pb.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.p(x1, y1);
        let (x2, y2) = self.p(x2, y2);
        let (x, y) = self.p(x, y);
        self.pb.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// Advance width of `text` at `size` (no kerning; chart labels are short).
fn text_width(face: &ttf_parser::Face<'_>, text: &str, size: f32) -> f32 {
    let scale = size / face.units_per_em() as f32;
    text.chars()
        .map(|c| {
            face.glyph_index(c)
                .and_then(|g| face.glyph_hor_advance(g))
                .unwrap_or(face.units_per_em() / 2) as f32
                * scale
        })
        .sum()
}

// ---- canvas -----------------------------------------------------------------

/// `tt_chart::render::Canvas` on a tiny-skia pixmap. Coordinates are chart
/// points; `origin` maps to the pixmap's top-left (below the header).
pub struct SkiaCanvas {
    pix: Pixmap,
    ppp: f32,
    transform: Transform,
    clip: Option<Mask>,
}

fn color(c: Color32) -> tiny_skia::Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    tiny_skia::Color::from_rgba8(r, g, b, a)
}

fn paint(c: Color32, aa: bool) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color(c));
    p.anti_alias = aa;
    p
}

fn rect_path(r: Rect) -> Option<tiny_skia::Path> {
    tiny_skia::Rect::from_ltrb(r.min.x, r.min.y, r.max.x, r.max.y).map(PathBuilder::from_rect)
}

fn poly_path(points: &[Pos2], close: bool) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let (first, rest) = points.split_first()?;
    pb.move_to(first.x, first.y);
    for p in rest {
        pb.line_to(p.x, p.y);
    }
    if close {
        pb.close();
    }
    pb.finish()
}

impl SkiaCanvas {
    /// A `size` (points) canvas at `ppp` pixels per point; chart point
    /// `origin` lands at pixmap point (0, `top`).
    fn new(size: Vec2, ppp: f32, origin: Pos2, top: f32) -> Option<Self> {
        let w = (size.x * ppp).round().max(1.0) as u32;
        let h = (size.y * ppp).round().max(1.0) as u32;
        let pix = Pixmap::new(w, h)?;
        let transform =
            Transform::from_row(ppp, 0.0, 0.0, ppp, -origin.x * ppp, (top - origin.y) * ppp);
        Some(Self {
            pix,
            ppp,
            transform,
            clip: None,
        })
    }

    fn fill(&mut self, path: &tiny_skia::Path, p: &Paint<'_>) {
        self.pix.fill_path(
            path,
            p,
            FillRule::Winding,
            self.transform,
            self.clip.as_ref(),
        );
    }

    fn stroke(&mut self, path: &tiny_skia::Path, p: &Paint<'_>, s: &Stroke) {
        self.pix
            .stroke_path(path, p, s, self.transform, self.clip.as_ref());
    }

    fn line_width(&self, w: f32) -> f32 {
        w.max(1.0 / self.ppp)
    }

    /// Glyph run from `face` with its top-left text box corner at `min`.
    fn glyphs(&mut self, font: &FontData, min: Pos2, text: &str, size: f32, c: Color32) {
        let Some(face) = font.face() else {
            return;
        };
        let upm = face.units_per_em() as f32;
        let scale = size / upm;
        let (asc, desc) = (face.ascender() as f32, face.descender() as f32);
        // gpui centers the font's ascent + descent in the line box.
        let lh = size * LINE_HEIGHT;
        let baseline = min.y + (lh - (asc - desc) * scale) / 2.0 + asc * scale;
        let mut pb = PathBuilder::new();
        let mut x = min.x;
        for ch in text.chars() {
            let Some(g) = face.glyph_index(ch) else {
                x += upm / 2.0 * scale;
                continue;
            };
            let mut o = Outline {
                pb: &mut pb,
                x,
                y: baseline,
                scale,
            };
            face.outline_glyph(g, &mut o);
            x += face.glyph_hor_advance(g).unwrap_or(0) as f32 * scale;
        }
        if let Some(path) = pb.finish() {
            self.fill(&path, &paint(c, true));
        }
    }

    fn ui_font(bold: bool) -> Option<&'static FontData> {
        let f = fonts();
        if bold {
            f.bold.as_ref().or(f.regular.as_ref())
        } else {
            f.regular.as_ref()
        }
    }

    pub fn encode_png(&self) -> Option<Vec<u8>> {
        self.pix.encode_png().ok()
    }
}

impl Canvas for SkiaCanvas {
    fn pixels_per_point(&self) -> f32 {
        self.ppp
    }

    fn set_clip(&mut self, rect: Rect) {
        let (w, h) = (self.pix.width(), self.pix.height());
        self.clip = rect_path(rect).and_then(|path| {
            let mut m = Mask::new(w, h)?;
            m.fill_path(&path, FillRule::Winding, false, self.transform);
            Some(m)
        });
    }

    fn fill_rect(&mut self, rect: Rect, color: Color32) {
        if color.a() == 0 {
            return;
        }
        if let Some(p) = rect_path(rect) {
            self.fill(&p, &paint(color, false));
        }
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color32) {
        let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
        if r <= 0.0 {
            return self.fill_rect(rect, color);
        }
        // Quarter circles as cubics (k = 0.5523).
        let k = r * 0.552_284_8;
        let (l, t, rt, b) = (rect.min.x, rect.min.y, rect.max.x, rect.max.y);
        let mut pb = PathBuilder::new();
        pb.move_to(l + r, t);
        pb.line_to(rt - r, t);
        pb.cubic_to(rt - r + k, t, rt, t + r - k, rt, t + r);
        pb.line_to(rt, b - r);
        pb.cubic_to(rt, b - r + k, rt - r + k, b, rt - r, b);
        pb.line_to(l + r, b);
        pb.cubic_to(l + r - k, b, l, b - r + k, l, b - r);
        pb.line_to(l, t + r);
        pb.cubic_to(l, t + r - k, l + r - k, t, l + r, t);
        pb.close();
        if let Some(p) = pb.finish() {
            self.fill(&p, &paint(color, true));
        }
    }

    fn stroke_rect(&mut self, rect: Rect, width: f32, color: Color32) {
        let w = self.line_width(width);
        let (l, t, r, b) = (rect.min.x, rect.min.y, rect.max.x, rect.max.y);
        for e in [
            Rect::from_min_max(pos2(l, t), pos2(r, t + w)),
            Rect::from_min_max(pos2(l, b - w), pos2(r, b)),
            Rect::from_min_max(pos2(l, t + w), pos2(l + w, b - w)),
            Rect::from_min_max(pos2(r - w, t + w), pos2(r, b - w)),
        ] {
            self.fill_rect(e, color);
        }
    }

    fn line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32) {
        self.polyline(&[a, b], width, color);
    }

    fn dashed_line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32, dash: f32, gap: f32) {
        if color.a() == 0 || dash <= 0.0 {
            return;
        }
        let Some(path) = poly_path(&[a, b], false) else {
            return;
        };
        let stroke = Stroke {
            width: self.line_width(width),
            line_cap: LineCap::Butt,
            dash: StrokeDash::new(vec![dash, gap.max(0.01)], 0.0),
            ..Stroke::default()
        };
        self.stroke(&path, &paint(color, true), &stroke);
    }

    fn polyline(&mut self, points: &[Pos2], width: f32, color: Color32) {
        if points.len() < 2 || color.a() == 0 {
            return;
        }
        let Some(path) = poly_path(points, false) else {
            return;
        };
        let w = self.line_width(width);
        // Hairline axis-aligned segments stay crisp without anti-aliasing.
        let crisp = points.len() == 2
            && (points[0].x == points[1].x || points[0].y == points[1].y)
            && w * self.ppp <= 1.5;
        let stroke = Stroke {
            width: w,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        self.stroke(&path, &paint(color, !crisp), &stroke);
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
        let (first, last) = (points[0], points[points.len() - 1]);
        let mut pts = Vec::with_capacity(points.len() + 2);
        pts.extend_from_slice(points);
        pts.push(pos2(last.x, baseline_y));
        pts.push(pos2(first.x, baseline_y));
        let Some(path) = poly_path(&pts, true) else {
            return;
        };
        let shader = LinearGradient::new(
            Point::from_xy(first.x, gradient_top_y),
            Point::from_xy(first.x, baseline_y),
            vec![
                GradientStop::new(0.0, color(top)),
                GradientStop::new(1.0, color(bottom)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        );
        let mut p = Paint::default();
        p.anti_alias = true;
        match shader {
            Some(s) => p.shader = s,
            None => p.set_color(color(top)),
        }
        self.fill(&path, &p);
    }

    fn fill_between(&mut self, top: &[Pos2], bottom: &[Pos2], color: Color32) {
        let n = top.len().min(bottom.len());
        if n < 2 || color.a() == 0 {
            return;
        }
        // One quad per step: the lines may cross (a self-intersecting
        // polygon would cancel out under even-odd and double under winding).
        let mut pb = PathBuilder::new();
        for i in 1..n {
            let q = [top[i - 1], top[i], bottom[i], bottom[i - 1]];
            pb.move_to(q[0].x, q[0].y);
            for p in &q[1..] {
                pb.line_to(p.x, p.y);
            }
            pb.close();
        }
        if let Some(path) = pb.finish() {
            self.fill(&path, &paint(color, true));
        }
    }

    fn fill_circle(&mut self, center: Pos2, radius: f32, color: Color32) {
        if let Some(p) = PathBuilder::from_circle(center.x, center.y, radius) {
            self.fill(&p, &paint(color, true));
        }
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
        let w = self.text_size_styled(text, size, bold).x;
        let rect = anchor.anchor_size(pos, vec2(w, size * LINE_HEIGHT));
        if let Some(font) = Self::ui_font(bold) {
            self.glyphs(font, rect.min, text, size, color);
        }
        rect
    }

    fn text_size_styled(&mut self, text: &str, size: f32, bold: bool) -> Vec2 {
        let w = Self::ui_font(bold)
            .and_then(FontData::face)
            .map_or(text.chars().count() as f32 * size * 0.55, |f| {
                text_width(&f, text, size)
            });
        vec2(w, size * LINE_HEIGHT)
    }

    fn icon(&mut self, center: Pos2, glyph: char, size: f32, color: Color32) -> Rect {
        let Some(font) = fonts().icons.as_ref() else {
            return Rect::from_center_size(center, Vec2::splat(size));
        };
        let mut s = [0u8; 4];
        let text = glyph.encode_utf8(&mut s);
        let w = font.face().map_or(size, |f| text_width(&f, text, size));
        let rect = Align2::CENTER_CENTER.anchor_size(center, vec2(w, size * LINE_HEIGHT));
        self.glyphs(font, rect.min, text, size, color);
        rect
    }

    fn fill_polygon(&mut self, points: &[Pos2], color: Color32) {
        if points.len() < 3 || color.a() == 0 {
            return;
        }
        // Triangles like the gpui backend, so both agree on odd shapes.
        let mut scratch = Vec::new();
        let mut pb = PathBuilder::new();
        triangulate(points, &mut scratch, |a, b, c| {
            pb.move_to(a.x, a.y);
            pb.line_to(b.x, b.y);
            pb.line_to(c.x, c.y);
            pb.close();
        });
        if let Some(path) = pb.finish() {
            self.fill(&path, &paint(color, true));
        }
    }
}

// ---- rendering --------------------------------------------------------------

/// "YYYY-MM-DD HH:MM UTC" for a Unix time in milliseconds.
pub fn utc_stamp(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60
    )
}

/// A rendered snapshot.
pub struct Snapshot {
    pub png: Vec<u8>,
    /// Suggested file name: "BTCUSDT_15m_2026-11-21_04-00.png".
    pub file_name: String,
}

/// Renders `chart` as it is on screen (its last layout), at `ppp` pixels
/// per point, with the header line. `None` before the chart's first paint.
pub fn render(chart: &TabChart, ppp: f32, cx: &mut App) -> Option<Snapshot> {
    let state = chart.read(cx).state.clone();
    state.update(cx, |s, _| {
        let base = s.theme.clone();
        let (view, data) = s.source.parts();
        let l = view.last_layout();
        let full = l.plot.union(l.price_axis).union(l.time_axis);
        if full.width() < 1.0 || full.height() < 1.0 || data.series.is_empty() {
            return None;
        }
        let theme: Theme = view.style.theme(&base);
        let size = vec2(full.width(), full.height() + HEADER_H);
        let mut c = SkiaCanvas::new(size, ppp, full.min, HEADER_H)?;
        c.pix.fill(color(theme.bg));
        // Chart: the base layer, then the overlay without crosshair.
        view.render_base(&mut c, full, ppp, data, &theme);
        let input = tt_chart::RenderInput {
            crosshair: None,
            legend_hover: None,
        };
        view.render_overlay(&mut c, data, &theme, input);
        // Header: title left, time right, a border line under it.
        c.clip = None;
        let now = crate::source::now_ms();
        let stamp = utc_stamp(now);
        let y = full.min.y - HEADER_H / 2.0;
        let pad = tt_ui::tokens::S3;
        c.fill_rect(
            Rect::from_min_max(
                pos2(full.min.x, full.min.y - 1.0),
                pos2(full.max.x, full.min.y),
            ),
            theme.border,
        );
        c.text_styled(
            pos2(full.min.x + pad, y),
            Align2::LEFT_CENTER,
            data.title,
            HEADER_TEXT,
            theme.text,
            true,
        );
        c.text_styled(
            pos2(full.max.x - pad, y),
            Align2::RIGHT_CENTER,
            &stamp,
            HEADER_TEXT,
            theme.text_muted,
            false,
        );
        let name: String = data
            .title
            .split(" · ")
            .take(2)
            .collect::<Vec<_>>()
            .join("_")
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect();
        let file_name = format!(
            "{}_{}.png",
            if name.is_empty() { "chart" } else { &name },
            stamp
                .trim_end_matches(" UTC")
                .replace(' ', "_")
                .replace(':', "-")
        );
        Some(Snapshot {
            png: c.encode_png()?,
            file_name,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn utc_stamps() {
        assert_eq!(utc_stamp(0), "1970-01-01 00:00 UTC");
        // 2026-11-21 04:05:00 UTC
        assert_eq!(utc_stamp(1_795_233_900_000), "2026-11-21 04:05 UTC");
        assert_eq!(utc_stamp(951_782_400_000), "2000-02-29 00:00 UTC");
    }

    #[::core::prelude::v1::test]
    fn canvas_draws_pixels_where_asked() {
        let mut c = SkiaCanvas::new(vec2(20.0, 10.0), 2.0, pos2(100.0, 50.0), 0.0).unwrap();
        c.fill_rect(
            Rect::from_min_max(pos2(100.0, 50.0), pos2(105.0, 55.0)),
            Color32::RED,
        );
        let px = |c: &SkiaCanvas, x: u32, y: u32| c.pix.pixel(x, y).unwrap();
        assert_eq!(px(&c, 1, 1).red(), 255);
        assert_eq!(px(&c, 12, 12).alpha(), 0);
        c.set_clip(Rect::from_min_max(pos2(110.0, 50.0), pos2(120.0, 60.0)));
        c.fill_rect(
            Rect::from_min_max(pos2(100.0, 50.0), pos2(120.0, 60.0)),
            Color32::BLUE,
        );
        assert_eq!(px(&c, 1, 1).red(), 255);
        assert_eq!(px(&c, 30, 5).blue(), 255);
        assert!(c.encode_png().is_some_and(|b| b.starts_with(b"\x89PNG")));
    }
}
