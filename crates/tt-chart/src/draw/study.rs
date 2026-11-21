//! Indicator drawing, driven only by the study metadata
//! ([`tt_core::StudyMeta`]: plots, levels, fills) and the indicator's
//! [`IndicatorStyle`](crate::indicator_style::IndicatorStyle). The only
//! per-kind rules are the palette plots the spec defines: MACD-style signed
//! columns and Volume columns colored by bar direction.
//!
//! Points are collected in thread-local scratch buffers, so steady-state
//! frames do not allocate.

use super::series::BarGeom;
use super::{Scene, Tag, crisp, price_axis_tag, volume_vp};
use crate::bars::DrawBar;
use crate::indicator_style::{ResolvedHLine, ResolvedPlot, line_style};
use crate::indicators::Indicator;
use crate::render::Canvas;
use crate::viewport::Viewport;
use ecolor::Color32;
use emath::{Pos2, Rect, pos2};
use std::cell::RefCell;
use tt_core::{FillEdge, LineStyle, Placement, PlotDef, PlotKind, PlotType, Study};
use tt_theme::with_alpha;

thread_local! {
    /// Point buffers reused by every plot and fill (cleared, never shrunk).
    static SCRATCH: RefCell<(Vec<Pos2>, Vec<Pos2>)> =
        const { RefCell::new((Vec::new(), Vec::new())) };
}

/// Runs `f` with the two cleared scratch buffers. Not re-entrant.
fn with_scratch<R>(f: impl FnOnce(&mut Vec<Pos2>, &mut Vec<Pos2>) -> R) -> R {
    SCRATCH.with(|s| {
        let mut s = s.borrow_mut();
        let (a, b) = &mut *s;
        a.clear();
        b.clear();
        f(a, b)
    })
}

/// Fill opacity of area plots, relative to the plot color's.
const AREA_FILL: f32 = 0.2;

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
pub(super) fn volume_rising_at(s: &Scene<'_>, ind: &Indicator, i: usize) -> bool {
    let Some(b) = s.display.get(i) else {
        return true;
    };
    let prev = i.checked_sub(1).map_or(b.open, |j| s.display.closes()[j]);
    volume_rising(ind, b.close, b.open, prev)
}

/// Column colors of a Volume plot (falling, growing): the user's palette
/// entries, else the theme's volume colors.
fn volume_colors(s: &Scene<'_>, p: &ResolvedPlot) -> (Color32, Color32) {
    let pick = |i: usize, theme: Color32| {
        if p.is_overridden(i) {
            p.palette(i).to_color32()
        } else {
            theme
        }
    };
    (pick(0, s.theme.volume_down), pick(1, s.theme.volume_up))
}

/// Palette index of a signed column (TradingView's MACD rule): grow above,
/// fall above, rise below, fall below.
#[inline]
fn signed_index(v: f64, prev: f64) -> usize {
    match (v >= 0.0, prev < v) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    }
}

/// Per-bar colors of one plot, resolved once per frame.
struct BarColors {
    kind: PlotKind,
    palette: [Color32; 4],
}

impl BarColors {
    fn new(s: &Scene<'_>, def: &PlotDef, p: &ResolvedPlot) -> Self {
        let mut palette = [p.color().to_color32(); 4];
        match def.kind {
            PlotKind::VolumeColumns => {
                let (down, up) = volume_colors(s, p);
                palette[0] = down;
                palette[1] = up;
            }
            _ => {
                for (i, c) in palette.iter_mut().enumerate() {
                    *c = p.palette(i).to_color32();
                }
            }
        }
        Self {
            kind: def.kind,
            palette,
        }
    }

    /// Color of value index `j`, drawn at bar `bar`.
    #[inline]
    fn at(&self, s: &Scene<'_>, ind: &Indicator, values: &[f64], j: usize, bar: i64) -> Color32 {
        match self.kind {
            PlotKind::SignedColumns => {
                let v = values[j];
                let prev = j
                    .checked_sub(1)
                    .map(|i| values[i])
                    .filter(|p| p.is_finite())
                    .unwrap_or(v);
                self.palette[signed_index(v, prev)]
            }
            PlotKind::VolumeColumns => {
                let rising = usize::try_from(bar).is_ok_and(|b| volume_rising_at(s, ind, b));
                self.palette[usize::from(rising)]
            }
            PlotKind::Value | PlotKind::Shape => self.palette[0],
        }
    }
}

/// Value indices to sample when drawing a plot shifted by `off` bars: every
/// bar when zoomed in, about one per pixel column when zoomed out; one extra
/// bar on each side so lines reach the plot edges. A positive offset also
/// covers the empty slots past the last bar.
fn sample_indices(
    s: &Scene<'_>,
    vp: &Viewport,
    ppp: f32,
    off: i64,
    n: usize,
) -> impl Iterator<Item = usize> + use<> {
    let stride = (1.0 / (s.vp.bar_spacing * ppp)).ceil().max(1.0) as usize;
    let first_bar = s.range.start as i64 - 1;
    let mut end_bar = s.range.end as i64 + 1;
    if off > 0 {
        end_bar = end_bar.max(vp.slot_at(vp.plot.right) + 2);
    }
    let start = ((first_bar - off).max(0) as usize) / stride * stride;
    let end = ((end_bar - off).max(0) as usize).min(n).max(start);
    let last = end
        .checked_sub(1)
        .filter(|l| *l >= start && !(l - start).is_multiple_of(stride));
    (start..end).step_by(stride).chain(last)
}

/// Screen point of value index `j` (offset applied).
#[inline]
fn point(vp: &Viewport, j: usize, off: i64, v: f64) -> Pos2 {
    pos2(vp.x_of((j as i64 + off) as f32), vp.y_of(v))
}

/// Strokes `pts` (if it has a segment) and clears it.
fn flush_line(c: &mut dyn Canvas, pts: &mut Vec<Pos2>, width: f32, color: Color32, ls: LineStyle) {
    if pts.len() > 1 {
        if ls == LineStyle::Solid {
            c.polyline(pts, width, color);
        } else {
            c.stroke_polyline_styled(pts, width, color, line_style(ls));
        }
    }
    pts.clear();
}

/// Draws every enabled study element in order: fills, levels, plots
/// (in metadata order), price lines.
pub(super) fn study_plots(c: &mut dyn Canvas, s: &Scene<'_>, vp: &Viewport, ind: &Indicator) {
    let meta = ind.study.meta();
    let outs = ind.outputs();
    let off = ind.offset();
    let ppp = c.pixels_per_point();
    let rect = super::vp_rect(vp);
    for def in meta.fills {
        let f = ind.style.fill(def);
        if !f.visible || f.color.a() == 0 {
            continue;
        }
        // Gradient fills (`color2`) are drawn in their first color for now.
        let color = f.color.to_color32();
        let hline = |id: &str| meta.hline(id).map(|d| ind.style.hline(d).value);
        let plot = |id: &str| {
            meta.plot_index(id)
                .filter(|&k| meta.plots[k].enabled_if.is_none_or(|f| f(&ind.study)))
                .and_then(|k| outs.get(k))
        };
        match def.between {
            (FillEdge::HLine(a), FillEdge::HLine(b)) => {
                if let (Some(a), Some(b)) = (hline(a), hline(b)) {
                    let (ya, yb) = (vp.y_of(a), vp.y_of(b));
                    let band = Rect::from_x_y_ranges(rect.x_range(), ya.min(yb)..=ya.max(yb));
                    c.fill_rect(band.intersect(rect), color);
                }
            }
            (FillEdge::Plot(a), FillEdge::Plot(b)) => {
                if let (Some(a), Some(b)) = (plot(a), plot(b)) {
                    fill_band(c, s, vp, off, a, |j| b.get(j).copied(), color);
                }
            }
            (FillEdge::Plot(p), FillEdge::HLine(h)) | (FillEdge::HLine(h), FillEdge::Plot(p)) => {
                if let (Some(a), Some(v)) = (plot(p), hline(h)) {
                    fill_band(c, s, vp, off, a, |_| Some(v), color);
                }
            }
        }
    }
    for def in meta.hlines {
        let h = ind.style.hline(def);
        if h.visible {
            hline(c, rect, vp, &h, ppp);
        }
    }
    for (k, def) in meta.plots.iter().enumerate() {
        if !ind.output_enabled(k) {
            continue;
        }
        let Some(values) = outs.get(k) else { continue };
        let p = ind.style.plot(def);
        plot(c, s, vp, ind, def, &p, values, off);
        if p.price_line
            && let Some(v) = last_value(values)
        {
            let y = crisp(vp.y_of(v), ppp);
            if (vp.plot.top..=vp.plot.bottom).contains(&y) {
                c.line(
                    pos2(rect.left(), y),
                    pos2(rect.right(), y),
                    1.0 / ppp,
                    p.color().to_color32(),
                );
            }
        }
    }
}

/// Fills between output `a` and `b(j)` (another output or a level), broken
/// where either is undefined.
fn fill_band(
    c: &mut dyn Canvas,
    s: &Scene<'_>,
    vp: &Viewport,
    off: i64,
    a: &[f64],
    b: impl Fn(usize) -> Option<f64>,
    color: Color32,
) {
    let ppp = c.pixels_per_point();
    with_scratch(|top, bot| {
        for j in sample_indices(s, vp, ppp, off, a.len()) {
            match (a[j], b(j)) {
                (u, Some(l)) if u.is_finite() && l.is_finite() => {
                    top.push(point(vp, j, off, u));
                    bot.push(point(vp, j, off, l));
                }
                _ => {
                    if top.len() > 1 {
                        c.fill_between(top, bot, color);
                    }
                    top.clear();
                    bot.clear();
                }
            }
        }
        if top.len() > 1 {
            c.fill_between(top, bot, color);
        }
    });
}

/// A horizontal level across `rect`. Width 1 is one physical pixel; dash
/// lengths scale with the width.
fn hline(c: &mut dyn Canvas, rect: Rect, vp: &Viewport, h: &ResolvedHLine, ppp: f32) {
    let y = crisp(vp.y_of(h.value), ppp);
    if !(rect.top()..=rect.bottom()).contains(&y) {
        return;
    }
    let (a, b) = (pos2(rect.left(), y), pos2(rect.right(), y));
    let unit = h.width as f32;
    let width = if h.width <= 1 { 1.0 / ppp } else { unit };
    let color = h.color.to_color32();
    match h.style {
        LineStyle::Solid => c.line(a, b, width, color),
        LineStyle::Dashed => c.dashed_line(a, b, width, color, 4.0 * unit, 4.0 * unit),
        LineStyle::Dotted => c.dashed_line(a, b, width, color, unit, 4.0 * unit),
    }
}

/// One plot in its plot type.
#[allow(clippy::too_many_arguments)]
fn plot(
    c: &mut dyn Canvas,
    s: &Scene<'_>,
    vp: &Viewport,
    ind: &Indicator,
    def: &PlotDef,
    p: &ResolvedPlot,
    values: &[f64],
    off: i64,
) {
    let ppp = c.pixels_per_point();
    let colors = BarColors::new(s, def, p);
    let width = p.width as f32;
    let color = colors.palette[0];
    match p.plot_type {
        PlotType::Columns | PlotType::Histogram => {
            if def.kind == PlotKind::VolumeColumns && p.plot_type == PlotType::Columns && off == 0 {
                // Per-pixel-column aggregates when zoomed out (as the bars).
                volume_columns(c, s, vp, ind, &colors, s.bars);
                return;
            }
            let g = BarGeom::new(s.vp.bar_spacing, ppp);
            let w_px = if p.plot_type == PlotType::Histogram {
                (width * ppp).round().max(1.0)
            } else if s.decimated {
                1.0
            } else {
                g.body_px
            };
            let y0 = vp.y_of(0.0);
            for j in sample_indices(s, vp, ppp, off, values.len()) {
                let v = values[j];
                if !v.is_finite() {
                    continue;
                }
                let bar = j as i64 + off;
                let (a, b) = g.span(y0, vp.y_of(v));
                let x = vp.x_of(bar as f32);
                c.fill_rect(
                    g.column(g.center_px(x), w_px, a, b),
                    colors.at(s, ind, values, j, bar),
                );
            }
        }
        PlotType::Line | PlotType::LineBr | PlotType::Step | PlotType::StepBr => {
            let breaks = matches!(p.plot_type, PlotType::LineBr | PlotType::StepBr);
            let step = matches!(p.plot_type, PlotType::Step | PlotType::StepBr);
            line(
                c,
                s,
                vp,
                values,
                off,
                width,
                color,
                p.line_style,
                breaks,
                step,
            );
        }
        PlotType::StepDiamond => {
            line(
                c,
                s,
                vp,
                values,
                off,
                width,
                color,
                p.line_style,
                true,
                true,
            );
            markers(c, s, vp, values, off, PlotType::StepDiamond, width, color);
        }
        PlotType::Circles | PlotType::Cross => {
            markers(c, s, vp, values, off, p.plot_type, width, color);
        }
        PlotType::Area | PlotType::AreaBr => {
            let breaks = p.plot_type == PlotType::AreaBr;
            let fill = with_alpha(color, p.color().opacity() * AREA_FILL);
            let base = vp.y_of(0.0).clamp(vp.plot.top, vp.plot.bottom);
            with_scratch(|pts, _| {
                let flush = |c: &mut dyn Canvas, pts: &mut Vec<Pos2>| {
                    if pts.len() > 1 {
                        c.fill_area(pts, base, vp.plot.top, fill, fill);
                    }
                    flush_line(c, pts, width, color, p.line_style);
                };
                for j in sample_indices(s, vp, ppp, off, values.len()) {
                    let v = values[j];
                    if v.is_finite() {
                        pts.push(point(vp, j, off, v));
                    } else if breaks {
                        flush(c, pts);
                    }
                }
                flush(c, pts);
            });
        }
    }
}

/// A plot as a polyline. `breaks`: gaps where the value is undefined
/// (otherwise they are bridged); `step`: horizontal then vertical.
#[allow(clippy::too_many_arguments)]
fn line(
    c: &mut dyn Canvas,
    s: &Scene<'_>,
    vp: &Viewport,
    values: &[f64],
    off: i64,
    width: f32,
    color: Color32,
    ls: LineStyle,
    breaks: bool,
    step: bool,
) {
    let ppp = c.pixels_per_point();
    with_scratch(|pts, _| {
        for j in sample_indices(s, vp, ppp, off, values.len()) {
            let v = values[j];
            if v.is_finite() {
                let q = point(vp, j, off, v);
                if step && let Some(prev) = pts.last() {
                    let corner = pos2(q.x, prev.y);
                    pts.push(corner);
                }
                pts.push(q);
            } else if breaks {
                flush_line(c, pts, width, color, ls);
            }
        }
        flush_line(c, pts, width, color, ls);
    });
}

/// Circles, crosses or diamonds at each sampled value.
#[allow(clippy::too_many_arguments)]
fn markers(
    c: &mut dyn Canvas,
    s: &Scene<'_>,
    vp: &Viewport,
    values: &[f64],
    off: i64,
    shape: PlotType,
    width: f32,
    color: Color32,
) {
    let ppp = c.pixels_per_point();
    let r = width + 1.5;
    for j in sample_indices(s, vp, ppp, off, values.len()) {
        let v = values[j];
        if !v.is_finite() {
            continue;
        }
        let q = point(vp, j, off, v);
        match shape {
            PlotType::Cross => {
                c.line(pos2(q.x - r, q.y), pos2(q.x + r, q.y), width, color);
                c.line(pos2(q.x, q.y - r), pos2(q.x, q.y + r), width, color);
            }
            PlotType::StepDiamond => {
                let d = [
                    pos2(q.x, q.y - r),
                    pos2(q.x + r, q.y),
                    pos2(q.x, q.y + r),
                    pos2(q.x - r, q.y),
                ];
                c.fill_polygon(&d, color);
            }
            _ => c.fill_circle(q, r, color),
        }
    }
}

/// Volume columns from the draw bars (aggregated per pixel column when
/// zoomed out), on the Volume overlay's scale.
fn volume_columns(
    c: &mut dyn Canvas,
    s: &Scene<'_>,
    vp: &Viewport,
    ind: &Indicator,
    colors: &BarColors,
    bars: &[DrawBar],
) {
    let g = BarGeom::new(s.vp.bar_spacing, c.pixels_per_point());
    let width = if s.decimated { 1.0 } else { g.body_px };
    let bottom = vp.plot.bottom;
    for b in bars {
        let rising = volume_rising(ind, b.close, b.open, b.prev_close);
        let top = vp.y_of(b.volume);
        c.fill_rect(
            g.column(g.center_px(b.x), width, top, bottom),
            colors.palette[usize::from(rising)],
        );
    }
}

/// Volume indicator: columns (and the MA) in the bottom of the price pane,
/// behind the bars like TradingView.
pub(super) fn volume(c: &mut dyn Canvas, s: &Scene<'_>, rect: Rect, ind: &Indicator) {
    let vvp = volume_vp(s, rect);
    study_plots(c, s, &vvp, ind);
}

/// Value at the last bar, if defined.
fn last_value(values: &[f64]) -> Option<f64> {
    values.last().copied().filter(|v| v.is_finite())
}

/// Opaque color of output `k` at value index `j` (axis tags and legend
/// values), following the same palette rules as the plot.
pub(super) fn value_color(s: &Scene<'_>, ind: &Indicator, k: usize, j: usize) -> Color32 {
    let (Some(def), Some(values)) = (ind.plot_def(k), ind.outputs().get(k)) else {
        return s.theme.text;
    };
    let p = ind.style.plot(def);
    let c = match def.kind {
        PlotKind::VolumeColumns if !p.is_overridden(0) && !p.is_overridden(1) => {
            let bar = j as i64 + ind.offset();
            let rising = usize::try_from(bar).is_ok_and(|b| volume_rising_at(s, ind, b));
            return if rising { s.theme.up } else { s.theme.down };
        }
        _ if j < values.len() => {
            BarColors::new(s, def, &p).at(s, ind, values, j, j as i64 + ind.offset())
        }
        _ => p.color().to_color32(),
    };
    with_alpha(c, 1.0)
}

/// Value tags on the price axis for every drawn indicator output
/// (TradingView's "Labels on price scale", per indicator and globally).
pub(super) fn study_tags(s: &Scene<'_>) -> Vec<(usize, Tag)> {
    let mut tags = Vec::new();
    for (idx, ind) in s.indicators.iter().enumerate() {
        if !ind.visible || !ind.style.labels_on_price_scale {
            continue;
        }
        let (pane_no, vp) = match ind.placement() {
            Placement::Overlay => (0, s.vp),
            Placement::VolumeOverlay => {
                let Some(vr) = s.volume_rect else { continue };
                (0, volume_vp(s, vr))
            }
            Placement::Pane => {
                let Some((no, pane)) = s.panes.iter().enumerate().find(|(_, p)| p.index == idx)
                else {
                    continue;
                };
                (no + 1, pane.vp)
            }
        };
        for (k, values) in ind.outputs().iter().enumerate() {
            if !ind.output_enabled(k) {
                continue;
            }
            let Some(v) = last_value(values) else {
                continue;
            };
            let y = vp.y_of(v);
            if y >= vp.plot.top && y <= vp.plot.bottom {
                let bg = value_color(s, ind, k, values.len() - 1);
                tags.push((
                    pane_no,
                    price_axis_tag(s, y, vec![ind.format(v, s.precision)], bg),
                ));
            }
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_columns_follow_tradingviews_rule() {
        assert_eq!(signed_index(2.0, 1.0), 0, "grow above");
        assert_eq!(signed_index(1.0, 2.0), 1, "fall above");
        assert_eq!(signed_index(-1.0, -2.0), 2, "rise below");
        assert_eq!(signed_index(-2.0, -1.0), 3, "fall below");
    }
}
