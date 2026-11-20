use crate::bars::{collect_bars, price_extent};
use crate::draw::{self, Scene};
use crate::lod::Lod;
use crate::render::{Canvas, EguiCanvas};
use crate::scale::{precision_from_tick_size, price_decimals};
use crate::viewport::{
    PlotRect, PriceScale, Viewport, fit_price_range, pan_price_range, visible_range_for,
    zoom_price_range,
};
use crate::{ChartStyle, ChartType, ScaleMode};
use egui::{CursorIcon, Rect, Response, Sense, Ui, pos2};
use std::time::{Duration, Instant};
use tt_core::{CandleSeries, HeikinAshi};
use tt_theme::Theme;

const DEFAULT_SPACING: f32 = 8.0;
const DEFAULT_RIGHT_OFFSET: f32 = 5.0;
const MIN_SPACING: f32 = 0.0005;
const MAX_SPACING: f32 = 80.0;
/// Fraction of the visible price span added above and below when auto-fitting.
const Y_PADDING: f64 = 0.08;
/// Below this bar width (physical pixels) bars are drawn decimated.
const DECIMATE_BELOW_PX: f32 = 2.0;
/// Zoom sensitivity: scale factor exponent per scrolled point.
const WHEEL_ZOOM_PER_POINT: f32 = 0.006;
/// Share of the plot height used by the volume pane.
const VOLUME_PANE_FRACTION: f32 = 0.2;

/// Per-frame numbers for performance overlays.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ChartStats {
    pub visible_bars: usize,
    pub total_bars: usize,
    /// CPU time to lay out and record the chart's draw commands
    /// (excludes egui tessellation and GPU work).
    pub draw_time: Duration,
}

/// Data handed to [`ChartView::show`] each frame.
#[derive(Clone, Copy)]
pub struct ChartData<'a> {
    pub series: &'a CandleSeries,
    /// Shown top-left before the OHLC readout.
    pub title: &'a str,
    /// Minimum price increment; drives label precision and the finest grid step.
    /// Precision is guessed from the price magnitude if `None`.
    pub tick_size: Option<f64>,
    /// Current time (Unix ms) for the bar-close countdown on the last-price
    /// tag; `None` hides it (e.g. for historical or synthetic data).
    pub now_ms: Option<i64>,
    /// The timeframe's bar length (ms). Inferred from the data if `None`.
    pub bar_interval_ms: Option<i64>,
    /// Exact close time of the last bar (calendar-aware for months), for the
    /// countdown on the last-price tag.
    pub bar_close_ms: Option<i64>,
}

/// What happened in [`ChartView::show`].
pub struct ChartOutput {
    /// Interaction response of the plot area.
    pub response: Response,
    /// The view is within [`BACKFILL_MARGIN_BARS`] of the oldest loaded bar;
    /// the caller should load older history (and prepend it) if available.
    pub needs_older_history: bool,
}

/// How close (in bars) to the oldest bar the view may get before
/// [`ChartOutput::needs_older_history`] is raised.
pub const BACKFILL_MARGIN_BARS: usize = 200;

/// Interactive chart widget.
///
/// Holds only view state; data is borrowed each frame.
///
/// - plot: drag pans (and moves the price range when it is manual), wheel/pinch
///   zooms around the pointer, horizontal wheel pans, double-click resets
/// - price axis: drag scales the price range, double-click returns to auto-fit
/// - time axis: drag zooms horizontally
#[derive(Debug, Clone)]
pub struct ChartView {
    pub style: ChartStyle,
    pub chart_type: ChartType,
    pub scale_mode: ScaleMode,
    pub show_volume: bool,
    stats: ChartStats,
    bar_spacing: f32,
    right_offset: f32,
    /// `None` = auto-fit to visible bars; otherwise a manual (min, max) in prices.
    manual_range: Option<(f64, f64)>,
    /// Price range used last frame (for switching auto → manual seamlessly).
    last_range: (f64, f64),
    last_scale_mode: ScaleMode,
    /// Heikin Ashi cache, kept in sync incrementally.
    ha: HeikinAshi,
    /// Min/max pyramid for decimated drawing, and whether it was built from HA bars.
    lod: Lod,
    lod_is_ha: bool,
    /// Requested zoom (number of bars to fit), applied on the next frame.
    pending_zoom: Option<usize>,
}

impl Default for ChartView {
    fn default() -> Self {
        Self::new(ChartStyle::default())
    }
}

impl ChartView {
    pub fn new(style: ChartStyle) -> Self {
        Self {
            style,
            chart_type: ChartType::default(),
            scale_mode: ScaleMode::default(),
            show_volume: true,
            stats: ChartStats::default(),
            bar_spacing: DEFAULT_SPACING,
            right_offset: DEFAULT_RIGHT_OFFSET,
            manual_range: None,
            last_range: (0.0, 1.0),
            last_scale_mode: ScaleMode::default(),
            ha: HeikinAshi::new(),
            lod: Lod::new(),
            lod_is_ha: false,
            pending_zoom: None,
        }
    }

    /// Statistics from the most recent [`Self::show`] call.
    pub fn stats(&self) -> ChartStats {
        self.stats
    }

    /// Default zoom, scrolled to the latest bar, auto-fit price axis.
    pub fn reset_view(&mut self) {
        self.bar_spacing = DEFAULT_SPACING;
        self.right_offset = DEFAULT_RIGHT_OFFSET;
        self.manual_range = None;
    }

    /// On the next frame, zoom so that `bars` bars fill the plot, newest at the right.
    pub fn zoom_to_bars(&mut self, bars: usize) {
        self.pending_zoom = Some(bars.max(1));
    }

    /// On the next frame, zoom out so the whole series is visible.
    pub fn fit_all(&mut self) {
        self.pending_zoom = Some(usize::MAX);
    }

    /// `true` while the price axis auto-fits the visible bars.
    pub fn is_auto_scale(&self) -> bool {
        self.manual_range.is_none()
    }

    pub fn show(&mut self, ui: &mut Ui, data: ChartData<'_>, theme: &Theme) -> ChartOutput {
        let started = Instant::now();
        let series = data.series;
        let n = series.len();
        if self.scale_mode != self.last_scale_mode {
            self.manual_range = None;
            self.last_scale_mode = self.scale_mode;
        }

        let (full, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let mut canvas =
            EguiCanvas::new(ui.painter_at(full)).with_bold_family(self.style.bold_family.clone());

        let precision = match data.tick_size {
            Some(t) => precision_from_tick_size(t),
            None => series.last().map_or(2, |c| price_decimals(c.close)),
        };
        let (plot, price_axis, time_axis) = self.layout(full, &mut canvas, series, precision);

        // ---- interaction --------------------------------------------------
        let id = ui.make_persistent_id("tt_chart");
        let plot_resp = ui.interact(plot, id.with("plot"), Sense::click_and_drag());
        let price_resp = ui.interact(price_axis, id.with("price_axis"), Sense::click_and_drag());
        let time_resp = ui.interact(time_axis, id.with("time_axis"), Sense::click_and_drag());

        if let Some(bars) = self.pending_zoom
            && n > 0
        {
            self.pending_zoom = None;
            let bars = bars.min(n) as f32;
            self.bar_spacing = (plot.width() / bars).clamp(MIN_SPACING, MAX_SPACING);
            self.right_offset = 0.0;
            self.manual_range = None;
        }
        self.handle_x_input(ui, &plot_resp, &time_resp, plot);
        self.clamp_offset(plot, n);

        // ---- bars to draw ---------------------------------------------------
        let mut ha = std::mem::take(&mut self.ha);
        let is_ha = self.chart_type == ChartType::HeikinAshi;
        if is_ha {
            ha.sync(series);
        }
        let display: &CandleSeries = if is_ha { ha.series() } else { series };

        // The volume pane takes the bottom of the plot; prices get the rest.
        let volume_rect = self.show_volume.then(|| {
            Rect::from_min_max(
                pos2(
                    plot.left(),
                    plot.bottom() - plot.height() * VOLUME_PANE_FRACTION,
                ),
                plot.max,
            )
        });
        let price_rect = match volume_rect {
            Some(vr) => Rect::from_min_max(plot.min, pos2(plot.right(), vr.top())),
            None => plot,
        };

        let range = visible_range_for(n, self.bar_spacing, self.right_offset, plot.width());
        let base = if range.is_empty() {
            1.0
        } else {
            display.closes()[range.start]
        };
        let mut vp = Viewport {
            bar_spacing: self.bar_spacing,
            right_offset_bars: self.right_offset,
            price_min: 0.0,
            price_max: 1.0,
            plot: PlotRect {
                left: price_rect.left(),
                top: price_rect.top(),
                right: price_rect.right(),
                bottom: price_rect.bottom(),
            },
            bar_count: n,
            scale: PriceScale::new(self.scale_mode, base),
        };
        // Bars narrower than ~2 px are drawn as per-pixel min/max columns.
        let ppp = ui.ctx().pixels_per_point();
        let decimated = self.bar_spacing * ppp < DECIMATE_BELOW_PX;
        let bars = if decimated {
            if self.lod_is_ha != is_ha {
                self.lod = Lod::new();
                self.lod_is_ha = is_ha;
            }
            self.lod.sync(display);
            self.lod.columns(display, range.clone(), &vp, ppp)
        } else {
            collect_bars(display, range.clone(), &vp)
        };

        // ---- vertical range -------------------------------------------------
        let close_only = matches!(self.chart_type, ChartType::Line | ChartType::Area);
        let auto = price_extent(&bars, close_only).map_or((0.0, 1.0), |(lo, hi)| {
            fit_price_range(lo, hi, vp.scale, Y_PADDING)
        });
        self.last_range = self.manual_range.unwrap_or(auto);
        self.handle_y_input(&plot_resp, &price_resp, price_rect, vp.scale);
        (vp.price_min, vp.price_max) = self.manual_range.unwrap_or(auto);

        let crosshair = plot_resp
            .hover_pos()
            .filter(|p| plot.contains(*p) && !plot_resp.dragged());

        let scene = Scene {
            vp,
            theme,
            style: &self.style,
            chart_type: self.chart_type,
            full,
            plot,
            volume_rect,
            price_axis,
            time_axis,
            display,
            last_raw: series.last(),
            bars: &bars,
            decimated,
            crosshair,
            precision,
            tick_size: data.tick_size,
            bar_interval_ms: data
                .bar_interval_ms
                .unwrap_or_else(|| bar_interval_ms(series)),
            now_ms: data.now_ms,
            bar_close_ms: data.bar_close_ms,
            title: data.title,
        };
        draw::draw(&mut canvas, &scene);
        canvas.finish();
        self.ha = ha;

        self.stats = ChartStats {
            visible_bars: range.len(),
            total_bars: n,
            draw_time: started.elapsed(),
        };

        price_resp.on_hover_cursor(CursorIcon::ResizeVertical);
        time_resp.on_hover_cursor(CursorIcon::ResizeHorizontal);
        let response = if plot_resp.dragged() {
            plot_resp.on_hover_cursor(CursorIcon::Grabbing)
        } else {
            plot_resp.on_hover_cursor(CursorIcon::Crosshair)
        };
        ChartOutput {
            response,
            needs_older_history: n > 0 && range.start < BACKFILL_MARGIN_BARS,
        }
    }

    /// Splits the widget into plot, price axis (right) and time axis (bottom).
    fn layout(
        &self,
        full: Rect,
        canvas: &mut dyn Canvas,
        series: &CandleSeries,
        precision: usize,
    ) -> (Rect, Rect, Rect) {
        // Size the axis for a label one digit wider than the latest price.
        let sample_price = series.highs().last().copied().unwrap_or(1.0).abs() * 10.0;
        let sample = match self.scale_mode {
            ScaleMode::Percent => "+1000.00%".to_owned(),
            _ => crate::scale::format_price(sample_price, precision),
        };
        let axis_w = (canvas.text_size(&sample, self.style.font_size).x + 22.0).max(64.0);
        let plot_right = full.right() - axis_w;
        let plot_bottom = full.bottom() - self.style.time_axis_height;
        let plot = Rect::from_min_max(full.min, pos2(plot_right, plot_bottom));
        let price_axis = Rect::from_min_max(
            pos2(plot_right, full.top()),
            pos2(full.right(), plot_bottom),
        );
        let time_axis = Rect::from_min_max(pos2(full.left(), plot_bottom), full.max);
        (plot, price_axis, time_axis)
    }

    fn handle_x_input(&mut self, ui: &Ui, plot_resp: &Response, time_resp: &Response, plot: Rect) {
        if plot_resp.double_clicked() {
            self.reset_view();
        }
        if plot_resp.dragged() {
            // Dragging right reveals older bars.
            self.right_offset -= plot_resp.drag_delta().x / self.bar_spacing;
        }
        if time_resp.dragged() {
            // Drag right to zoom in, anchored at the right edge like TradingView.
            let factor = (time_resp.drag_delta().x * 0.01).exp();
            self.zoom_at(factor, plot.right(), plot.right());
        }
        if plot_resp.hovered() || time_resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta(), i.zoom_delta()));
            if scroll.x != 0.0 {
                self.right_offset += scroll.x / self.bar_spacing;
            }
            // ~11% per wheel notch (egui reports ~20 points per notch).
            let factor = zoom * (scroll.y * WHEEL_ZOOM_PER_POINT).exp();
            if factor != 1.0 {
                let anchor_x = plot_resp
                    .hover_pos()
                    .filter(|p| plot.contains(*p))
                    .map_or(plot.right(), |p| p.x);
                self.zoom_at(factor, anchor_x, plot.right());
            }
        }
    }

    fn handle_y_input(
        &mut self,
        plot_resp: &Response,
        price_resp: &Response,
        plot: Rect,
        scale: PriceScale,
    ) {
        if price_resp.double_clicked() || plot_resp.double_clicked() {
            self.manual_range = None;
            return;
        }
        if price_resp.dragged() {
            let (lo, hi) = self.last_range;
            // Drag down expands the range (bars get flatter), drag up compresses it.
            let factor = (price_resp.drag_delta().y as f64 * 0.005).exp();
            self.manual_range = Some(zoom_price_range(lo, hi, scale, factor));
        }
        if let Some((lo, hi)) = self.manual_range
            && plot_resp.dragged()
        {
            let frac = plot_resp.drag_delta().y as f64 / plot.height().max(1.0) as f64;
            self.manual_range = Some(pan_price_range(lo, hi, scale, frac));
        }
    }

    /// Keeps at least a few bars on screen.
    fn clamp_offset(&mut self, plot: Rect, n: usize) {
        let visible_bars = plot.width() / self.bar_spacing;
        let max_offset = (visible_bars - 3.0).max(0.0);
        let min_offset = -((n as f32) - 3.0).max(0.0);
        self.right_offset = self.right_offset.clamp(min_offset, max_offset);
    }

    /// Zooms by `factor`, keeping the bar under `anchor_x` fixed on screen.
    fn zoom_at(&mut self, factor: f32, anchor_x: f32, plot_right: f32) {
        let new_spacing = (self.bar_spacing * factor).clamp(MIN_SPACING, MAX_SPACING);
        let dist = plot_right - anchor_x;
        self.right_offset += dist / new_spacing - dist / self.bar_spacing;
        self.bar_spacing = new_spacing;
    }
}

/// Bar interval inferred from the data (smallest gap among the last bars).
fn bar_interval_ms(series: &CandleSeries) -> i64 {
    let t = series.open_times();
    let tail = &t[t.len().saturating_sub(50)..];
    tail.windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| *d > 0)
        .min()
        .unwrap_or(60_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_anchor_bar_fixed() {
        let mut v = ChartView::default();
        let right = 1000.0;
        let anchor = 600.0;
        // Continuous index under the anchor: n-1+off+0.5 - (right-x)/spacing; n cancels out.
        let idx = |v: &ChartView| v.right_offset - (right - anchor) / v.bar_spacing;
        let before = idx(&v);
        v.zoom_at(1.7, anchor, right);
        assert!((idx(&v) - before).abs() < 1e-4);
        v.zoom_at(1000.0, anchor, right);
        assert_eq!(v.bar_spacing, MAX_SPACING);
        assert!((idx(&v) - before).abs() < 1e-3);
    }

    #[test]
    fn zoom_at_right_edge_keeps_offset() {
        let mut v = ChartView::default();
        let before = v.right_offset;
        v.zoom_at(2.0, 500.0, 500.0);
        assert_eq!(v.right_offset, before);
        assert_eq!(v.bar_spacing, DEFAULT_SPACING * 2.0);
    }
}
