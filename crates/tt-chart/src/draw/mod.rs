//! Chart scene drawing. Uses only the [`Canvas`] trait, never a UI toolkit directly.

use crate::bars::{DrawBar, max_volume};
use crate::indicators::Indicator;
use crate::render::Canvas;
use crate::scale::{
    self, TimeFormat, format_countdown, format_full_time_with, format_percent, format_price,
};
use crate::style::{
    BackgroundKind, HoursFormat, LastValueMode, LineKind, LineLabel, Visibility3, resolve,
};
use crate::time_axis;
use crate::viewport::Viewport;
use crate::{ChartStyle, ChartType, ScaleMode};
use ecolor::Color32;
use emath::{Align2, Pos2, Rect, pos2, vec2};
use std::ops::Range;
use tt_core::{Candle, CandleSeries, Placement};
use tt_theme::Theme;

mod legend;
mod series;
mod study;

use legend::{legends, readout};
use series::series;
use study::{study_plots, study_tags, volume};

/// Everything needed to draw one frame.
pub(crate) struct Scene<'a> {
    /// Price viewport; `vp.plot` is the main (price) pane.
    pub vp: Viewport,
    pub theme: &'a Theme,
    pub style: &'a ChartStyle,
    pub chart_type: ChartType,
    /// Whole widget.
    pub full: Rect,
    /// Plot area of all panes (left of the price axis, above the time axis).
    pub plot: Rect,
    /// Bottom of the price pane used by Volume overlays.
    pub volume_rect: Option<Rect>,
    pub indicators: &'a [Indicator],
    /// Indicator panes below the price pane.
    pub panes: Vec<Pane>,
    /// Indices of the visible bars (`display` indices).
    pub range: Range<usize>,
    pub legend_collapsed: bool,
    /// The price pane is collapsed to a strip (another pane is maximized or
    /// it was collapsed): only the readout is drawn.
    pub main_collapsed: bool,
    /// Legend row under the pointer (indicator id; 0 = collapse button).
    pub legend_hover: Option<u64>,
    /// Legend button under the pointer.
    pub legend_hover_button: Option<(u64, LegendButton)>,
    pub price_axis: Rect,
    pub time_axis: Rect,
    /// Series being displayed (raw or Heikin Ashi).
    pub display: &'a CandleSeries,
    /// Renko / Line break: index (in `display`) of the first projection
    /// (forming) brick.
    pub projection_from: Option<usize>,
    /// Last *real* candle, for the last-price line.
    pub last_raw: Option<Candle>,
    /// The raw series (previous day close).
    pub raw: &'a CandleSeries,
    /// Ticker, currency and description (name labels, watermark).
    pub labels: &'a crate::view::SymbolLabels,
    /// Date and hours format of time labels.
    pub time_fmt: TimeFormat,
    pub bars: &'a [DrawBar],
    /// `true` when bars are per-pixel-column aggregates.
    pub decimated: bool,
    pub crosshair: Option<Pos2>,
    pub precision: usize,
    pub tick_size: Option<f64>,
    pub bar_interval_ms: i64,
    /// Current time, for the bar-close countdown (None = no countdown).
    pub now_ms: Option<i64>,
    /// Exact close time of the last bar, for the countdown.
    pub bar_close_ms: Option<i64>,
    pub title: &'a str,
    pub drawings: &'a crate::drawings::Drawings,
    /// Zone of the time labels.
    pub tz: &'a crate::TzOffsets,
}

/// An indicator pane: the indicator (index into `Scene::indicators`) and
/// its viewport (same x mapping as the price pane, own value range).
#[derive(Debug, Clone)]
pub(crate) struct Pane {
    pub index: usize,
    pub vp: Viewport,
    /// Collapsed to a strip: only the legend is drawn.
    pub collapsed: bool,
}

impl Pane {
    pub fn rect(&self) -> Rect {
        vp_rect(&self.vp)
    }
}

fn vp_rect(vp: &Viewport) -> Rect {
    Rect::from_min_max(
        pos2(vp.plot.left, vp.plot.top),
        pos2(vp.plot.right, vp.plot.bottom),
    )
}

/// Clickable parts of the legend, laid out by [`draw`] and hit-tested by the
/// view on the next frame.
/// A clickable button in a legend row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendButton {
    ToggleVisible,
    Settings,
    Remove,
    /// Collapse / expand the price pane's legend.
    Collapse,
}

#[derive(Debug, Clone)]
pub(crate) struct LegendHit {
    /// Indicator id (0 for the collapse button).
    pub id: u64,
    pub row: Rect,
    pub buttons: Vec<(LegendButton, Rect)>,
}

/// Draws the chart; returns the legend's clickable areas.
/// Draws the whole chart (base, then overlay); returns the legend's
/// clickable areas. (Views call the two layers directly.)
#[cfg(test)]
pub(crate) fn draw(c: &mut dyn Canvas, s: &Scene<'_>) -> Vec<LegendHit> {
    draw_base(c, s);
    draw_overlay(c, s)
}

/// The static layer: everything that does not depend on the pointer (grid,
/// bars, indicators, axes, value tags). Backends can cache it and redraw only
/// the overlay while the mouse moves, like TradingView's paired canvases.
pub(crate) fn draw_base(c: &mut dyn Canvas, s: &Scene<'_>) {
    let t = s.theme;
    let cv = &s.style.canvas;
    c.set_clip(s.full);
    match cv.background_kind {
        BackgroundKind::Solid => c.fill_rect(s.full, t.bg),
        BackgroundKind::Gradient => {
            c.fill_rect_gradient(s.full, t.bg, resolve(cv.background_bottom, t.bg))
        }
    }

    if s.display.is_empty() {
        c.text(
            s.plot.center(),
            Align2::CENTER_CENTER,
            "Waiting for data…",
            s.style.scales.text_size + 1.0,
            t.text_muted,
        );
        return;
    }

    let price_ticks = price_ticks(s);
    let time_ticks = time_ticks(c, s);
    // Previous day close, visible high / low and average close: (value,
    // style, axis label prefix). Computed once per frame, only when on.
    let marks = price_marks(s);

    let pane_ticks: Vec<Vec<(f32, String)>> = s.panes.iter().map(|p| pane_ticks(s, p)).collect();

    if cv.watermark.visible && !s.main_collapsed {
        watermark(c, s);
    }

    // Grid: one line per price label and per time label, like TradingView.
    let ppp = c.pixels_per_point();
    let grid_h = (s.style.grid_color(t, false), cv.grid_h_kind);
    let grid_v = (s.style.grid_color(t, true), cv.grid_v_kind);
    if cv.grid_lines.horizontal() {
        c.set_clip(s.vp_rect());
        for (y, _) in price_ticks.iter().filter(|_| !s.main_collapsed) {
            let y = crisp(*y, ppp);
            grid_line(c, grid_h, pos2(s.plot.left(), y), pos2(s.plot.right(), y));
        }
        for (pane, ticks) in s.panes.iter().zip(&pane_ticks) {
            if pane.collapsed {
                continue;
            }
            c.set_clip(pane.rect());
            for (y, _) in ticks {
                let y = crisp(*y, ppp);
                grid_line(c, grid_h, pos2(s.plot.left(), y), pos2(s.plot.right(), y));
            }
        }
    }
    c.set_clip(s.plot);
    if cv.grid_lines.vertical() {
        for tick in &time_ticks {
            grid_line(
                c,
                grid_v,
                pos2(tick.x, s.plot.top()),
                pos2(tick.x, s.plot.bottom()),
            );
        }
    }
    session_breaks(c, s);

    if let Some(vr) = s.volume_rect.filter(|_| !s.main_collapsed) {
        c.set_clip(vr);
        for ind in s.indicators.iter().filter(|i| i.shown()) {
            if ind.placement() == Placement::VolumeOverlay {
                volume(c, s, vr, ind);
            }
        }
    }
    if !s.main_collapsed {
        c.set_clip(s.vp_rect());
        series(c, s);
        for ind in s.indicators.iter().filter(|i| i.shown()) {
            if ind.placement() == Placement::Overlay {
                study_plots(c, s, &s.vp, ind);
            }
        }
        last_price_line(c, s);
        for (value, ll, _, color) in &marks {
            mark_line(c, s, *value, ll, *color);
        }
        crate::drawings::draw_base(c, s);
    }
    for pane in &s.panes {
        c.set_clip(pane.rect());
        let ind = &s.indicators[pane.index];
        if ind.shown() && !pane.collapsed {
            study_plots(c, s, &pane.vp, ind);
        }
    }
    // Pane dividers across plot and price axis.
    c.set_clip(s.full);
    let separator = resolve(cv.separators, t.border);
    for pane in &s.panes {
        let y = crisp(pane.rect().top(), ppp) - 0.5 / ppp;
        c.line(
            pos2(s.full.left(), y),
            pos2(s.full.right(), y),
            1.0 / ppp,
            separator,
        );
    }
    // Scale lines (Canvas › Scales › Lines): only when picked, so the
    // default look (no axis borders) stays.
    if let Some(lines) = cv.scale_lines {
        let col = lines.to_color32();
        let x = if s.price_axis.left() >= s.plot.right() {
            s.price_axis.left()
        } else {
            s.price_axis.right()
        };
        let x = crisp(x, ppp) - 0.5 / ppp;
        c.line(
            pos2(x, s.full.top()),
            pos2(x, s.time_axis.top()),
            1.0 / ppp,
            col,
        );
        let y = crisp(s.time_axis.top(), ppp) - 0.5 / ppp;
        c.line(
            pos2(s.full.left(), y),
            pos2(s.full.right(), y),
            1.0 / ppp,
            col,
        );
    }

    // Axis tags are computed before the tick labels so labels underneath can
    // be hidden. (The crosshair's tags are opaque and drawn by the overlay.)
    c.set_clip(s.full);
    // Axis tags per pane (0 = price pane), spread so they don't overlap.
    let ind_mode = s.style.scales.indicators;
    let mut grouped = if ind_mode.shows_value() || ind_mode.shows_name() {
        study_tags(s)
    } else {
        Vec::new()
    };
    if let Some(tag) = last_price_tag(c, s) {
        grouped.push((0, tag));
    }
    for (value, ll, label, color) in &marks {
        if ll.label && !s.main_collapsed {
            let y = crisp(s.vp.y_of(*value), ppp);
            if y >= s.plot.top() && y <= s.vp.plot.bottom {
                let mut lines = vec![axis_value_label(s, *value)];
                if let Some(l) = label {
                    lines[0] = format!("{l} {}", lines[0]);
                }
                grouped.push((0, price_axis_tag(s, y, lines, *color)));
            }
        }
    }
    // Collapsed panes show no tags.
    grouped.retain(|(pane, _)| match pane {
        0 => !s.main_collapsed,
        k => !s.panes[k - 1].collapsed,
    });
    let mut tags: Vec<Tag> = Vec::new();
    for pane in 0..=s.panes.len() {
        let (top, bottom) = if pane == 0 {
            (s.plot.top(), s.vp.plot.bottom)
        } else {
            let r = s.panes[pane - 1].rect();
            (r.top(), r.bottom())
        };
        let mut group: Vec<Tag> = grouped
            .iter()
            .filter(|(p, _)| *p == pane)
            .map(|(_, t)| t.clone())
            .collect();
        if s.style.scales.no_overlapping_labels {
            spread_tags(&mut group, top, bottom);
        }
        tags.extend(group);
    }
    if !s.main_collapsed {
        price_labels(c, s, &price_ticks, &tags, (s.plot.top(), s.vp.plot.bottom));
    }
    for (pane, ticks) in s.panes.iter().zip(&pane_ticks) {
        if pane.collapsed {
            continue;
        }
        let r = pane.rect();
        price_labels(c, s, ticks, &tags, (r.top(), r.bottom()));
    }
    time_labels(c, s, &time_ticks, None);
    if s.style.scales.currency_label == Visibility3::Always && !s.main_collapsed {
        currency_label(c, s);
    }
    for tag in &tags {
        draw_tag(c, s, tag);
    }
    for tag in crate::drawings::axis_tags(s) {
        draw_tag(c, s, &tag);
    }
}

/// A price mark: price, style, axis label prefix and color.
type Mark = (f64, LineLabel, Option<&'static str>, Color32);

/// Previous day close, visible high / low and average close lines and
/// labels that are switched on: (price, style, label prefix).
fn price_marks(s: &Scene<'_>) -> Vec<Mark> {
    let sc = &s.style.scales;
    let muted = s.theme.text_muted;
    let on = |l: &LineLabel| l.label || l.line;
    let mut out = Vec::new();
    if on(&sc.prev_close)
        && let Some(v) = prev_day_close(s.raw.open_times(), s.raw.closes(), s.bar_interval_ms)
    {
        out.push((
            v,
            sc.prev_close,
            None,
            resolve(sc.prev_close.color, PREV_CLOSE_COLOR),
        ));
    }
    if on(&sc.high_low)
        && let Some((lo, hi)) = visible_high_low(s.bars)
    {
        let col = resolve(sc.high_low.color, muted);
        out.push((hi, sc.high_low, Some("H"), col));
        out.push((lo, sc.high_low, Some("L"), col));
    }
    if on(&sc.avg_close)
        && let Some(v) = average(s.display.closes().get(s.range.clone()).unwrap_or(&[]))
    {
        out.push((v, sc.avg_close, None, resolve(sc.avg_close.color, muted)));
    }
    out
}

/// TradingView's previous-close line color.
const PREV_CLOSE_COLOR: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);

/// A dashed horizontal price line across the price pane.
fn mark_line(c: &mut dyn Canvas, s: &Scene<'_>, value: f64, ll: &LineLabel, color: Color32) {
    if !ll.line {
        return;
    }
    let ppp = c.pixels_per_point();
    let y = crisp(s.vp.y_of(value), ppp);
    if y < s.plot.top() || y > s.vp.plot.bottom {
        return;
    }
    stroke_kind(
        c,
        pos2(s.plot.left(), y),
        pos2(s.plot.right(), y),
        ll.width.max(1.0 / ppp),
        color,
        LineKind::Dashed,
    );
}

/// Close of the previous (UTC) day: the last bar before the last bar's
/// day began; on daily and longer bars, the previous bar's close.
pub(crate) fn prev_day_close(times: &[i64], closes: &[f64], interval_ms: i64) -> Option<f64> {
    let last = *times.last()?;
    let k = if interval_ms >= scale::DAY_MS {
        times.len().checked_sub(1)?
    } else {
        let day_start = last.div_euclid(scale::DAY_MS) * scale::DAY_MS;
        times.partition_point(|t| *t < day_start)
    };
    closes.get(k.checked_sub(1)?).copied()
}

/// Highest high and lowest low of the drawn bars (or columns).
pub(crate) fn visible_high_low(bars: &[DrawBar]) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for b in bars {
        if b.low.is_finite() && b.high.is_finite() {
            lo = lo.min(b.low);
            hi = hi.max(b.high);
        }
    }
    (lo <= hi).then_some((lo, hi))
}

/// Mean of the finite values.
pub(crate) fn average(values: &[f64]) -> Option<f64> {
    let (mut sum, mut n) = (0.0, 0usize);
    for v in values.iter().filter(|v| v.is_finite()) {
        sum += v;
        n += 1;
    }
    (n > 0).then(|| sum / n as f64)
}

/// A straight line in a settings line kind (dash lengths follow the width).
fn stroke_kind(c: &mut dyn Canvas, a: Pos2, b: Pos2, width: f32, color: Color32, kind: LineKind) {
    let w = width.max(0.5);
    match kind {
        LineKind::Solid => c.line(a, b, width, color),
        LineKind::Dashed => c.dashed_line(a, b, width, color, 4.0 * w, 4.0 * w),
        LineKind::Dotted => c.dashed_line(a, b, width, color, w, 2.0 * w),
    }
}

/// The watermark: ticker and interval large in the middle of the price
/// pane, the description below.
fn watermark(c: &mut dyn Canvas, s: &Scene<'_>) {
    let wm = &s.style.canvas.watermark;
    let color = resolve(wm.color, tt_theme::with_alpha(s.theme.text, 0.06));
    let r = s.vp_rect();
    let big = (r.height() / 6.0).clamp(18.0, 72.0);
    let small = (big * 0.4).max(12.0);
    let first = wm.ticker_interval && !s.labels.ticker_interval.is_empty();
    let second = wm.description && !s.labels.description.is_empty();
    let h = if first { big } else { 0.0 } + if second { small + 4.0 } else { 0.0 };
    let mut y = r.center().y - h / 2.0;
    c.set_clip(r);
    if first {
        c.text_styled(
            pos2(r.center().x, y + big / 2.0),
            Align2::CENTER_CENTER,
            &s.labels.ticker_interval,
            big,
            color,
            true,
        );
        y += big + 4.0;
    }
    if second {
        c.text(
            pos2(r.center().x, y + small / 2.0),
            Align2::CENTER_CENTER,
            &s.labels.description,
            small,
            color,
        );
    }
    c.set_clip(s.full);
}

/// The currency and unit label ("USDT") at the top of the price axis.
fn currency_label(c: &mut dyn Canvas, s: &Scene<'_>) {
    let text = s.labels.currency.as_str();
    if text.is_empty() {
        return;
    }
    let size = s.style.scales.text_size;
    let h = size + 6.0;
    let rect = Rect::from_min_max(
        pos2(s.price_axis.left() + 1.0, s.plot.top() + 2.0),
        pos2(s.price_axis.right() - 1.0, s.plot.top() + 2.0 + h),
    );
    c.fill_rect(rect, s.theme.bg);
    c.text(
        pos2(s.price_axis.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        size,
        s.theme.text_muted,
    );
}

/// The pointer layer, drawn over [`draw_base`]: crosshair lines and their
/// axis tags, the OHLC readout and the legends (values follow the hovered
/// bar). Returns the legend's clickable areas.
pub(crate) fn draw_overlay(c: &mut dyn Canvas, s: &Scene<'_>) -> Vec<LegendHit> {
    c.set_clip(s.full);
    if s.display.is_empty() {
        readout(c, s, None);
        return Vec::new();
    }
    // The crosshair snaps to any slot (also past the last bar); the readout
    // shows the bar under it, or the last bar over empty space.
    crate::drawings::draw_overlay(c, s);
    c.set_clip(s.full);
    let hovered_slot = s.crosshair.map(|p| s.vp.slot_at(p.x));
    let hovered = s.crosshair.and_then(|p| s.vp.bar_at(p.x));
    if s.crosshair.is_some()
        && s.style.scales.currency_label == Visibility3::OnHover
        && !s.main_collapsed
    {
        currency_label(c, s);
    }
    if let Some(p) = s.crosshair {
        let (price_tag, time_tag) = crosshair(c, s, p, hovered_slot);
        draw_tag(c, s, &price_tag);
        if let Some(t) = &time_tag {
            draw_tag(c, s, t);
        }
    }
    let bar = hovered.or(s.display.len().checked_sub(1));
    readout(c, s, bar);
    legends(c, s, bar)
}

impl Scene<'_> {
    pub(crate) fn vp_rect(&self) -> Rect {
        vp_rect(&self.vp)
    }

    /// Whether a bar counts as rising; `by_prev_close` is the chart type's
    /// "Color bars based on previous close" (read once per frame).
    fn rising(by_prev_close: bool, b: &DrawBar) -> bool {
        if by_prev_close {
            b.close >= b.prev_close
        } else {
            b.close >= b.open
        }
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

/// Volume scale of the overlay: the busiest visible bar reaches the top.
fn volume_vp(s: &Scene<'_>, rect: Rect) -> Viewport {
    let max = max_volume(s.bars).max(f64::MIN_POSITIVE);
    Viewport {
        price_min: 0.0,
        price_max: max,
        plot: crate::viewport::PlotRect {
            left: rect.left(),
            top: rect.top(),
            right: rect.right(),
            bottom: rect.bottom(),
        },
        scale: crate::viewport::PriceScale::linear(),
        ..s.vp
    }
}

/// Price-axis labels of an indicator pane.
fn pane_ticks(s: &Scene<'_>, pane: &Pane) -> Vec<(f32, String)> {
    let ind = &s.indicators[pane.index];
    scale::price_ticks(&pane.vp, s.style.price_label_spacing, None)
        .into_iter()
        .map(|v| (pane.vp.y_of(v), ind.format(v, s.precision)))
        .collect()
}

/// One grid line: solid, dashed or dotted (1 px on, 3 px off), per the style.
fn grid_line(c: &mut dyn Canvas, (color, kind): (Color32, LineKind), a: Pos2, b: Pos2) {
    match kind {
        LineKind::Solid => c.line(a, b, 1.0, color),
        LineKind::Dashed => c.dashed_line(a, b, 1.0, color, 4.0, 3.0),
        LineKind::Dotted => c.dotted_line(a, b, color),
    }
}

/// Events › Session breaks: a line where each day starts in the chart's
/// time zone, on intraday charts only.
fn session_breaks(c: &mut dyn Canvas, s: &Scene<'_>) {
    let sb = &s.style.events.session_breaks;
    if !sb.visible || s.bar_interval_ms >= scale::DAY_MS {
        return;
    }
    let ppp = c.pixels_per_point();
    let vp = s.vp;
    let color = s
        .style
        .resolve(sb.color, Color32::from_rgb(0x49, 0x85, 0xe7));
    let w = sb.width.max(0.5);
    let stride = (3.0 / vp.bar_spacing).ceil().max(1.0) as usize;
    let slots = time_axis::SlotTimes::new(s.display.open_times(), s.bar_interval_ms).with_tz(s.tz);
    for i in time_axis::day_boundaries(&slots, vp.visible_slots(), stride) {
        // Between the last bar of the old day and the first of the new.
        let x = crisp(vp.x_of(i as f32 - 0.5), ppp);
        let (a, b) = (pos2(x, s.plot.top()), pos2(x, s.plot.bottom()));
        match sb.kind {
            LineKind::Solid => c.line(a, b, w, color),
            LineKind::Dashed => c.dashed_line(a, b, w, color, 6.0 * w, 4.0 * w),
            LineKind::Dotted => c.dashed_line(a, b, w, color, w, 2.0 * w),
        }
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
    let size = s.style.scales.text_size;
    // Gap chosen so regular labels land ~`time_label_spacing` apart.
    let typical = c.text_size("00:00", size).x;
    let gap = (s.style.time_label_spacing - typical).max(16.0);
    let slots = time_axis::SlotTimes::new(s.display.open_times(), s.bar_interval_ms).with_tz(s.tz);
    let hours = s.time_fmt.hours;
    // 12-hour clock: wider labels, so ticks are placed with their width.
    let mut ticks = time_axis::time_ticks(
        &slots,
        vp.visible_slots(),
        (s.plot.left(), s.plot.right()),
        stride,
        gap,
        |i| crisp(vp.x_of(i as f32), ppp),
        |text| {
            let w = c.text_size_styled(text, size, true).x;
            if hours == HoursFormat::H12 && text.contains(':') {
                w * 1.6
            } else {
                w
            }
        },
    );
    if hours == HoursFormat::H12 {
        for t in &mut ticks {
            if t.weight < time_axis::TickWeight::Day
                && let Some(ms) = slots.local_at(t.index)
            {
                t.label = scale::format_tick_time(ms, hours);
            }
        }
    }
    ticks
}

/// Moves overlapping axis tags apart (keeping their order) within
/// `top..bottom`, like TradingView stacks colliding price-scale labels.
fn spread_tags(tags: &mut [Tag], top: f32, bottom: f32) {
    tags.sort_by(|a, b| a.rect.center().y.total_cmp(&b.rect.center().y));
    let mut min_top = top;
    for t in tags.iter_mut() {
        if t.rect.top() < min_top {
            t.rect = t.rect.translate(vec2(0.0, min_top - t.rect.top()));
        }
        min_top = t.rect.bottom() + 1.0;
    }
    let mut max_bottom = bottom;
    for t in tags.iter_mut().rev() {
        if t.rect.bottom() > max_bottom {
            t.rect = t.rect.translate(vec2(0.0, max_bottom - t.rect.bottom()));
        }
        max_bottom = t.rect.top() - 1.0;
    }
}

/// A label box on an axis. Price-axis tags span the whole axis width.
#[derive(Clone)]
pub(crate) struct Tag {
    rect: Rect,
    /// Value lines; empty for a name-only label.
    lines: Vec<String>,
    bg: Color32,
    /// Text aligned with the price labels (price axis) vs. centered (time axis).
    on_price_axis: bool,
    /// Name label ("BTCUSDT", "RSI") drawn in a box beside the price axis,
    /// on the plot side, at the tag's height.
    name: Option<&'static str>,
    /// `name` is the symbol's ticker (owned by the scene's labels).
    ticker: bool,
}

pub(crate) fn draw_tag(c: &mut dyn Canvas, s: &Scene<'_>, tag: &Tag) {
    let name = if tag.ticker {
        Some(s.labels.ticker.as_str()).filter(|t| !t.is_empty())
    } else {
        tag.name
    };
    if let Some(name) = name {
        let size = s.style.scales.text_size;
        let w = c.text_size(name, size).x + 12.0;
        let left_axis = s.price_axis.right() <= s.plot.left();
        let (l, r) = if left_axis {
            (s.price_axis.right() + 1.0, s.price_axis.right() + 1.0 + w)
        } else {
            (s.price_axis.left() - 1.0 - w, s.price_axis.left() - 1.0)
        };
        let rect = Rect::from_x_y_ranges(l..=r, tag.rect.y_range());
        c.fill_rounded_rect(rect, tt_theme::metrics::R_XS, tag.bg);
        c.text(
            rect.center(),
            Align2::CENTER_CENTER,
            name,
            size,
            s.theme.text_on_accent,
        );
    }
    if tag.lines.is_empty() {
        return;
    }
    c.fill_rounded_rect(tag.rect, tt_theme::metrics::R_XS, tag.bg);
    let line_h = s.style.scales.text_size + 3.0;
    let top = tag.rect.center().y - line_h * tag.lines.len() as f32 / 2.0;
    for (i, text) in tag.lines.iter().enumerate() {
        let y = top + line_h * (i as f32 + 0.5);
        let (pos, align) = if tag.on_price_axis {
            // Left-aligned with the axis labels, like TradingView.
            (pos2(s.price_axis.left() + 8.0, y), Align2::LEFT_CENTER)
        } else {
            (pos2(tag.rect.center().x, y), Align2::CENTER_CENTER)
        };
        c.text(
            pos,
            align,
            text,
            s.style.scales.text_size,
            s.theme.text_on_accent,
        );
    }
}

/// A tag spanning the price axis, centered on `y`.
pub(crate) fn price_axis_tag(s: &Scene<'_>, y: f32, lines: Vec<String>, bg: Color32) -> Tag {
    let line_h = s.style.scales.text_size + 3.0;
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
        name: None,
        ticker: false,
    }
}

fn price_labels(
    c: &mut dyn Canvas,
    s: &Scene<'_>,
    ticks: &[(f32, String)],
    tags: &[Tag],
    (top, bottom): (f32, f32),
) {
    let x = s.price_axis.left() + 8.0;
    let half = s.style.scales.text_size * 0.7;
    for (y, label) in ticks {
        if *y < top + half || *y > bottom - half {
            continue;
        }
        // Hidden under the last-price / crosshair tags, as on TradingView.
        if tags
            .iter()
            .any(|t| !t.lines.is_empty() && *y + half > t.rect.top() && *y - half < t.rect.bottom())
        {
            continue;
        }
        c.text(
            pos2(x, *y),
            Align2::LEFT_CENTER,
            label,
            s.style.scales.text_size,
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
    let size = s.style.scales.text_size;
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
        ScaleMode::IndexedTo100 => format!("{:.2}", s.vp.scale.to_scaled(price)),
        _ => format_price(price, s.precision),
    }
}

/// Dotted last-price line across the plot: the picked color and width, or
/// the last candle's color at 1 px.
fn last_price_line(c: &mut dyn Canvas, s: &Scene<'_>) {
    let sc = &s.style.scales;
    if !sc.symbol_label.shows_line() {
        return;
    }
    let Some(last) = s.last_raw else { return };
    let (bar, _) = s.colors(last.is_bullish());
    let color = resolve(sc.price_line_color, bar);
    let ppp = c.pixels_per_point();
    let y = crisp(s.vp.y_of(last.close), ppp);
    if y < s.plot.top() || y > s.vp.plot.bottom {
        return;
    }
    // Width 1 = one physical pixel (the default look).
    let w = sc.price_line_width.max(1.0) / ppp;
    c.dashed_line(
        pos2(s.plot.left(), y),
        pos2(s.plot.right(), y),
        w,
        color,
        w,
        2.0 * w,
    );
}

/// Last-price tag: price plus the time left until the bar closes.
fn last_price_tag(c: &mut dyn Canvas, s: &Scene<'_>) -> Option<Tag> {
    let sc = &s.style.scales;
    let (value, name) = (sc.symbol_label.shows_value(), sc.symbol_label.shows_name());
    if !value && !name {
        return None;
    }
    let last = s.last_raw?;
    let (color, _) = s.colors(last.is_bullish());
    let y = crisp(s.vp.y_of(last.close), c.pixels_per_point());
    if y < s.plot.top() || y > s.vp.plot.bottom {
        return None;
    }
    if !value {
        let mut tag = price_axis_tag(s, y, Vec::new(), color);
        tag.ticker = true;
        return Some(tag);
    }
    let mut lines = vec![axis_value_label(s, last.close)];
    if sc.symbol_value_mode == LastValueMode::PriceAndPercent
        && let Some(base) = prev_day_close(s.raw.open_times(), s.raw.closes(), s.bar_interval_ms)
        && base != 0.0
    {
        lines.push(format_percent((last.close / base - 1.0) * 100.0));
    }
    if let (true, Some(now), Some(close)) = (sc.countdown, s.now_ms, s.bar_close_ms) {
        // Hidden once the bar should have closed (e.g. market closed / stale).
        let remaining = close - now;
        if remaining > 0 && close - last.open_time_ms >= remaining {
            lines.push(format_countdown(remaining));
        }
    }
    let mut tag = price_axis_tag(s, y, lines, color);
    tag.ticker = name;
    Some(tag)
}

/// Draws crosshair lines; returns its price-axis tag and time-axis tag.
fn crosshair(c: &mut dyn Canvas, s: &Scene<'_>, p: Pos2, slot: Option<i64>) -> (Tag, Option<Tag>) {
    let ppp = c.pixels_per_point();
    let cv = &s.style.canvas;
    let (col, width, kind) = (s.theme.crosshair, cv.crosshair_width, cv.crosshair_kind);
    // Snap the vertical line to the hovered bar's center.
    let x = crisp(slot.map_or(p.x, |i| s.vp.x_of(i as f32)), ppp);
    let y = crisp(p.y, ppp);
    c.set_clip(s.plot);
    stroke_kind(
        c,
        pos2(x, s.plot.top()),
        pos2(x, s.plot.bottom()),
        width,
        col,
        kind,
    );
    stroke_kind(
        c,
        pos2(s.plot.left(), y),
        pos2(s.plot.right(), y),
        width,
        col,
        kind,
    );
    c.set_clip(s.full);

    // Inside an indicator pane the axis shows the indicator's value.
    let label = match s.panes.iter().find(|pane| pane.rect().contains(p)) {
        Some(pane) => s.indicators[pane.index].format(pane.vp.price_at(p.y), s.precision),
        None => axis_value_label(s, s.vp.price_at(p.y)),
    };
    let price_tag = price_axis_tag(s, y, vec![label], s.theme.crosshair_label_bg);

    let slots = time_axis::SlotTimes::new(s.display.open_times(), s.bar_interval_ms);
    let time_tag = slot.and_then(|i| slots.time_at(i)).map(|t| {
        let text = format_full_time_with(s.tz.local_ms(t), s.bar_interval_ms, s.time_fmt);
        let size = c.text_size(&text, s.style.scales.text_size) + vec2(16.0, 8.0);
        let mut rect = Align2::CENTER_CENTER.anchor_size(pos2(x, s.time_axis.center().y), size);
        // Keep it inside the time axis.
        let dx = (s.plot.left() - rect.left()).max(0.0) + (s.plot.right() - rect.right()).min(0.0);
        rect = rect.translate(vec2(dx, 0.0));
        Tag {
            rect,
            lines: vec![text],
            bg: s.theme.crosshair_label_bg,
            on_price_axis: false,
            name: None,
            ticker: false,
        }
    });
    (price_tag, time_tag)
}

#[cfg(test)]
static NO_DRAWINGS: crate::drawings::Drawings = crate::drawings::Drawings::empty();
#[cfg(test)]
pub(crate) static NO_LABELS: crate::view::SymbolLabels = crate::view::SymbolLabels {
    ticker: String::new(),
    interval: String::new(),
    currency: String::new(),
    description: String::new(),
    ticker_interval: String::new(),
    logo_key: String::new(),
    logo: None,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::{PlotRect, PriceScale};
    use emath::Vec2;

    /// Records drawn text, filled rects, dashed lines and polylines.
    #[derive(Default)]
    struct Recorder {
        texts: Vec<String>,
        fills: Vec<(Rect, Color32)>,
        dashes: Vec<(f32, f32, f32)>,
        polylines: Vec<(usize, f32, Color32)>,
        /// Left edge of each text in `texts`.
        text_x: Vec<f32>,
        /// `Some`: the backend draws images (rects recorded here).
        images: Option<Vec<Rect>>,
        circles: usize,
    }

    impl Canvas for Recorder {
        fn pixels_per_point(&self) -> f32 {
            1.0
        }
        fn set_clip(&mut self, _: Rect) {}
        fn fill_rect(&mut self, r: Rect, c: Color32) {
            self.fills.push((r, c));
        }
        fn fill_rounded_rect(&mut self, _: Rect, _: f32, _: Color32) {}
        fn stroke_rect(&mut self, _: Rect, _: f32, _: Color32) {}
        fn line(&mut self, _: Pos2, _: Pos2, _: f32, _: Color32) {}
        fn dashed_line(&mut self, a: Pos2, _: Pos2, _: f32, _: Color32, dash: f32, gap: f32) {
            self.dashes.push((a.y, dash, gap));
        }
        fn polyline(&mut self, p: &[Pos2], w: f32, c: Color32) {
            self.polylines.push((p.len(), w, c));
        }
        fn fill_area(&mut self, _: &[Pos2], _: f32, _: f32, _: Color32, _: Color32) {}
        fn fill_between(&mut self, _: &[Pos2], _: &[Pos2], _: Color32) {}
        fn fill_circle(&mut self, _: Pos2, _: f32, _: Color32) {
            self.circles += 1;
        }
        fn image(&mut self, r: Rect, _: f32, _: &std::sync::Arc<std::path::Path>) -> bool {
            self.images.as_mut().map(|v| v.push(r)).is_some()
        }
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
            let r = anchor.anchor_size(pos, self.text_size_styled(text, size, bold));
            self.text_x.push(r.left());
            r
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
            indicators: &[],
            panes: Vec::new(),
            range: 0..series.len(),
            legend_collapsed: false,
            main_collapsed: false,
            legend_hover: None,
            legend_hover_button: None,
            price_axis: Rect::from_min_max(pos2(800.0, 0.0), pos2(880.0, 500.0)),
            time_axis: Rect::from_min_max(pos2(0.0, 500.0), pos2(800.0, 528.0)),
            display: series,
            projection_from: None,
            last_raw: series.last(),
            raw: series,
            labels: &NO_LABELS,
            time_fmt: TimeFormat::default(),
            bars: &[],
            decimated: false,
            crosshair: None,
            precision: 2,
            tick_size: Some(0.01),
            bar_interval_ms: 300_000,
            now_ms,
            bar_close_ms: Some(series.last().map_or(0, |c| c.open_time_ms + 300_000)),
            title: "",
            drawings: &NO_DRAWINGS,
            tz: crate::TzOffsets::utc_static(),
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
    fn indicator_legends_tags_and_panes() {
        let (s, theme, style) = (series(), Theme::dark(), ChartStyle::default());
        let mut inds = vec![
            Indicator::new(tt_core::StudyKind::Volume.default_study()),
            Indicator::new(tt_core::Study::Sma {
                length: 3,
                source: tt_core::Source::Close,
            }),
            Indicator::new(tt_core::Study::Rsi {
                length: 5,
                source: tt_core::Source::Close,
            }),
        ];
        for i in &mut inds {
            i.sync(&s);
        }
        let mut scene = scene_with(&s, &theme, &style, None);
        scene.indicators = &inds;
        scene.volume_rect = Some(Rect::from_min_max(pos2(0.0, 300.0), pos2(800.0, 400.0)));
        scene.vp.plot.bottom = 400.0;
        scene.panes = vec![Pane {
            index: 2,
            vp: Viewport {
                price_min: 0.0,
                price_max: 100.0,
                plot: PlotRect {
                    left: 0.0,
                    top: 400.0,
                    right: 800.0,
                    bottom: 500.0,
                },
                scale: PriceScale::linear(),
                ..scene.vp
            },
            collapsed: false,
        }];
        scene.legend_hover = Some(inds[1].id);
        let mut c = Recorder::default();
        let hits = draw(&mut c, &scene);
        for title in ["Vol", "MA 3 close", "RSI 5 close"] {
            assert!(c.texts.iter().any(|t| t == title), "{title}: {:?}", c.texts);
        }
        // Flat closes: the MA sits on the close; RSI of no change is 100.
        assert!(
            c.texts.iter().any(|t| t == "1.00"),
            "volume value {:?}",
            c.texts
        );
        assert!(c.texts.iter().any(|t| t == "100.00"), "{:?}", c.texts);
        // Hovered row has eye / settings / remove; plus the collapse button.
        let ma = hits.iter().find(|h| h.id == inds[1].id).unwrap();
        assert_eq!(ma.buttons.len(), 3);
        assert!(hits.iter().any(|h| h.id == 0));

        scene.legend_collapsed = true;
        let mut c = Recorder::default();
        draw(&mut c, &scene);
        assert!(!c.texts.iter().any(|t| t == "MA 3 close"));
        assert!(
            c.texts.iter().any(|t| t == "2"),
            "count of price-pane indicators"
        );
        assert!(
            c.texts.iter().any(|t| t == "RSI 5 close"),
            "pane legends stay"
        );
    }

    #[test]
    fn colliding_tags_are_spread_apart_within_bounds() {
        let tag = |y: f32| Tag {
            rect: Rect::from_center_size(pos2(840.0, y), vec2(78.0, 20.0)),
            lines: vec![],
            bg: Color32::RED,
            on_price_axis: true,
            name: None,
            ticker: false,
        };
        let mut tags = vec![tag(105.0), tag(100.0), tag(492.0)];
        spread_tags(&mut tags, 0.0, 500.0);
        assert_eq!(tags[0].rect.center().y, 100.0, "order kept, first stays");
        assert!(tags[1].rect.top() >= tags[0].rect.bottom());
        assert!(tags[2].rect.bottom() <= 500.0, "kept inside the pane");
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

    /// A scene with one RSI pane at 400..500 showing values 0..75, so the 70
    /// level runs through the pane's legend row.
    fn rsi_scene<'a>(
        s: &'a CandleSeries,
        theme: &'a Theme,
        style: &'a ChartStyle,
        inds: &'a [Indicator],
    ) -> Scene<'a> {
        let mut scene = scene_with(s, theme, style, None);
        scene.indicators = inds;
        scene.vp.plot.bottom = 400.0;
        scene.panes = vec![Pane {
            index: 0,
            vp: Viewport {
                price_min: 0.0,
                price_max: 75.0,
                plot: PlotRect {
                    left: 0.0,
                    top: 400.0,
                    right: 800.0,
                    bottom: 500.0,
                },
                scale: PriceScale::linear(),
                ..scene.vp
            },
            collapsed: false,
        }];
        scene
    }

    fn rsi(s: &CandleSeries) -> Indicator {
        let mut ind = Indicator::new(tt_core::StudyKind::Rsi.default_study());
        ind.sync(s);
        ind
    }

    #[test]
    fn levels_come_from_the_style_and_stay_behind_the_legend() {
        let (s, theme) = (series(), Theme::dark());
        let mut style = ChartStyle::default();
        let mut inds = [rsi(&s)];
        let scene = rsi_scene(&s, &theme, &style, &inds);
        let y70 = crisp(scene.panes[0].vp.y_of(70.0), 1.0);
        let y50 = crisp(scene.panes[0].vp.y_of(50.0), 1.0);
        let mut c = Recorder::default();
        draw_base(&mut c, &scene);
        // 70 / 30 dashed, 50 dotted.
        assert!(c.dashes.contains(&(y70, 4.0, 4.0)), "{:?}", c.dashes);
        assert!(c.dashes.contains(&(y50, 1.0, 4.0)), "{:?}", c.dashes);

        // The legend row is backed so the level does not run through it.
        let backed = |c: &Recorder| {
            c.fills
                .iter()
                .any(|(r, _)| r.contains(pos2(12.0, y70)) && r.top() >= 400.0)
        };
        let mut c = Recorder::default();
        draw_overlay(&mut c, &scene);
        assert!(backed(&c), "{:?}", c.fills);
        style.status_line.indicator_background = false;
        let scene = rsi_scene(&s, &theme, &style, &inds);
        let mut c = Recorder::default();
        draw_overlay(&mut c, &scene);
        assert!(!backed(&c), "no background when it is off");

        // Edited levels: moved, restyled, hidden.
        inds[0].style.hline_mut("upper").value = Some(60.0);
        inds[0].style.hline_mut("upper").style = Some(tt_core::LineStyle::Solid);
        inds[0].style.hline_mut("middle").visible = Some(false);
        let scene = rsi_scene(&s, &theme, &style, &inds);
        let mut c = Recorder::default();
        draw_base(&mut c, &scene);
        assert!(
            !c.dashes.contains(&(y70, 4.0, 4.0)) && !c.dashes.contains(&(y50, 1.0, 4.0)),
            "{:?}",
            c.dashes
        );
    }

    #[test]
    fn plot_style_and_status_line_switches() {
        let (s, theme, style) = (series(), Theme::dark(), ChartStyle::default());
        let mut inds = [rsi(&s)];
        let purple = inds[0].color(0);
        let scene = rsi_scene(&s, &theme, &style, &inds);
        let mut c = Recorder::default();
        draw(&mut c, &scene);
        assert!(c.polylines.iter().any(|p| p.1 == 1.0 && p.2 == purple));
        assert!(c.texts.iter().any(|t| t == "RSI 14 close"));
        let count = |c: &Recorder, t: &str| c.texts.iter().filter(|x| *x == t).count();
        // Readout open + legend value (100 is above the pane: no tag).
        assert_eq!(count(&c, "100.00"), 2, "{:?}", c.texts);

        let red = Color32::from_rgb(200, 0, 0);
        inds[0].set_color(0, red);
        inds[0].style.plot_mut("rsi").width = Some(3);
        inds[0].style.precision = Some(1);
        let scene = rsi_scene(&s, &theme, &style, &inds);
        let mut c = Recorder::default();
        draw(&mut c, &scene);
        assert!(c.polylines.iter().any(|p| p.1 == 3.0 && p.2 == red));
        assert_eq!(count(&c, "100.0"), 1, "{:?}", c.texts);

        // Per-indicator switches: no tag, no values, no inputs.
        inds[0].style.labels_on_price_scale = false;
        inds[0].style.values_in_status_line = false;
        inds[0].style.inputs_in_status_line = false;
        let scene = rsi_scene(&s, &theme, &style, &inds);
        let mut c = Recorder::default();
        draw(&mut c, &scene);
        assert_eq!(count(&c, "100.0"), 0, "{:?}", c.texts);
        assert_eq!(count(&c, "RSI"), 1);

        // Columns instead of a line; hidden plots are not drawn.
        inds[0].style.plot_mut("rsi").plot_type = Some(tt_core::PlotType::Columns);
        let scene = rsi_scene(&s, &theme, &style, &inds);
        let mut c = Recorder::default();
        draw_base(&mut c, &scene);
        assert!(!c.polylines.iter().any(|p| p.2 == red));
        assert!(c.fills.iter().filter(|f| f.1 == red).count() >= 5);
        inds[0].style.plot_mut("rsi").visible = Some(false);
        let scene = rsi_scene(&s, &theme, &style, &inds);
        let mut c = Recorder::default();
        draw_base(&mut c, &scene);
        assert!(!c.fills.iter().any(|f| f.1 == red));
    }

    #[test]
    fn previous_close_high_low_and_average() {
        const H: i64 = 3_600_000;
        // 30 hourly bars from 00:00 day 0: the last is 05:00 on day 1, so the
        // previous day closes with bar 23.
        let times: Vec<i64> = (0..30).map(|i| i * H).collect();
        let closes: Vec<f64> = (0..30).map(|i| i as f64).collect();
        assert_eq!(prev_day_close(&times, &closes, H), Some(23.0));
        assert_eq!(
            prev_day_close(&times[..10], &closes[..10], H),
            None,
            "one day only"
        );
        assert_eq!(prev_day_close(&times, &closes, scale::DAY_MS), Some(28.0));
        assert_eq!(prev_day_close(&[], &[], H), None);

        let bar = |low: f64, high: f64| DrawBar {
            x: 0.0,
            open: low,
            high,
            low,
            close: high,
            prev_close: low,
            value: high,
            prev_value: low,
            value_min: high,
            value_max: high,
            volume: 0.0,
        };
        assert_eq!(
            visible_high_low(&[bar(5.0, 9.0), bar(f64::NAN, 1.0), bar(3.0, 7.0)]),
            Some((3.0, 9.0))
        );
        assert_eq!(visible_high_low(&[]), None);
        assert_eq!(average(&[1.0, 2.0, f64::NAN, 6.0]), Some(3.0));
        assert_eq!(average(&[]), None);
    }

    fn day_series() -> CandleSeries {
        // 5-minute bars across midnight: the previous day closes at 100.
        let mut s = CandleSeries::new();
        for i in 0..20 {
            let close = if i < 10 { 100.0 } else { 101.0 };
            s.upsert(Candle {
                open_time_ms: scale::DAY_MS - 10 * 300_000 + i * 300_000,
                open: 100.0,
                high: 102.0,
                low: 99.0,
                close,
                volume: 1.0,
            });
        }
        s
    }

    #[test]
    fn symbol_label_modes_and_marks() {
        use crate::style::LabelMode;
        let (s, theme) = (day_series(), Theme::dark());
        let labels = crate::view::SymbolLabels {
            ticker: "BTCUSDT".into(),
            currency: "USDT".into(),
            ..Default::default()
        };
        let base_texts = |style: &ChartStyle| {
            let mut scene = scene_with(&s, &theme, style, None);
            scene.labels = &labels;
            let mut c = Recorder::default();
            draw_base(&mut c, &scene);
            c
        };
        let mut style = ChartStyle::default();
        let c = base_texts(&style);
        assert!(c.texts.iter().any(|t| t == "101.00"));
        assert!(!c.texts.iter().any(|t| t == "BTCUSDT"));
        assert!(
            !c.texts.iter().any(|t| t == "USDT"),
            "currency on hover only"
        );

        style.scales.symbol_label = LabelMode::NameValueLine;
        style.scales.symbol_value_mode = LastValueMode::PriceAndPercent;
        style.scales.currency_label = Visibility3::Always;
        let c = base_texts(&style);
        for want in ["BTCUSDT", "101.00", "+1.00%", "USDT"] {
            assert!(c.texts.iter().any(|t| t == want), "{want}: {:?}", c.texts);
        }

        style.scales.symbol_label = LabelMode::Name;
        let c = base_texts(&style);
        assert!(c.texts.iter().any(|t| t == "BTCUSDT"));
        assert!(!c.texts.iter().any(|t| t == "101.00"), "{:?}", c.texts);

        // Price line: picked color and width.
        style.scales.symbol_label = LabelMode::Line;
        style.scales.price_line_width = 3.0;
        let mut scene = scene_with(&s, &theme, &style, None);
        scene.labels = &labels;
        let y = crisp(scene.vp.y_of(101.0), 1.0);
        let mut c = Recorder::default();
        draw_base(&mut c, &scene);
        assert!(c.dashes.contains(&(y, 3.0, 6.0)), "{:?}", c.dashes);

        // Previous close, high / low labels and lines.
        let mut style = ChartStyle::default();
        style.scales.prev_close.label = true;
        style.scales.prev_close.line = true;
        style.scales.high_low.label = true;
        let mut scene = scene_with(&s, &theme, &style, None);
        let bars =
            crate::bars::collect_bars(&s, 0..s.len(), &scene.vp, crate::style::PriceSource::Close);
        scene.bars = &bars;
        let y_prev = crisp(scene.vp.y_of(100.0), 1.0);
        let mut c = Recorder::default();
        draw_base(&mut c, &scene);
        assert!(
            c.texts.iter().any(|t| t == "100.00"),
            "prev close {:?}",
            c.texts
        );
        assert!(c.texts.iter().any(|t| t == "H 102.00"), "{:?}", c.texts);
        assert!(c.texts.iter().any(|t| t == "L 99.00"), "{:?}", c.texts);
        assert!(c.dashes.contains(&(y_prev, 4.0, 4.0)), "{:?}", c.dashes);
    }

    #[test]
    fn indicator_name_modes() {
        use crate::style::LabelMode;
        let (s, theme) = (series(), Theme::dark());
        let inds = [rsi(&s)];
        let mut style = ChartStyle::default();
        // RSI of flat closes is 100: widen the pane so the tag shows.
        let tags = |style: &ChartStyle| {
            let mut scene = rsi_scene(&s, &theme, style, &inds);
            scene.panes[0].vp.price_max = 120.0;
            let mut c = Recorder::default();
            draw_base(&mut c, &scene);
            c.texts
        };
        // "100.00" may also be a grid label: compare counts between modes.
        let count = |t: &[String], w: &str| t.iter().filter(|x| *x == w).count();
        let value = tags(&style);
        assert!(count(&value, "RSI") == 0);
        style.scales.indicators = LabelMode::Hidden;
        let hidden = tags(&style);
        assert!(count(&hidden, "RSI") == 0);
        style.scales.indicators = LabelMode::NameValue;
        let both = tags(&style);
        assert_eq!(count(&both, "RSI"), 1, "{both:?}");
        assert_eq!(count(&both, "100.00"), count(&value, "100.00"));
        style.scales.indicators = LabelMode::Name;
        let name = tags(&style);
        assert_eq!(count(&name, "RSI"), 1);
        assert_eq!(count(&name, "100.00"), count(&hidden, "100.00"));
        assert!(count(&value, "100.00") >= 1);
    }

    #[test]
    fn canvas_background_grid_and_crosshair_styles() {
        let (s, theme) = (series(), Theme::dark());
        let mut style = ChartStyle::default();
        // Solid: one full fill in the background color.
        let scene = scene_with(&s, &theme, &style, None);
        let mut c = Recorder::default();
        draw_base(&mut c, &scene);
        assert_eq!(c.fills[0], (scene.full, theme.bg));

        style.canvas.background_kind = BackgroundKind::Gradient;
        style.canvas.background_bottom = Some(crate::Rgba::rgb(200, 0, 0));
        let scene = scene_with(&s, &theme, &style, None);
        let mut c = Recorder::default();
        draw_base(&mut c, &scene);
        let first = c.fills.first().unwrap();
        let last_band = c.fills[63];
        assert!(first.0.top() == 0.0 && first.1 != last_band.1);
        assert!(last_band.1.r() > 150, "bottom is the picked color");

        // Grid styles: dashed horizontal lines, solid vertical ones.
        let mut style = ChartStyle::default();
        style.canvas.grid_h_kind = LineKind::Dashed;
        style.canvas.grid_v_kind = LineKind::Solid;
        let scene = scene_with(&s, &theme, &style, None);
        let mut c = Recorder::default();
        draw_base(&mut c, &scene);
        assert!(
            c.dashes.iter().any(|d| d.1 == 4.0 && d.2 == 3.0),
            "{:?}",
            c.dashes
        );
        assert!(
            !c.dashes.iter().any(|d| d.1 == 1.0 && d.2 == 3.0),
            "no dotted grid"
        );

        // Crosshair: dotted, 2 wide.
        style.canvas.crosshair_kind = LineKind::Dotted;
        style.canvas.crosshair_width = 2.0;
        let mut scene = scene_with(&s, &theme, &style, None);
        scene.crosshair = Some(pos2(400.0, 250.0));
        let mut c = Recorder::default();
        draw_overlay(&mut c, &scene);
        assert!(c.dashes.contains(&(250.5, 2.0, 4.0)), "{:?}", c.dashes);
    }

    #[test]
    fn crosshair_time_tag_follows_formats() {
        let (s, theme) = (series(), Theme::dark());
        let mut style = ChartStyle::default();
        style.scales.date_format = crate::style::DateFormat::Iso;
        style.scales.hours_format = HoursFormat::H12;
        style.scales.day_of_week = false;
        let mut scene = scene_with(&s, &theme, &style, None);
        scene.time_fmt = TimeFormat::from_style(&style.scales);
        scene.crosshair = Some(pos2(scene.vp.x_of(2.0), 250.0));
        let mut c = Recorder::default();
        draw_overlay(&mut c, &scene);
        assert!(
            c.texts.iter().any(|t| t == "1970-01-01  12:10 AM"),
            "{:?}",
            c.texts
        );
    }

    #[test]
    fn watermark_lines() {
        let (s, theme) = (series(), Theme::dark());
        let labels = crate::view::SymbolLabels {
            ticker_interval: "BTCUSDT, 1h".into(),
            description: "BTC / USDT".into(),
            ..Default::default()
        };
        let mut style = ChartStyle::default();
        let draw_wm = |style: &ChartStyle| {
            let mut scene = scene_with(&s, &theme, style, None);
            scene.labels = &labels;
            let mut c = Recorder::default();
            draw_base(&mut c, &scene);
            c.texts
        };
        assert!(!draw_wm(&style).iter().any(|t| t == "BTCUSDT, 1h"));
        style.canvas.watermark.visible = true;
        let t = draw_wm(&style);
        assert!(t.iter().any(|t| t == "BTCUSDT, 1h") && t.iter().any(|t| t == "BTC / USDT"));
        style.canvas.watermark.description = false;
        assert!(!draw_wm(&style).iter().any(|t| t == "BTC / USDT"));
    }

    #[test]
    fn status_line_logo_shifts_the_readout() {
        let (s, theme) = (series(), Theme::dark());
        let mut style = ChartStyle::default();
        let x_of = |c: &Recorder, t: &str| c.text_x[c.texts.iter().position(|x| x == t).unwrap()];
        let mut labels = crate::view::SymbolLabels::default();
        let draw_with = |labels: &crate::view::SymbolLabels, style: &ChartStyle, images: bool| {
            let mut scene = scene_with(&s, &theme, style, None);
            scene.title = "BTCUSDT · 1h · Binance";
            scene.labels = labels;
            let mut c = Recorder {
                images: images.then(Vec::new),
                ..Default::default()
            };
            readout(&mut c, &scene, None);
            c
        };
        // No asset: no logo.
        let plain = draw_with(&labels, &style, true);
        let x0 = x_of(&plain, "BTCUSDT · 1h · Binance");
        assert_eq!(plain.circles, 0);

        // Known asset, no cached logo: the letter badge, text shifted.
        labels.logo_key = "BTC".into();
        let badge = draw_with(&labels, &style, true);
        assert!(badge.texts.iter().any(|t| t == "B"));
        assert_eq!(badge.circles, 1);
        let x1 = x_of(&badge, "BTCUSDT · 1h · Binance");
        assert!(x1 > x0 + 15.0, "{x0} → {x1}");

        // Logo file: the image instead of the badge, same shift.
        labels.logo = Some(std::path::Path::new("logos/BTC.png").into());
        let img = draw_with(&labels, &style, true);
        assert_eq!(img.images.as_ref().map(Vec::len), Some(1));
        assert_eq!(img.circles, 0);
        assert_eq!(x_of(&img, "BTCUSDT · 1h · Binance"), x1);
        // A backend without images falls back to the badge.
        assert_eq!(draw_with(&labels, &style, false).circles, 1);

        // Logo off in Settings › Status line.
        style.status_line.logo = false;
        let off = draw_with(&labels, &style, true);
        assert_eq!(x_of(&off, "BTCUSDT · 1h · Binance"), x0);
    }
}
