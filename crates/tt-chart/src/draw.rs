//! Chart scene drawing. Uses only the [`Canvas`] trait, never egui directly.

use crate::bars::{DrawBar, max_volume};
use crate::render::Canvas;
use crate::scale::{
    self, format_countdown, format_full_time, format_percent, format_price, format_signed_price,
    format_volume,
};
use crate::time_axis;
use crate::viewport::Viewport;
use crate::{ChartStyle, ChartType, ScaleMode};
use egui::{Align2, Color32, Pos2, Rect, pos2, vec2};
use tt_core::{Candle, CandleSeries};
use tt_theme::{Theme, with_alpha};

/// Everything needed to draw one frame.
pub(crate) struct Scene<'a> {
    /// Price viewport; `vp.plot` is the price area (above the volume pane).
    pub vp: Viewport,
    pub theme: &'a Theme,
    pub style: &'a ChartStyle,
    pub chart_type: ChartType,
    /// Whole widget.
    pub full: Rect,
    /// Plot area including the volume pane.
    pub plot: Rect,
    pub volume_rect: Option<Rect>,
    pub price_axis: Rect,
    pub time_axis: Rect,
    /// Series being displayed (raw or Heikin Ashi).
    pub display: &'a CandleSeries,
    /// Last *real* candle, for the last-price line.
    pub last_raw: Option<Candle>,
    pub bars: &'a [DrawBar],
    /// `true` when bars are per-pixel-column aggregates.
    pub decimated: bool,
    pub crosshair: Option<Pos2>,
    pub precision: usize,
    pub tick_size: Option<f64>,
    pub bar_interval_ms: i64,
    /// Current time, for the bar-close countdown (None = no countdown).
    pub now_ms: Option<i64>,
    /// Bar length used for the countdown (the timeframe's, if known).
    pub countdown_interval_ms: Option<i64>,
    pub title: &'a str,
}

pub(crate) fn draw(c: &mut dyn Canvas, s: &Scene<'_>) {
    let t = s.theme;
    c.set_clip(s.full);
    c.fill_rect(s.full, t.bg);

    if s.display.is_empty() {
        c.text(
            s.plot.center(),
            Align2::CENTER_CENTER,
            "Waiting for data…",
            s.style.font_size + 1.0,
            t.text_muted,
        );
        readout(c, s, None);
        return;
    }

    let price_ticks = price_ticks(s);
    let time_ticks = time_ticks(c, s);

    // Grid: one line per price label and per time label, like TradingView.
    c.set_clip(s.plot);
    let ppp = c.pixels_per_point();
    for (y, _) in &price_ticks {
        let y = crisp(*y, ppp);
        grid_line(c, s, pos2(s.plot.left(), y), pos2(s.plot.right(), y));
    }
    for tick in &time_ticks {
        grid_line(
            c,
            s,
            pos2(tick.x, s.plot.top()),
            pos2(tick.x, s.plot.bottom()),
        );
    }

    if let Some(vr) = s.volume_rect {
        c.set_clip(vr);
        volume(c, s, vr);
    }
    c.set_clip(s.vp_rect());
    series(c, s);
    c.set_clip(s.plot);
    last_price_line(c, s);

    // Axis tags are computed before the tick labels so labels underneath can
    // be hidden; the crosshair tag is drawn last, on top.
    c.set_clip(s.full);
    let hovered = s.crosshair.and_then(|p| s.vp.bar_at(p.x));
    let mut tags: Vec<Tag> = Vec::new();
    if let Some(tag) = last_price_tag(c, s) {
        tags.push(tag);
    }
    let mut time_tag = None;
    if let Some(p) = s.crosshair {
        let (price_tag, ttag) = crosshair(c, s, p, hovered);
        tags.push(price_tag);
        time_tag = ttag;
    }

    price_labels(c, s, &price_ticks, &tags);
    time_labels(c, s, &time_ticks, time_tag.as_ref());
    for tag in tags.iter().chain(time_tag.iter()) {
        draw_tag(c, s, tag);
    }
    readout(c, s, hovered.or(s.display.len().checked_sub(1)));
}

impl Scene<'_> {
    fn vp_rect(&self) -> Rect {
        Rect::from_min_max(
            pos2(self.vp.plot.left, self.vp.plot.top),
            pos2(self.vp.plot.right, self.vp.plot.bottom),
        )
    }

    fn colors(&self, up: bool) -> (Color32, Color32) {
        if up {
            (self.theme.up, self.theme.up_wick)
        } else {
            (self.theme.down, self.theme.down_wick)
        }
    }
}

/// Snaps a coordinate to the center of a physical pixel for crisp 1px lines.
fn crisp(v: f32, ppp: f32) -> f32 {
    ((v * ppp).floor() + 0.5) / ppp
}

/// Pixel-aligned bar geometry shared by all bar-like chart types.
struct BarGeom {
    ppp: f32,
    /// Body width in physical pixels (odd, ≥ 1).
    body_px: f32,
    /// Wick/line width in physical pixels.
    wick_px: f32,
}

impl BarGeom {
    fn new(spacing: f32, ppp: f32) -> Self {
        let mut body_px = (spacing * ppp * 0.72).floor().max(1.0);
        if body_px as i32 % 2 == 0 {
            body_px -= 1.0;
        }
        let body_px = body_px.max(1.0);
        Self {
            ppp,
            body_px,
            wick_px: ppp.round().max(1.0).min(body_px),
        }
    }

    /// Physical-pixel center column for screen x.
    fn center_px(&self, x: f32) -> f32 {
        (x * self.ppp).round()
    }

    /// Rect `width_px` physical pixels wide, centered on `cx_px`, between y0 and y1.
    fn column(&self, cx_px: f32, width_px: f32, y0: f32, y1: f32) -> Rect {
        let x0 = (cx_px - (width_px / 2.0).floor()) / self.ppp;
        Rect::from_min_max(
            pos2(x0, y0.min(y1)),
            pos2(x0 + width_px / self.ppp, y0.max(y1)),
        )
    }

    /// Snaps y0..y1 to pixels, at least one pixel tall.
    fn span(&self, a: f32, b: f32) -> (f32, f32) {
        let y0 = (a.min(b) * self.ppp).round() / self.ppp;
        let y1 = ((a.max(b) * self.ppp).round() / self.ppp).max(y0 + 1.0 / self.ppp);
        (y0, y1)
    }
}

fn series(c: &mut dyn Canvas, s: &Scene<'_>) {
    let g = BarGeom::new(s.vp.bar_spacing, c.pixels_per_point());
    if s.decimated {
        decimated(c, s, &g);
        return;
    }
    match s.chart_type {
        ChartType::Candles | ChartType::HeikinAshi => candles(c, s, &g, false),
        ChartType::HollowCandles => candles(c, s, &g, true),
        ChartType::Bars => ohlc_bars(c, s, &g),
        ChartType::Line => line(c, s, false),
        ChartType::Area => line(c, s, true),
    }
}

fn candles(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom, hollow: bool) {
    let vp = &s.vp;
    for b in s.bars {
        let rising = b.close >= b.open;
        // Hollow candles: color by close vs previous close, hollow when rising.
        let (body, wick) = if hollow {
            s.colors(b.close >= b.prev_close)
        } else {
            s.colors(rising)
        };
        let cx = g.center_px(b.x);
        let (yh, yl) = (vp.y_of(b.high), vp.y_of(b.low));
        let (y0, y1) = g.span(vp.y_of(b.open), vp.y_of(b.close));

        if g.body_px <= g.wick_px {
            c.fill_rect(g.column(cx, g.wick_px, yh, yl), wick);
            continue;
        }
        if hollow && rising {
            // Wick above and below the body only, then an outlined body.
            c.fill_rect(g.column(cx, g.wick_px, yh, y0), wick);
            c.fill_rect(g.column(cx, g.wick_px, y1, yl), wick);
            c.stroke_rect(g.column(cx, g.body_px, y0, y1), g.wick_px / g.ppp, body);
        } else {
            c.fill_rect(g.column(cx, g.wick_px, yh, yl), wick);
            c.fill_rect(g.column(cx, g.body_px, y0, y1), body);
        }
    }
}

fn ohlc_bars(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom) {
    let vp = &s.vp;
    let tick_px = ((g.body_px - g.wick_px) / 2.0).floor().max(1.0);
    let w = g.wick_px / g.ppp;
    for b in s.bars {
        let (color, _) = s.colors(b.close >= b.open);
        let cx = g.center_px(b.x);
        c.fill_rect(
            g.column(cx, g.wick_px, vp.y_of(b.high), vp.y_of(b.low)),
            color,
        );
        let half_wick = (g.wick_px / 2.0).floor();
        let yo = crisp(vp.y_of(b.open), g.ppp);
        let yc = crisp(vp.y_of(b.close), g.ppp);
        // Open tick to the left, close tick to the right.
        let left = (cx - half_wick - tick_px) / g.ppp;
        let right = (cx - half_wick + g.wick_px) / g.ppp;
        let tick = |x0: f32, y: f32| {
            Rect::from_min_max(
                pos2(x0, y - w / 2.0),
                pos2(x0 + tick_px / g.ppp, y + w / 2.0),
            )
        };
        c.fill_rect(tick(left, yo), color);
        c.fill_rect(tick(right, yc), color);
    }
}

fn line_color(s: &Scene<'_>) -> Color32 {
    s.theme.series_palette[0]
}

fn area_fill(c: &mut dyn Canvas, s: &Scene<'_>, pts: &[Pos2]) {
    let color = line_color(s);
    let alpha = if s.theme.dark { 0.28 } else { 0.22 };
    c.fill_area(
        pts,
        s.vp.plot.bottom,
        s.vp.plot.top,
        with_alpha(color, alpha),
        with_alpha(color, 0.0),
    );
}

fn line(c: &mut dyn Canvas, s: &Scene<'_>, area: bool) {
    let pts: Vec<Pos2> = s
        .bars
        .iter()
        .map(|b| pos2(b.x, s.vp.y_of(b.close)))
        .collect();
    if area {
        area_fill(c, s, &pts);
    }
    c.polyline(&pts, 1.5, line_color(s));
}

/// Zoomed far out: one aggregated column per pixel.
fn decimated(c: &mut dyn Canvas, s: &Scene<'_>, g: &BarGeom) {
    let vp = &s.vp;
    match s.chart_type {
        ChartType::Line | ChartType::Area => {
            let color = line_color(s);
            let pts: Vec<Pos2> = s.bars.iter().map(|b| pos2(b.x, vp.y_of(b.close))).collect();
            if s.chart_type == ChartType::Area {
                area_fill(c, s, &pts);
            }
            // Min/max envelope per column keeps spikes visible; then the close path.
            for b in s.bars {
                let col = g.column(
                    g.center_px(b.x),
                    1.0,
                    vp.y_of(b.close_max),
                    vp.y_of(b.close_min),
                );
                c.fill_rect(col, color);
            }
            c.polyline(&pts, 1.0, color);
        }
        _ => {
            for b in s.bars {
                let (color, _) = s.colors(b.close >= b.open);
                let col = g.column(g.center_px(b.x), 1.0, vp.y_of(b.high), vp.y_of(b.low));
                c.fill_rect(col, color);
            }
        }
    }
}

fn volume(c: &mut dyn Canvas, s: &Scene<'_>, rect: Rect) {
    let max = max_volume(s.bars);
    if max <= 0.0 {
        return;
    }
    let g = BarGeom::new(s.vp.bar_spacing, c.pixels_per_point());
    let width = if s.decimated { 1.0 } else { g.body_px };
    let h = rect.height() * 0.95;
    for b in s.bars {
        let color = if b.close >= b.open {
            s.theme.volume_up
        } else {
            s.theme.volume_down
        };
        let top = rect.bottom() - (b.volume / max) as f32 * h;
        c.fill_rect(g.column(g.center_px(b.x), width, top, rect.bottom()), color);
    }
}

/// One grid line: dotted (1 px on, 3 px off) or solid, per the style.
fn grid_line(c: &mut dyn Canvas, s: &Scene<'_>, a: Pos2, b: Pos2) {
    if s.style.dotted_grid {
        c.dotted_line(a, b, s.theme.grid);
    } else {
        c.line(a, b, 1.0, s.theme.grid);
    }
}

/// Grid/label positions on the price axis: (y, label).
fn price_ticks(s: &Scene<'_>) -> Vec<(f32, String)> {
    let vp = &s.vp;
    scale::price_ticks(vp, s.style.price_label_spacing, s.tick_size)
        .into_iter()
        .map(|p| (vp.y_of(p), axis_value_label(s, p)))
        .collect()
}

/// Time-axis ticks (pixel-snapped x, label, major flag).
fn time_ticks(c: &mut dyn Canvas, s: &Scene<'_>) -> Vec<time_axis::TimeTick> {
    let ppp = c.pixels_per_point();
    let vp = s.vp;
    // Examine at most one candidate bar per ~3 points.
    let stride = (3.0 / vp.bar_spacing).ceil().max(1.0) as usize;
    let size = s.style.font_size;
    // Gap chosen so regular labels land ~`time_label_spacing` apart.
    let typical = c.text_size("00:00", size).x;
    let gap = (s.style.time_label_spacing - typical).max(16.0);
    time_axis::time_ticks(
        s.display.open_times(),
        vp.visible_range(),
        (s.plot.left(), s.plot.right()),
        stride,
        gap,
        |i| crisp(vp.x_of(i as f32), ppp),
        |text| c.text_size_styled(text, size, true).x,
    )
}

/// A label box on an axis. Price-axis tags span the whole axis width.
struct Tag {
    rect: Rect,
    lines: Vec<String>,
    bg: Color32,
    /// Text aligned with the price labels (price axis) vs. centered (time axis).
    on_price_axis: bool,
}

fn draw_tag(c: &mut dyn Canvas, s: &Scene<'_>, tag: &Tag) {
    c.fill_rounded_rect(tag.rect, 2.0, tag.bg);
    let line_h = s.style.font_size + 3.0;
    let top = tag.rect.center().y - line_h * tag.lines.len() as f32 / 2.0;
    for (i, text) in tag.lines.iter().enumerate() {
        let y = top + line_h * (i as f32 + 0.5);
        let (pos, align) = if tag.on_price_axis {
            // Left-aligned with the axis labels, like TradingView.
            (pos2(s.price_axis.left() + 8.0, y), Align2::LEFT_CENTER)
        } else {
            (pos2(tag.rect.center().x, y), Align2::CENTER_CENTER)
        };
        c.text(pos, align, text, s.style.font_size, s.theme.text_on_accent);
    }
}

/// A tag spanning the price axis, centered on `y`.
fn price_axis_tag(s: &Scene<'_>, y: f32, lines: Vec<String>, bg: Color32) -> Tag {
    let line_h = s.style.font_size + 3.0;
    let h = line_h * lines.len() as f32 + 6.0;
    let rect = Rect::from_min_max(
        pos2(s.price_axis.left() + 1.0, y - h / 2.0),
        pos2(s.price_axis.right() - 1.0, y + h / 2.0),
    );
    Tag {
        rect,
        lines,
        bg,
        on_price_axis: true,
    }
}

fn price_labels(c: &mut dyn Canvas, s: &Scene<'_>, ticks: &[(f32, String)], tags: &[Tag]) {
    let x = s.price_axis.left() + 8.0;
    let half = s.style.font_size * 0.7;
    for (y, label) in ticks {
        if *y < s.plot.top() + half || *y > s.vp.plot.bottom - half {
            continue;
        }
        // Hidden under the last-price / crosshair tags, as on TradingView.
        if tags
            .iter()
            .any(|t| *y + half > t.rect.top() && *y - half < t.rect.bottom())
        {
            continue;
        }
        c.text(
            pos2(x, *y),
            Align2::LEFT_CENTER,
            label,
            s.style.font_size,
            s.theme.text_muted,
        );
    }
}

fn time_labels(
    c: &mut dyn Canvas,
    s: &Scene<'_>,
    ticks: &[time_axis::TimeTick],
    tag: Option<&Tag>,
) {
    let y = s.time_axis.center().y;
    let size = s.style.font_size;
    for t in ticks {
        let w = c.text_size_styled(&t.label, size, t.major).x;
        let (l, r) = (t.x - w / 2.0, t.x + w / 2.0);
        if let Some(tag) = tag
            && r > tag.rect.left() - 4.0
            && l < tag.rect.right() + 4.0
        {
            continue;
        }
        c.text_styled(
            pos2(t.x, y),
            Align2::CENTER_CENTER,
            &t.label,
            size,
            s.theme.text_muted,
            t.major,
        );
    }
}

/// Label as shown on the price axis for a price (percent in percent mode).
fn axis_value_label(s: &Scene<'_>, price: f64) -> String {
    match s.vp.scale.mode {
        ScaleMode::Percent => format_percent(s.vp.scale.to_scaled(price)),
        _ => format_price(price, s.precision),
    }
}

/// Dotted last-price line across the plot in the last candle's color.
fn last_price_line(c: &mut dyn Canvas, s: &Scene<'_>) {
    let Some(last) = s.last_raw else { return };
    let (color, _) = s.colors(last.is_bullish());
    let ppp = c.pixels_per_point();
    let y = crisp(s.vp.y_of(last.close), ppp);
    if y < s.plot.top() || y > s.vp.plot.bottom {
        return;
    }
    let px = 1.0 / ppp;
    c.dashed_line(
        pos2(s.plot.left(), y),
        pos2(s.plot.right(), y),
        px,
        color,
        px,
        2.0 * px,
    );
}

/// Last-price tag: price plus the time left until the bar closes.
fn last_price_tag(c: &mut dyn Canvas, s: &Scene<'_>) -> Option<Tag> {
    let last = s.last_raw?;
    let (color, _) = s.colors(last.is_bullish());
    let y = crisp(s.vp.y_of(last.close), c.pixels_per_point());
    if y < s.plot.top() || y > s.vp.plot.bottom {
        return None;
    }
    let mut lines = vec![axis_value_label(s, last.close)];
    if let (Some(now), Some(interval)) = (s.now_ms, s.countdown_interval_ms) {
        let remaining = last.open_time_ms + interval - now;
        if remaining > 0 && remaining <= interval {
            lines.push(format_countdown(remaining));
        }
    }
    Some(price_axis_tag(s, y, lines, color))
}

/// Draws crosshair lines; returns its price-axis tag and time-axis tag.
fn crosshair(c: &mut dyn Canvas, s: &Scene<'_>, p: Pos2, bar: Option<usize>) -> (Tag, Option<Tag>) {
    let ppp = c.pixels_per_point();
    let col = s.theme.crosshair;
    // Snap the vertical line to the hovered bar's center.
    let x = crisp(bar.map_or(p.x, |i| s.vp.x_of(i as f32)), ppp);
    let y = crisp(p.y, ppp);
    c.set_clip(s.plot);
    c.dashed_line(
        pos2(x, s.plot.top()),
        pos2(x, s.plot.bottom()),
        1.0,
        col,
        4.0,
        4.0,
    );
    c.dashed_line(
        pos2(s.plot.left(), y),
        pos2(s.plot.right(), y),
        1.0,
        col,
        4.0,
        4.0,
    );
    c.set_clip(s.full);

    let label = match s.volume_rect {
        // Inside the volume pane the axis shows volume, not price.
        Some(vr) if vr.contains(p) => {
            let max = max_volume(s.bars);
            let v = (vr.bottom() - p.y) as f64 / (vr.height() * 0.95) as f64 * max;
            format_volume(v.max(0.0))
        }
        _ => axis_value_label(s, s.vp.price_at(p.y)),
    };
    let price_tag = price_axis_tag(s, y, vec![label], s.theme.crosshair_label_bg);

    let time_tag = bar.map(|i| {
        let text = format_full_time(s.display.open_times()[i], s.bar_interval_ms);
        let size = c.text_size(&text, s.style.font_size) + vec2(16.0, 8.0);
        let mut rect = Align2::CENTER_CENTER.anchor_size(pos2(x, s.time_axis.center().y), size);
        // Keep it inside the time axis.
        let dx = (s.plot.left() - rect.left()).max(0.0) + (s.plot.right() - rect.right()).min(0.0);
        rect = rect.translate(vec2(dx, 0.0));
        Tag {
            rect,
            lines: vec![text],
            bg: s.theme.crosshair_label_bg,
            on_price_axis: false,
        }
    });
    (price_tag, time_tag)
}

/// Top-left readout: title, OHLC, change vs previous close and volume of the
/// hovered (or last) bar.
fn readout(c: &mut dyn Canvas, s: &Scene<'_>, bar: Option<usize>) {
    let t = s.theme;
    let size = s.style.font_size;
    let mut pos = s.plot.left_top() + vec2(10.0, 8.0);
    let mut put = |c: &mut dyn Canvas, text: &str, color: Color32, gap: f32| {
        let r = c.text(pos, Align2::LEFT_TOP, text, size, color);
        pos.x = r.right() + gap;
    };

    if !s.title.is_empty() {
        put(c, s.title, t.text, 14.0);
    }
    let Some(i) = bar else { return };
    let Some(k) = s.display.get(i) else { return };
    let prev_close = i
        .checked_sub(1)
        .and_then(|j| s.display.get(j))
        .map_or(k.open, |p| p.close);
    let change = k.close - prev_close;
    let (color, _) = s.colors(change >= 0.0);
    let d = s.precision;
    for (name, v) in [("O", k.open), ("H", k.high), ("L", k.low), ("C", k.close)] {
        put(c, name, t.text_muted, 3.0);
        put(c, &format_price(v, d), color, 10.0);
    }
    let pct = if prev_close != 0.0 {
        change / prev_close * 100.0
    } else {
        0.0
    };
    put(
        c,
        &format!("{} ({pct:+.2}%)", format_signed_price(change, d)),
        color,
        14.0,
    );
    put(c, "Vol", t.text_muted, 3.0);
    put(c, &format_volume(k.volume), t.text, 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::{PlotRect, PriceScale};
    use egui::Vec2;

    /// Records drawn text; geometry primitives are ignored.
    #[derive(Default)]
    struct Recorder {
        texts: Vec<String>,
    }

    impl Canvas for Recorder {
        fn pixels_per_point(&self) -> f32 {
            1.0
        }
        fn set_clip(&mut self, _: Rect) {}
        fn fill_rect(&mut self, _: Rect, _: Color32) {}
        fn fill_rounded_rect(&mut self, _: Rect, _: f32, _: Color32) {}
        fn stroke_rect(&mut self, _: Rect, _: f32, _: Color32) {}
        fn line(&mut self, _: Pos2, _: Pos2, _: f32, _: Color32) {}
        fn dashed_line(&mut self, _: Pos2, _: Pos2, _: f32, _: Color32, _: f32, _: f32) {}
        fn polyline(&mut self, _: &[Pos2], _: f32, _: Color32) {}
        fn fill_area(&mut self, _: &[Pos2], _: f32, _: f32, _: Color32, _: Color32) {}
        fn text_styled(
            &mut self,
            pos: Pos2,
            anchor: Align2,
            text: &str,
            size: f32,
            _: Color32,
            bold: bool,
        ) -> Rect {
            self.texts.push(text.to_owned());
            anchor.anchor_size(pos, self.text_size_styled(text, size, bold))
        }
        fn text_size_styled(&mut self, text: &str, size: f32, _: bool) -> Vec2 {
            vec2(text.len() as f32 * size * 0.6, size)
        }
    }

    fn scene_with<'a>(
        series: &'a CandleSeries,
        theme: &'a Theme,
        style: &'a ChartStyle,
        now_ms: Option<i64>,
    ) -> Scene<'a> {
        let plot = Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 500.0));
        let vp = Viewport {
            bar_spacing: 10.0,
            right_offset_bars: 2.0,
            price_min: 90.0,
            price_max: 110.0,
            plot: PlotRect {
                left: 0.0,
                top: 0.0,
                right: 800.0,
                bottom: 500.0,
            },
            bar_count: series.len(),
            scale: PriceScale::linear(),
        };
        Scene {
            vp,
            theme,
            style,
            chart_type: ChartType::Candles,
            full: Rect::from_min_max(pos2(0.0, 0.0), pos2(880.0, 528.0)),
            plot,
            volume_rect: None,
            price_axis: Rect::from_min_max(pos2(800.0, 0.0), pos2(880.0, 500.0)),
            time_axis: Rect::from_min_max(pos2(0.0, 500.0), pos2(800.0, 528.0)),
            display: series,
            last_raw: series.last(),
            bars: &[],
            decimated: false,
            crosshair: None,
            precision: 2,
            tick_size: Some(0.01),
            bar_interval_ms: 300_000,
            now_ms,
            countdown_interval_ms: Some(300_000),
            title: "",
        }
    }

    fn series() -> CandleSeries {
        let mut s = CandleSeries::new();
        for i in 0..20 {
            s.upsert(Candle {
                open_time_ms: i * 300_000,
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.5,
                volume: 1.0,
            });
        }
        s
    }

    #[test]
    fn last_price_tag_shows_countdown_while_bar_is_open() {
        let (s, theme, style) = (series(), Theme::dark(), ChartStyle::default());
        let last_open = 19 * 300_000;
        // 2m16s into the 5m bar → 02:44 left.
        let scene = scene_with(&s, &theme, &style, Some(last_open + 136_000));
        let mut c = Recorder::default();
        let tag = last_price_tag(&mut c, &scene).expect("tag");
        assert_eq!(tag.lines, ["100.50", "02:44"]);

        // No clock (synthetic/historical data) → price only.
        let scene = scene_with(&s, &theme, &style, None);
        assert_eq!(last_price_tag(&mut c, &scene).unwrap().lines.len(), 1);
    }

    #[test]
    fn labels_under_tags_are_hidden_and_prices_have_separators() {
        let (s, theme, style) = (series(), Theme::dark(), ChartStyle::default());
        let scene = scene_with(&s, &theme, &style, None);
        let mut c = Recorder::default();
        draw(&mut c, &scene);
        // The 100.00 grid label sits under the 100.50 last-price tag, so it is
        // hidden; it appears only once, as the readout's open value.
        assert_eq!(
            c.texts.iter().filter(|t| *t == "100.00").count(),
            1,
            "{:?}",
            c.texts
        );
        assert!(c.texts.iter().any(|t| t == "98.00"), "{:?}", c.texts);
        assert!(c.texts.iter().any(|t| t == "102.00"), "{:?}", c.texts);
    }
}
