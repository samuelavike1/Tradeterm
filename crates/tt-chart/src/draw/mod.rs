//! Chart scene drawing. Uses only the [`Canvas`] trait, never a UI toolkit directly.

use crate::bars::{DrawBar, max_volume};
use crate::indicators::{Indicator, format_value, histogram_down};
use crate::render::Canvas;
use crate::scale::{self, format_countdown, format_full_time, format_percent, format_price};
use crate::style::LineKind;
use crate::time_axis;
use crate::viewport::Viewport;
use crate::{ChartStyle, ChartType, ScaleMode};
use ecolor::Color32;
use emath::{Align2, Pos2, Rect, pos2, vec2};
use std::ops::Range;
use tt_core::{Candle, CandleSeries, Placement, Plot, Study};
use tt_theme::{Theme, with_alpha};

mod legend;
mod series;

use legend::{legends, readout};
use series::{BarGeom, series};

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
    /// Exact close time of the last bar, for the countdown.
    pub bar_close_ms: Option<i64>,
    pub title: &'a str,
    pub drawings: &'a crate::drawings::Drawings,
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
    c.set_clip(s.full);
    c.fill_rect(s.full, t.bg);

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

    let pane_ticks: Vec<Vec<(f32, String)>> = s.panes.iter().map(|p| pane_ticks(s, p)).collect();

    // Grid: one line per price label and per time label, like TradingView.
    let ppp = c.pixels_per_point();
    let cv = &s.style.canvas;
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

    if let Some(vr) = s.volume_rect.filter(|_| !s.main_collapsed) {
        c.set_clip(vr);
        for ind in s.indicators.iter().filter(|i| i.visible) {
            if ind.placement() == Placement::VolumeOverlay {
                volume(c, s, vr, ind);
            }
        }
    }
    if !s.main_collapsed {
        c.set_clip(s.vp_rect());
        series(c, s);
        for ind in s.indicators.iter().filter(|i| i.visible) {
            if ind.placement() == Placement::Overlay {
                study_plots(c, s, &s.vp, ind);
            }
        }
        last_price_line(c, s);
        crate::drawings::draw_base(c, s);
    }
    for pane in &s.panes {
        c.set_clip(pane.rect());
        let ind = &s.indicators[pane.index];
        if ind.visible && !pane.collapsed {
            pane_guides(c, s, pane, ind);
            study_plots(c, s, &pane.vp, ind);
        }
    }
    // Pane dividers across plot and price axis.
    c.set_clip(s.full);
    for pane in &s.panes {
        let y = crisp(pane.rect().top(), ppp) - 0.5 / ppp;
        c.line(
            pos2(s.full.left(), y),
            pos2(s.full.right(), y),
            1.0 / ppp,
            t.border,
        );
    }

    // Axis tags are computed before the tick labels so labels underneath can
    // be hidden. (The crosshair's tags are opaque and drawn by the overlay.)
    c.set_clip(s.full);
    // Axis tags per pane (0 = price pane), spread so they don't overlap.
    let mut grouped = if s.style.scales.indicators.shows_value() {
        study_tags(s)
    } else {
        Vec::new()
    };
    if let Some(tag) = last_price_tag(c, s) {
        grouped.push((0, tag));
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
    for tag in &tags {
        draw_tag(c, s, tag);
    }
    for tag in crate::drawings::axis_tags(s) {
        draw_tag(c, s, &tag);
    }
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

/// Whether a bar's volume counts as rising.
fn volume_rising(ind: &Indicator, close: f64, open: f64, prev_close: f64) -> bool {
    match ind.study {
        Study::Volume {
            color_by_prev_close: true,
            ..
        } => close >= prev_close,
        _ => close >= open,
    }
}

/// [`volume_rising`] for bar `i` of the displayed series.
fn volume_rising_at(s: &Scene<'_>, ind: &Indicator, i: usize) -> bool {
    let Some(b) = s.display.get(i) else {
        return true;
    };
    let prev = i.checked_sub(1).map_or(b.open, |j| s.display.closes()[j]);
    volume_rising(ind, b.close, b.open, prev)
}

/// Volume indicator: columns in the bottom of the price pane (behind the
/// bars, like TradingView), plus the optional MA line.
fn volume(c: &mut dyn Canvas, s: &Scene<'_>, rect: Rect, ind: &Indicator) {
    let vvp = volume_vp(s, rect);
    let g = BarGeom::new(s.vp.bar_spacing, c.pixels_per_point());
    let width = if s.decimated { 1.0 } else { g.body_px };
    for b in s.bars {
        let color = if volume_rising(ind, b.close, b.open, b.prev_close) {
            s.theme.volume_up
        } else {
            s.theme.volume_down
        };
        let top = vvp.y_of(b.volume);
        c.fill_rect(g.column(g.center_px(b.x), width, top, rect.bottom()), color);
    }
    if ind.output_enabled(1)
        && let Some(ma) = ind.outputs().get(1)
    {
        study_line(c, s, &vvp, ma, 1.0, ind.color(1));
    }
}

/// Indices to sample when drawing a study: every bar when zoomed in, about
/// one per pixel column when zoomed out; one extra bar on each side so lines
/// reach the plot edges.
fn sample_indices(s: &Scene<'_>, ppp: f32) -> impl Iterator<Item = usize> + use<> {
    let stride = (1.0 / (s.vp.bar_spacing * ppp)).ceil().max(1.0) as usize;
    let start = s.range.start.saturating_sub(1) / stride * stride;
    let end = (s.range.end + 1).min(s.display.len()).max(start);
    let last = end
        .checked_sub(1)
        .filter(|l| *l >= start && !(l - start).is_multiple_of(stride));
    (start..end).step_by(stride).chain(last)
}

/// A study output as a line, broken where it is undefined.
fn study_line(
    c: &mut dyn Canvas,
    s: &Scene<'_>,
    vp: &Viewport,
    values: &[f64],
    width: f32,
    color: Color32,
) {
    let mut pts: Vec<Pos2> = Vec::new();
    for i in sample_indices(s, c.pixels_per_point()) {
        match values.get(i) {
            Some(v) if v.is_finite() => pts.push(pos2(vp.x_of(i as f32), vp.y_of(*v))),
            _ => {
                if pts.len() > 1 {
                    c.polyline(&pts, width, color);
                }
                pts.clear();
            }
        }
    }
    if pts.len() > 1 {
        c.polyline(&pts, width, color);
    }
}

/// Draws every enabled output of a study (lines, histograms, band fills).
fn study_plots(c: &mut dyn Canvas, s: &Scene<'_>, vp: &Viewport, ind: &Indicator) {
    let outs = ind.outputs();
    if let Study::BollingerBands { .. } = ind.study
        && outs.len() == 3
    {
        // Light fill between the bands.
        let (mut top, mut bot) = (Vec::new(), Vec::new());
        for i in sample_indices(s, c.pixels_per_point()) {
            if let (Some(&u), Some(&l)) = (outs[1].get(i), outs[2].get(i))
                && u.is_finite()
                && l.is_finite()
            {
                let x = vp.x_of(i as f32);
                top.push(pos2(x, vp.y_of(u)));
                bot.push(pos2(x, vp.y_of(l)));
            }
        }
        c.fill_between(&top, &bot, with_alpha(ind.color(0), 0.08));
    }
    for (k, (_, plot)) in ind.study.outputs().iter().enumerate() {
        if !ind.output_enabled(k) {
            continue;
        }
        let Some(values) = outs.get(k) else { continue };
        match plot {
            Plot::Line => study_line(c, s, vp, values, 1.0, ind.color(k)),
            Plot::Histogram => histogram(c, s, vp, values, ind.color(k)),
            Plot::VolumeColumns => {}
        }
    }
}

/// Columns from zero with TradingView's MACD coloring: strong while growing
/// away from zero, faded while shrinking.
fn histogram(c: &mut dyn Canvas, s: &Scene<'_>, vp: &Viewport, values: &[f64], up: Color32) {
    let ppp = c.pixels_per_point();
    let g = BarGeom::new(s.vp.bar_spacing, ppp);
    let width = if s.decimated { 1.0 } else { g.body_px };
    let y0 = vp.y_of(0.0);
    for i in sample_indices(s, ppp) {
        let Some(&v) = values.get(i) else { continue };
        if !v.is_finite() {
            continue;
        }
        let prev = i
            .checked_sub(1)
            .and_then(|j| values.get(j))
            .copied()
            .filter(|p| p.is_finite())
            .unwrap_or(v);
        let base = if v >= 0.0 { up } else { histogram_down() };
        let color = if v.abs() >= prev.abs() {
            base
        } else {
            with_alpha(base, 0.45)
        };
        let (a, b) = g.span(y0, vp.y_of(v));
        c.fill_rect(g.column(g.center_px(vp.x_of(i as f32)), width, a, b), color);
    }
}

/// Guide levels of bounded oscillators (RSI 30/50/70…) with a band fill.
fn pane_guides(c: &mut dyn Canvas, s: &Scene<'_>, pane: &Pane, ind: &Indicator) {
    let Some((_, _, levels)) = ind.study.bounds() else {
        return;
    };
    let r = pane.rect();
    let ppp = c.pixels_per_point();
    if let (Some(lo), Some(hi)) = (levels.first(), levels.last()) {
        let band = Rect::from_x_y_ranges(r.x_range(), pane.vp.y_of(*hi)..=pane.vp.y_of(*lo));
        c.fill_rect(band, with_alpha(ind.color(0), 0.08));
    }
    for (i, l) in levels.iter().enumerate() {
        let y = crisp(pane.vp.y_of(*l), ppp);
        let middle = i > 0 && i + 1 < levels.len();
        let color = with_alpha(s.theme.text_muted, if middle { 0.35 } else { 0.6 });
        c.dashed_line(
            pos2(r.left(), y),
            pos2(r.right(), y),
            1.0 / ppp,
            color,
            if middle { 1.0 } else { 4.0 },
            4.0,
        );
    }
}

/// Price-axis labels of an indicator pane.
fn pane_ticks(s: &Scene<'_>, pane: &Pane) -> Vec<(f32, String)> {
    let ind = &s.indicators[pane.index];
    scale::price_ticks(&pane.vp, s.style.price_label_spacing, None)
        .into_iter()
        .map(|v| (pane.vp.y_of(v), format_value(&ind.study, v, s.precision)))
        .collect()
}

/// Value at the last bar, if defined.
fn last_value(values: &[f64]) -> Option<f64> {
    values.last().copied().filter(|v| v.is_finite())
}

/// Value tags on the price axis for every visible indicator output
/// (TradingView's "Labels on price scale").
fn study_tags(s: &Scene<'_>) -> Vec<(usize, Tag)> {
    let mut tags = Vec::new();
    let mut push = |pane: usize, vp: &Viewport, ind: &Indicator, v: f64, bg: Color32| {
        let y = vp.y_of(v);
        if y >= vp.plot.top && y <= vp.plot.bottom {
            tags.push((
                pane,
                price_axis_tag(s, y, vec![format_value(&ind.study, v, s.precision)], bg),
            ));
        }
    };
    let last = s.display.len().checked_sub(1);
    for (idx, ind) in s.indicators.iter().enumerate() {
        if !ind.visible {
            continue;
        }
        let outs = ind.outputs();
        let enabled = |k: usize| ind.output_enabled(k);
        match ind.placement() {
            Placement::Overlay => {
                for (k, v) in outs.iter().enumerate() {
                    if let Some(v) = last_value(v) {
                        push(0, &s.vp, ind, v, ind.color(k));
                    }
                }
            }
            Placement::VolumeOverlay => {
                let (Some(vr), Some(i)) = (s.volume_rect, last) else {
                    continue;
                };
                let bg = if volume_rising_at(s, ind, i) {
                    s.theme.up
                } else {
                    s.theme.down
                };
                if let Some(v) = outs.first().and_then(|o| last_value(o)) {
                    push(0, &volume_vp(s, vr), ind, v, bg);
                }
            }
            Placement::Pane => {
                let Some((pane_no, pane)) =
                    s.panes.iter().enumerate().find(|(_, p)| p.index == idx)
                else {
                    continue;
                };
                for (k, ((_, plot), v)) in ind.study.outputs().iter().zip(outs).enumerate() {
                    if !enabled(k) {
                        continue;
                    }
                    if let Some(v) = last_value(v) {
                        let bg = match plot {
                            Plot::Histogram if v < 0.0 => histogram_down(),
                            _ => ind.color(k),
                        };
                        push(pane_no + 1, &pane.vp, ind, v, bg);
                    }
                }
            }
        }
    }
    tags
}

/// One grid line: dotted (1 px on, 3 px off) or solid, per the style.
fn grid_line(c: &mut dyn Canvas, (color, kind): (Color32, LineKind), a: Pos2, b: Pos2) {
    if kind == LineKind::Solid {
        c.line(a, b, 1.0, color);
    } else {
        c.dotted_line(a, b, color);
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
    let slots = time_axis::SlotTimes::new(s.display.open_times(), s.bar_interval_ms);
    time_axis::time_ticks(
        &slots,
        vp.visible_slots(),
        (s.plot.left(), s.plot.right()),
        stride,
        gap,
        |i| crisp(vp.x_of(i as f32), ppp),
        |text| c.text_size_styled(text, size, true).x,
    )
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
    lines: Vec<String>,
    bg: Color32,
    /// Text aligned with the price labels (price axis) vs. centered (time axis).
    on_price_axis: bool,
}

pub(crate) fn draw_tag(c: &mut dyn Canvas, s: &Scene<'_>, tag: &Tag) {
    c.fill_rounded_rect(tag.rect, 2.0, tag.bg);
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
            .any(|t| *y + half > t.rect.top() && *y - half < t.rect.bottom())
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

/// Dotted last-price line across the plot in the last candle's color.
fn last_price_line(c: &mut dyn Canvas, s: &Scene<'_>) {
    if !s.style.scales.symbol_label.shows_line() {
        return;
    }
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
    if !s.style.scales.symbol_label.shows_value() {
        return None;
    }
    let last = s.last_raw?;
    let (color, _) = s.colors(last.is_bullish());
    let y = crisp(s.vp.y_of(last.close), c.pixels_per_point());
    if y < s.plot.top() || y > s.vp.plot.bottom {
        return None;
    }
    let mut lines = vec![axis_value_label(s, last.close)];
    if let (true, Some(now), Some(close)) = (s.style.scales.countdown, s.now_ms, s.bar_close_ms) {
        // Hidden once the bar should have closed (e.g. market closed / stale).
        let remaining = close - now;
        if remaining > 0 && close - last.open_time_ms >= remaining {
            lines.push(format_countdown(remaining));
        }
    }
    Some(price_axis_tag(s, y, lines, color))
}

/// Draws crosshair lines; returns its price-axis tag and time-axis tag.
fn crosshair(c: &mut dyn Canvas, s: &Scene<'_>, p: Pos2, slot: Option<i64>) -> (Tag, Option<Tag>) {
    let ppp = c.pixels_per_point();
    let col = s.theme.crosshair;
    // Snap the vertical line to the hovered bar's center.
    let x = crisp(slot.map_or(p.x, |i| s.vp.x_of(i as f32)), ppp);
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

    // Inside an indicator pane the axis shows the indicator's value.
    let label = match s.panes.iter().find(|pane| pane.rect().contains(p)) {
        Some(pane) => format_value(
            &s.indicators[pane.index].study,
            pane.vp.price_at(p.y),
            s.precision,
        ),
        None => axis_value_label(s, s.vp.price_at(p.y)),
    };
    let price_tag = price_axis_tag(s, y, vec![label], s.theme.crosshair_label_bg);

    let slots = time_axis::SlotTimes::new(s.display.open_times(), s.bar_interval_ms);
    let time_tag = slot.and_then(|i| slots.time_at(i)).map(|t| {
        let mut text = format_full_time(t, s.bar_interval_ms);
        if !s.style.scales.day_of_week
            && let Some((_, rest)) = text.split_once(' ')
        {
            text = rest.to_owned();
        }
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
        }
    });
    (price_tag, time_tag)
}

#[cfg(test)]
static NO_DRAWINGS: crate::drawings::Drawings = crate::drawings::Drawings::empty();

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::{PlotRect, PriceScale};
    use emath::Vec2;

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
        fn fill_between(&mut self, _: &[Pos2], _: &[Pos2], _: Color32) {}
        fn fill_circle(&mut self, _: Pos2, _: f32, _: Color32) {}
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
            indicators: &[],
            panes: Vec::new(),
            range: 0..series.len(),
            legend_collapsed: false,
            main_collapsed: false,
            legend_hover: None,
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
            bar_close_ms: Some(series.last().map_or(0, |c| c.open_time_ms + 300_000)),
            title: "",
            drawings: &NO_DRAWINGS,
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
}
