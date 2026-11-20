use crate::bars::{collect_bars, price_extent};
use crate::derived::Derived;
use crate::draw::{self, LegendButton, LegendHit, Pane, Scene};
use crate::indicators::Indicator;
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
use tt_core::{CandleSeries, Placement};
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
/// Share of the price pane used by Volume overlays (TradingView: bottom quarter).
const VOLUME_OVERLAY_FRACTION: f32 = 0.25;
/// Relative height of the price pane; indicator panes default to 1.
const MAIN_PANE_WEIGHT: f32 = 3.0;
/// Panes can't be dragged smaller than this.
const MIN_PANE_HEIGHT: f32 = 40.0;
/// Padding above and below the values of an indicator pane.
const PANE_PADDING: f64 = 0.1;

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

/// Pointer-dependent state for [`ChartView::render`].
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderInput {
    /// Pointer position (crosshair) if over the plot.
    pub crosshair: Option<egui::Pos2>,
    /// Legend row under the pointer (indicator id; 0 = collapse button).
    pub legend_hover: Option<u64>,
}

/// What [`ChartView::render`] reports to the caller.
#[derive(Debug, Clone, Copy)]
pub struct RenderOutput {
    /// See [`ChartOutput::needs_older_history`].
    pub needs_older_history: bool,
}

/// Screen areas of the last frame.
#[derive(Debug, Clone)]
pub struct ChartLayout {
    /// All panes (left of the price axis, above the time axis).
    pub plot: Rect,
    pub price_axis: Rect,
    pub time_axis: Rect,
    /// Price pane first, then indicator panes.
    pub panes: Vec<Rect>,
}

impl Default for ChartLayout {
    fn default() -> Self {
        Self {
            plot: Rect::NOTHING,
            price_axis: Rect::NOTHING,
            time_axis: Rect::NOTHING,
            panes: Vec::new(),
        }
    }
}

/// Everything the last [`ChartView::render_base`] computed that the overlay
/// needs again.
#[derive(Debug, Clone)]
struct Frame {
    vp: Viewport,
    panes: Vec<Pane>,
    bars: Vec<crate::bars::DrawBar>,
    range: std::ops::Range<usize>,
    full: Rect,
    plot: Rect,
    volume_rect: Option<Rect>,
    price_axis: Rect,
    time_axis: Rect,
    decimated: bool,
    precision: usize,
    bar_interval_ms: i64,
}

/// What happened in [`ChartView::show`].
pub struct ChartOutput {
    /// Interaction response of the plot area.
    pub response: Response,
    /// The view is within [`BACKFILL_MARGIN_BARS`] of the oldest loaded bar;
    /// the caller should load older history (and prepend it) if available.
    pub needs_older_history: bool,
    /// The settings button (or a double-click) on an indicator's legend row
    /// was used; the caller shows its settings dialog.
    pub open_settings: Option<u64>,
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
    /// Indicators in draw order. Overlays draw on the price pane, Volume at
    /// its bottom, others each in their own pane below.
    pub indicators: Vec<Indicator>,
    /// Price-pane legend collapsed to a "⌄ N" button.
    pub legend_collapsed: bool,
    /// Relative height of the price pane vs. indicator panes.
    main_weight: f32,
    /// Legend buttons laid out last frame.
    legend_hits: Vec<LegendHit>,
    last_layout: ChartLayout,
    last_price_scale: PriceScale,
    /// Layout and bars of the last base render, reused by the overlay.
    frame: Option<Frame>,
    stats: ChartStats,
    bar_spacing: f32,
    right_offset: f32,
    /// `None` = auto-fit to visible bars; otherwise a manual (min, max) in prices.
    manual_range: Option<(f64, f64)>,
    /// Price range used last frame (for switching auto → manual seamlessly).
    last_range: (f64, f64),
    last_scale_mode: ScaleMode,
    /// Derived series (Heikin Ashi, Renko, Line break) for the chart type.
    derived: Derived,
    /// Min/max pyramid for decimated drawing, and whether it was built from HA bars.
    lod: Lod,
    lod_kind: Option<ChartType>,
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
            indicators: Vec::new(),
            legend_collapsed: false,
            main_weight: MAIN_PANE_WEIGHT,
            legend_hits: Vec::new(),
            last_layout: ChartLayout::default(),
            frame: None,
            last_price_scale: PriceScale::linear(),
            stats: ChartStats::default(),
            bar_spacing: DEFAULT_SPACING,
            right_offset: DEFAULT_RIGHT_OFFSET,
            manual_range: None,
            last_range: (0.0, 1.0),
            last_scale_mode: ScaleMode::default(),
            derived: Derived::default(),
            lod: Lod::new(),
            lod_kind: None,
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

    /// Indicator by runtime id.
    pub fn indicator_mut(&mut self, id: u64) -> Option<&mut Indicator> {
        self.indicators.iter_mut().find(|i| i.id == id)
    }

    /// `true` while the price axis auto-fits the visible bars.
    pub fn is_auto_scale(&self) -> bool {
        self.manual_range.is_none()
    }

    /// Scrolls by `dx` points (positive = drag right = reveal older bars).
    pub fn pan(&mut self, dx: f32) {
        self.right_offset -= dx / self.bar_spacing;
    }

    /// Zooms by `factor` around screen x `anchor_x` (`plot_right` = the
    /// plot's right edge); see [`ChartLayout::plot`].
    pub fn zoom(&mut self, factor: f32, anchor_x: f32, plot_right: f32) {
        self.zoom_at(factor, anchor_x, plot_right);
    }

    /// Where things were laid out by the last frame (for hit testing in
    /// backends that handle input themselves).
    pub fn last_layout(&self) -> &ChartLayout {
        &self.last_layout
    }

    /// egui front end: handles input on the widget, then [`Self::render`]s.
    pub fn show(&mut self, ui: &mut Ui, data: ChartData<'_>, theme: &Theme) -> ChartOutput {
        let series = data.series;
        let (full, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let mut canvas = EguiCanvas::new(ui.painter_at(full))
            .with_bold_family(self.style.bold_family.clone())
            .with_icon_family(self.style.icon_family.clone());

        let precision = precision_for(&data);
        let (plot, price_axis, time_axis) = self.layout(full, &mut canvas, series, precision);
        let pane_rects = self.pane_rects(plot);
        let price_rect = pane_rects[0];

        // ---- interaction --------------------------------------------------
        let id = ui.make_persistent_id("tt_chart");
        let plot_resp = ui.interact(plot, id.with("plot"), Sense::click_and_drag());
        let price_resp = ui.interact(
            Rect::from_x_y_ranges(price_axis.x_range(), price_rect.y_range()),
            id.with("price_axis"),
            Sense::click_and_drag(),
        );
        let time_resp = ui.interact(time_axis, id.with("time_axis"), Sense::click_and_drag());
        // Pane dividers (registered after the plot so they win the drag).
        for (k, pane) in pane_rects.iter().enumerate().skip(1) {
            let y = pane.top();
            let r = Rect::from_x_y_ranges(plot.left()..=price_axis.right(), y - 3.0..=y + 3.0);
            let resp = ui
                .interact(r, id.with(("pane_divider", k)), Sense::drag())
                .on_hover_cursor(CursorIcon::ResizeRow);
            if resp.dragged() {
                self.drag_divider(k, resp.drag_delta().y, plot.height());
            }
        }
        let (legend_hover, open_settings) = self.legend_input(ui, id);
        self.handle_x_input(ui, &plot_resp, &time_resp, plot);
        // Uses last frame's range and scale (one frame of lag is invisible).
        self.handle_y_input(&plot_resp, &price_resp, price_rect, self.last_price_scale);

        let crosshair = plot_resp
            .hover_pos()
            .filter(|p| plot.contains(*p) && !plot_resp.dragged());
        let out = self.render(
            &mut canvas,
            full,
            ui.ctx().pixels_per_point(),
            data,
            theme,
            RenderInput {
                crosshair,
                legend_hover,
            },
        );
        canvas.finish();

        price_resp.on_hover_cursor(CursorIcon::ResizeVertical);
        time_resp.on_hover_cursor(CursorIcon::ResizeHorizontal);
        let response = if plot_resp.dragged() {
            plot_resp.on_hover_cursor(CursorIcon::Grabbing)
        } else {
            plot_resp.on_hover_cursor(CursorIcon::Crosshair)
        };
        ChartOutput {
            response,
            needs_older_history: out.needs_older_history,
            open_settings,
        }
    }

    /// Backend-neutral frame: [`Self::render_base`] then
    /// [`Self::render_overlay`] into the same canvas.
    pub fn render(
        &mut self,
        canvas: &mut dyn Canvas,
        full: Rect,
        ppp: f32,
        data: ChartData<'_>,
        theme: &Theme,
        input: RenderInput,
    ) -> RenderOutput {
        let out = self.render_base(canvas, full, ppp, data, theme);
        self.render_overlay(canvas, data, theme, input);
        out
    }

    /// The pointer layer (crosshair, its axis tags, readout and legends)
    /// over the layout of the last [`Self::render_base`]. Cheap: backends
    /// that cache the base layer redraw only this while the mouse moves.
    pub fn render_overlay(
        &mut self,
        canvas: &mut dyn Canvas,
        data: ChartData<'_>,
        theme: &Theme,
        input: RenderInput,
    ) {
        if let Some(hits) = self.with_scene(data, theme, input, |s| draw::draw_overlay(canvas, s)) {
            self.legend_hits = hits;
        }
    }

    /// Builds the [`Scene`] for the stored frame and runs `f` on it.
    fn with_scene<R>(
        &mut self,
        data: ChartData<'_>,
        theme: &Theme,
        input: RenderInput,
        f: impl FnOnce(&Scene<'_>) -> R,
    ) -> Option<R> {
        let series = data.series;
        let mut derived = std::mem::take(&mut self.derived);
        let display: &CandleSeries = derived.sync(self.chart_type, series).unwrap_or(series);
        let out = self.frame.as_ref().map(|fr| {
            let scene = Scene {
                vp: fr.vp,
                theme,
                style: &self.style,
                chart_type: self.chart_type,
                full: fr.full,
                plot: fr.plot,
                volume_rect: fr.volume_rect,
                indicators: &self.indicators,
                panes: fr.panes.clone(),
                range: fr.range.clone(),
                legend_collapsed: self.legend_collapsed,
                legend_hover: input.legend_hover,
                price_axis: fr.price_axis,
                time_axis: fr.time_axis,
                display,
                last_raw: series.last(),
                bars: &fr.bars,
                decimated: fr.decimated,
                crosshair: input.crosshair.filter(|p| fr.plot.contains(*p)),
                precision: fr.precision,
                tick_size: data.tick_size,
                bar_interval_ms: fr.bar_interval_ms,
                now_ms: data.now_ms,
                bar_close_ms: data.bar_close_ms,
                title: data.title,
            };
            f(&scene)
        });
        self.derived = derived;
        out
    }

    /// The static layer: syncs caches, lays out panes and draws everything
    /// that doesn't follow the pointer into `canvas` (covering `full`).
    /// Input must already have been applied ([`Self::pan`], [`Self::zoom`],
    /// …). Redo it when data, viewport, size or settings change.
    pub fn render_base(
        &mut self,
        canvas: &mut dyn Canvas,
        full: Rect,
        ppp: f32,
        data: ChartData<'_>,
        theme: &Theme,
    ) -> RenderOutput {
        let started = Instant::now();
        let series = data.series;
        // Some chart types display a derived series (Heikin Ashi, Renko, Line
        // break) whose bar count can differ from the raw one.
        let mut derived = std::mem::take(&mut self.derived);
        let display: &CandleSeries = derived.sync(self.chart_type, series).unwrap_or(series);
        let n = display.len();
        if self.scale_mode != self.last_scale_mode {
            self.manual_range = None;
            self.last_scale_mode = self.scale_mode;
        }
        let precision = precision_for(&data);
        let (plot, price_axis, time_axis) = self.layout(full, canvas, series, precision);
        let pane_rects = self.pane_rects(plot);
        let price_rect = pane_rects[0];

        if let Some(bars) = self.pending_zoom
            && n > 0
        {
            self.pending_zoom = None;
            let bars = bars.min(n) as f32;
            self.bar_spacing = (plot.width() / bars).clamp(MIN_SPACING, MAX_SPACING);
            self.right_offset = 0.0;
            self.manual_range = None;
        }
        self.clamp_offset(plot, n);

        // ---- bars to draw ---------------------------------------------------
        let lod_kind = match self.chart_type {
            ChartType::HeikinAshi | ChartType::Renko | ChartType::LineBreak => {
                Some(self.chart_type)
            }
            _ => None,
        };
        for ind in &mut self.indicators {
            ind.sync(display);
        }
        // Volume overlays share the bottom of the price pane.
        let volume_rect = self
            .indicators
            .iter()
            .any(|i| i.visible && i.placement() == Placement::VolumeOverlay)
            .then(|| {
                Rect::from_min_max(
                    pos2(
                        price_rect.left(),
                        price_rect.bottom() - price_rect.height() * VOLUME_OVERLAY_FRACTION,
                    ),
                    price_rect.max,
                )
            });

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
        self.last_price_scale = vp.scale;
        // Bars narrower than ~2 px are drawn as per-pixel min/max columns.
        let decimated = self.bar_spacing * ppp < DECIMATE_BELOW_PX;
        let bars = if decimated {
            if self.lod_kind != lod_kind {
                self.lod = Lod::new();
                self.lod_kind = lod_kind;
            }
            self.lod.sync(display);
            self.lod.columns(display, range.clone(), &vp, ppp)
        } else {
            collect_bars(display, range.clone(), &vp)
        };

        // ---- vertical range -------------------------------------------------
        // Indicator lines are drawn with about one point per pixel column;
        // fit to the same samples.
        let stride = (1.0 / (self.bar_spacing * ppp)).ceil().max(1.0) as usize;
        let close_only = self.chart_type.uses_close_only();
        // Price-pane overlays (moving averages, bands…) take part in auto-fit.
        let mut extent = price_extent(&bars, close_only);
        for ind in self.indicators.iter().filter(|i| i.visible) {
            if ind.placement() == Placement::Overlay
                && let Some((lo, hi)) = ind.extent_sampled(range.clone(), stride)
            {
                extent = Some(extent.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
            }
        }
        let auto = extent.map_or((0.0, 1.0), |(lo, hi)| {
            fit_price_range(lo, hi, vp.scale, Y_PADDING)
        });
        self.last_range = self.manual_range.unwrap_or(auto);
        (vp.price_min, vp.price_max) = self.last_range;

        // Indicator panes: same x mapping, own auto-fit value range.
        let panes: Vec<Pane> = self
            .indicators
            .iter()
            .enumerate()
            .filter(|(_, i)| i.placement() == Placement::Pane)
            .zip(pane_rects.iter().skip(1))
            .map(|((index, ind), rect)| {
                let (min, max) = pane_range(ind, range.clone(), stride);
                Pane {
                    index,
                    vp: Viewport {
                        price_min: min,
                        price_max: max,
                        plot: PlotRect {
                            left: rect.left(),
                            top: rect.top(),
                            right: rect.right(),
                            bottom: rect.bottom(),
                        },
                        scale: PriceScale::linear(),
                        ..vp
                    },
                }
            })
            .collect();

        let frame = Frame {
            vp,
            panes,
            bars,
            range: range.clone(),
            full,
            plot,
            volume_rect,
            price_axis,
            time_axis,
            decimated,
            precision,
            bar_interval_ms: data
                .bar_interval_ms
                .unwrap_or_else(|| bar_interval_ms(series)),
        };
        self.derived = derived;
        self.frame = Some(frame);
        self.with_scene(data, theme, RenderInput::default(), |s| {
            draw::draw_base(canvas, s)
        });
        self.last_layout = ChartLayout {
            plot,
            price_axis,
            time_axis,
            panes: pane_rects,
        };
        self.stats = ChartStats {
            visible_bars: range.len(),
            total_bars: n,
            draw_time: started.elapsed(),
        };
        RenderOutput {
            needs_older_history: n > 0
                && range.start < BACKFILL_MARGIN_BARS
                // Brick charts compress many bars into few bricks, so the oldest
                // brick is usually on screen; only load more once the user has
                // scrolled back, or every view would page through all history.
                && (!matches!(self.chart_type, ChartType::Renko | ChartType::LineBreak)
                    || self.right_offset < 0.0),
        }
    }

    /// Price pane first, then one rect per indicator pane, split by weight.
    fn pane_rects(&self, plot: Rect) -> Vec<Rect> {
        let weights: Vec<f32> = std::iter::once(self.main_weight)
            .chain(
                self.indicators
                    .iter()
                    .filter(|i| i.placement() == Placement::Pane)
                    .map(|i| i.pane_weight),
            )
            .collect();
        let total: f32 = weights.iter().sum();
        let mut top = plot.top();
        weights
            .iter()
            .enumerate()
            .map(|(k, w)| {
                let bottom = if k + 1 == weights.len() {
                    plot.bottom()
                } else {
                    (top + plot.height() * w / total).round()
                };
                let r = Rect::from_x_y_ranges(plot.x_range(), top..=bottom);
                top = bottom;
                r
            })
            .collect()
    }

    /// Moves divider `k` (above pane `k`; 0 is the price pane) by `dy` points.
    fn drag_divider(&mut self, k: usize, dy: f32, plot_h: f32) {
        let mut weights: Vec<&mut f32> = std::iter::once(&mut self.main_weight)
            .chain(
                self.indicators
                    .iter_mut()
                    .filter(|i| i.placement() == Placement::Pane)
                    .map(|i| &mut i.pane_weight),
            )
            .collect();
        if k == 0 || k >= weights.len() || plot_h <= 0.0 {
            return;
        }
        let total: f32 = weights.iter().map(|w| **w).sum();
        let min = total * MIN_PANE_HEIGHT / plot_h;
        let (above, below) = (*weights[k - 1], *weights[k]);
        let dw = (dy / plot_h * total).clamp(min - above, below - min);
        if above + dw >= min && below - dw >= min {
            *weights[k - 1] = above + dw;
            *weights[k] = below - dw;
        }
    }

    /// Hit-tests last frame's legend layout: hover, and clicks on the eye /
    /// settings / remove / collapse buttons. Returns (hovered row id,
    /// indicator whose settings were requested).
    fn legend_input(&mut self, ui: &Ui, id: egui::Id) -> (Option<u64>, Option<u64>) {
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let mut hover = None;
        let mut settings = None;
        let hits = std::mem::take(&mut self.legend_hits);
        for hit in &hits {
            if pointer.is_some_and(|p| hit.row.contains(p)) {
                hover = Some(hit.id);
            }
            let row = ui.interact(hit.row, id.with(("legend_row", hit.id)), Sense::click());
            if hit.id != 0 && row.double_clicked() {
                settings = Some(hit.id);
            }
            for (k, (button, rect)) in hit.buttons.iter().enumerate() {
                let resp = ui.interact(*rect, id.with(("legend", hit.id, k)), Sense::click());
                if !resp.clicked() {
                    continue;
                }
                match button {
                    LegendButton::Collapse => self.legend_collapsed = !self.legend_collapsed,
                    LegendButton::ToggleVisible => {
                        if let Some(ind) = self.indicator_mut(hit.id) {
                            ind.visible = !ind.visible;
                        }
                    }
                    LegendButton::Settings => settings = Some(hit.id),
                    LegendButton::Remove => self.indicators.retain(|i| i.id != hit.id),
                }
            }
        }
        (hover, settings)
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

/// Value range of an indicator pane: the visible values (plus guide levels
/// for bounded oscillators) with some padding.
fn pane_range(ind: &Indicator, range: std::ops::Range<usize>, stride: usize) -> (f64, f64) {
    let mut ext = ind.extent_sampled(range, stride);
    if let Some((_, _, levels)) = ind.study.bounds()
        && let (Some(a), Some(b)) = (levels.first(), levels.last())
    {
        ext = Some(ext.map_or((*a, *b), |(lo, hi)| (lo.min(*a), hi.max(*b))));
    }
    let (lo, hi) = ext.unwrap_or((0.0, 1.0));
    let span = hi - lo;
    if span <= f64::EPSILON * hi.abs().max(1.0) {
        let pad = hi.abs().max(1.0) * 0.05;
        return (lo - pad, hi + pad);
    }
    (lo - span * PANE_PADDING, hi + span * PANE_PADDING)
}

/// Price label precision: from the tick size, else guessed from the price.
fn precision_for(data: &ChartData<'_>) -> usize {
    match data.tick_size {
        Some(t) => precision_from_tick_size(t),
        None => data.series.last().map_or(2, |c| price_decimals(c.close)),
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

    /// Records text only.
    #[derive(Default)]
    struct Texts(Vec<String>);

    impl Canvas for Texts {
        fn pixels_per_point(&self) -> f32 {
            1.0
        }
        fn set_clip(&mut self, _: Rect) {}
        fn fill_rect(&mut self, _: Rect, _: egui::Color32) {}
        fn fill_rounded_rect(&mut self, _: Rect, _: f32, _: egui::Color32) {}
        fn stroke_rect(&mut self, _: Rect, _: f32, _: egui::Color32) {}
        fn line(&mut self, _: egui::Pos2, _: egui::Pos2, _: f32, _: egui::Color32) {}
        fn dashed_line(
            &mut self,
            _: egui::Pos2,
            _: egui::Pos2,
            _: f32,
            _: egui::Color32,
            _: f32,
            _: f32,
        ) {
        }
        fn polyline(&mut self, _: &[egui::Pos2], _: f32, _: egui::Color32) {}
        fn fill_area(
            &mut self,
            _: &[egui::Pos2],
            _: f32,
            _: f32,
            _: egui::Color32,
            _: egui::Color32,
        ) {
        }
        fn fill_between(&mut self, _: &[egui::Pos2], _: &[egui::Pos2], _: egui::Color32) {}
        fn fill_circle(&mut self, _: egui::Pos2, _: f32, _: egui::Color32) {}
        fn text_styled(
            &mut self,
            pos: egui::Pos2,
            anchor: egui::Align2,
            text: &str,
            size: f32,
            _: egui::Color32,
            bold: bool,
        ) -> Rect {
            self.0.push(text.to_owned());
            anchor.anchor_size(pos, self.text_size_styled(text, size, bold))
        }
        fn text_size_styled(&mut self, text: &str, size: f32, _: bool) -> egui::Vec2 {
            egui::vec2(text.len() as f32 * size * 0.6, size)
        }
    }

    #[test]
    fn base_and_overlay_layers_split_pointer_state() {
        let mut series = CandleSeries::new();
        for i in 0..50 {
            series.upsert(tt_core::Candle {
                open_time_ms: i * 60_000,
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.5,
                volume: 1.0,
            });
        }
        let data = ChartData {
            series: &series,
            title: "T",
            tick_size: Some(0.01),
            now_ms: None,
            bar_interval_ms: Some(60_000),
            bar_close_ms: None,
        };
        let theme = Theme::dark();
        let full = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(900.0, 500.0));
        let mut v = ChartView::default();

        // Overlay before any base render: nothing to draw on.
        let mut c = Texts::default();
        v.render_overlay(&mut c, data, &theme, RenderInput::default());
        assert!(c.0.is_empty());

        let mut base = Texts::default();
        v.render_base(&mut base, full, 1.0, data, &theme);
        assert!(
            !base.0.iter().any(|t| t == "O" || t == "T"),
            "readout belongs to the overlay: {:?}",
            base.0
        );
        assert!(
            base.0.iter().any(|t| t == "100.50"),
            "last-price tag stays in the base"
        );

        // The overlay alone, twice (pointer moves), reusing the stored frame.
        for x in [300.0, 500.0] {
            let mut over = Texts::default();
            v.render_overlay(
                &mut over,
                data,
                &theme,
                RenderInput {
                    crosshair: Some(pos2(x, 200.0)),
                    legend_hover: None,
                },
            );
            assert!(over.0.iter().any(|t| t == "T") && over.0.iter().any(|t| t == "O"));
            assert!(
                over.0.iter().any(|t| t.contains(" '")),
                "crosshair time tag: {:?}",
                over.0
            );
        }
    }

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
