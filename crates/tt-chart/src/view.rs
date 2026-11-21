use crate::bars::{collect_bars, price_extent, price_source};
use crate::derived::Derived;
use crate::draw::{self, LegendButton, LegendHit, Pane, Scene};
use crate::drawings::{DrawingMenuTarget, DrawingPress, Drawings, Mapper, Mods, Repaint};
use crate::indicators::Indicator;
use crate::lod::Lod;
use crate::render::Canvas;
use crate::scale::{precision_from_tick_size, price_decimals};
use crate::time_axis::SlotTimes;
use crate::viewport::{
    PlotRect, PriceScale, Viewport, fit_price_range_margins, pan_price_range, visible_range_for,
    zoom_price_range,
};
use crate::{ChartStyle, ChartType, ScaleMode};
use emath::{Pos2, Rect, Vec2, pos2};
use std::time::{Duration, Instant};
use tt_core::{CandleSeries, Placement};
use tt_theme::Theme;

const DEFAULT_SPACING: f32 = 8.0;
const DEFAULT_RIGHT_OFFSET: f32 = 5.0;
const MIN_SPACING: f32 = 0.0005;
const MAX_SPACING: f32 = 80.0;
/// Below this bar width (physical pixels) bars are drawn decimated.
const DECIMATE_BELOW_PX: f32 = 2.0;
/// Zoom sensitivity: scale factor exponent per scrolled point (~20 points
/// per wheel notch, so ~12% per notch).
pub const WHEEL_ZOOM_PER_POINT: f32 = 0.006;
/// Share of the price pane used by Volume overlays (TradingView: bottom quarter).
const VOLUME_OVERLAY_FRACTION: f32 = 0.25;
/// Relative height of the price pane; indicator panes default to 1.
const MAIN_PANE_WEIGHT: f32 = 3.0;
/// Panes can't be dragged smaller than this.
const MIN_PANE_HEIGHT: f32 = 40.0;
/// Height of a collapsed pane beyond the legend's font size.
const COLLAPSED_PANE_EXTRA: f32 = 14.0;
/// Padding above and below the values of an indicator pane.
const PANE_PADDING: f64 = 0.1;

/// Per-frame numbers for performance overlays.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ChartStats {
    pub visible_bars: usize,
    pub total_bars: usize,
    /// CPU time to lay out and record the chart's draw commands
    /// (excludes the backend's own frame work and GPU time).
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

/// Names of the charted symbol for price-scale labels and the watermark,
/// set by the app when the symbol or interval changes
/// ([`ChartView::set_symbol_labels`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolLabels {
    /// "BTCUSDT": the symbol's name label on the price scale.
    pub ticker: String,
    /// "1h".
    pub interval: String,
    /// Quote currency ("USDT"): the currency and unit label.
    pub currency: String,
    /// "BTC / USDT": the watermark's second line.
    pub description: String,
    /// "BTCUSDT, 1h" (built once, for the watermark).
    pub ticker_interval: String,
    /// Base asset code ("BTC"): the status line's logo, drawn as a letter
    /// badge when there is no logo image. Empty: no logo.
    pub logo_key: String,
    /// The base asset's logo image file, once cached.
    pub logo: Option<std::sync::Arc<std::path::Path>>,
}

/// What is under a point, from the last frame's layout
/// ([`ChartView::hit_test`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartHit {
    /// A button in a legend row (indicator id; 0 = the collapse button).
    LegendButton(u64, LegendButton),
    /// A legend row (indicator id): hovering it shows its buttons.
    LegendRow(u64),
    /// The divider above indicator pane `k` (1 = first pane).
    PaneDivider(usize),
    /// The price axis of the price pane: drag scales prices.
    PriceAxis,
    /// The time axis: drag zooms in time.
    TimeAxis,
    /// The plot: drag pans, wheel zooms.
    Plot,
    None,
}

/// Pointer-dependent state for [`ChartView::render`].
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderInput {
    /// Pointer position (crosshair) if over the plot.
    pub crosshair: Option<emath::Pos2>,
    /// Legend row under the pointer (indicator id; 0 = collapse button).
    pub legend_hover: Option<u64>,
    /// Legend button under the pointer (drawn with the `hover` fill).
    pub legend_hover_button: Option<(u64, LegendButton)>,
}

/// What [`ChartView::render`] reports to the caller.
#[derive(Debug, Clone, Copy)]
pub struct RenderOutput {
    /// The view is within [`BACKFILL_MARGIN_BARS`] of the oldest loaded bar;
    /// the caller should load older history (and prepend it) if available.
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
    /// Per pane: its id for the pane actions (0 = price pane, otherwise the
    /// indicator id).
    pub pane_ids: Vec<u64>,
    /// Per pane: collapsed to a strip.
    pub collapsed: Vec<bool>,
}

impl ChartLayout {
    /// Horizontal extent of the panes including the price axis (on either
    /// side), for pane hover and divider hit tests.
    pub fn panes_x_range(&self) -> emath::Rangef {
        emath::Rangef::new(
            self.plot.left().min(self.price_axis.left()),
            self.plot.right().max(self.price_axis.right()),
        )
    }

    /// Whether the price axis is left of the plot (Scales placement › Left).
    pub fn price_axis_left(&self) -> bool {
        self.price_axis.right() <= self.plot.left()
    }
}

impl Default for ChartLayout {
    fn default() -> Self {
        Self {
            plot: Rect::NOTHING,
            price_axis: Rect::NOTHING,
            time_axis: Rect::NOTHING,
            panes: Vec::new(),
            pane_ids: Vec::new(),
            collapsed: Vec::new(),
        }
    }
}

/// Everything the last [`ChartView::render_base`] computed that the overlay
/// needs again.
#[derive(Debug, Clone)]
struct Frame {
    vp: Viewport,
    main_collapsed: bool,
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

/// How close (in bars) to the oldest bar the view may get before
/// [`RenderOutput::needs_older_history`] is raised.
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
    /// Higher prices lower on screen ("Invert scale").
    pub invert_scale: bool,
    /// Indicators in draw order. Overlays draw on the price pane, Volume at
    /// its bottom, others each in their own pane below.
    pub indicators: Vec<Indicator>,
    /// Price-pane legend collapsed to a "⌄ N" button.
    pub legend_collapsed: bool,
    /// Drawings (trend lines, Fib…) and the drawing tool state.
    pub drawings: Drawings,
    /// Price pane collapsed to a strip.
    pub main_collapsed: bool,
    /// Pane filling the chart, the others collapsed (0 = price pane,
    /// otherwise an indicator id). Not saved.
    pub maximized: Option<u64>,
    /// The chart's interval, for the indicators' Visibility tab (`None`:
    /// unknown, every indicator is shown). Not saved.
    pub timeframe: Option<tt_core::Timeframe>,
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
    /// Time zone of the labels (built by the app, see [`Self::set_time_zone`]).
    tz: crate::TzOffsets,
    /// Time to put at the left edge once bars arrive (Save chart left edge
    /// position when changing interval).
    pending_left_edge: Option<i64>,
    /// `canvas.margins.right` the offset was last adjusted for, so a changed
    /// margin moves the view live.
    applied_right_margin: f32,
    /// Range button / Go to target, re-applied each frame until its bars
    /// are loaded (see [`Self::show_target`]).
    target: Option<crate::range::ViewTarget>,
    /// The range button whose span is on screen (highlighted).
    active_range: Option<crate::range::DateRange>,
    /// (spacing, offset) the target left; any other value means the user
    /// zoomed or scrolled, which drops the target and the highlight.
    target_view: Option<(f32, f32)>,
    /// Ticker, currency and description for labels and the watermark.
    labels: SymbolLabels,
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
            invert_scale: false,
            indicators: Vec::new(),
            legend_collapsed: false,
            drawings: Drawings::default(),
            main_collapsed: false,
            maximized: None,
            timeframe: None,
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
            pending_left_edge: None,
            applied_right_margin: DEFAULT_RIGHT_OFFSET,
            target: None,
            active_range: None,
            target_view: None,
            labels: SymbolLabels::default(),
            tz: crate::TzOffsets::utc(),
        }
    }

    /// Time zone of the time labels (axis, crosshair, drawings' dates).
    /// Bars keep their exchange alignment; only labels move.
    pub fn set_time_zone(&mut self, tz: crate::TzOffsets) {
        self.tz = tz;
    }

    pub fn time_zone(&self) -> &crate::TzOffsets {
        &self.tz
    }

    /// Sets the names shown by the symbol name label, the currency label
    /// and the watermark. Allocates only when something changed.
    pub fn set_symbol_labels(
        &mut self,
        ticker: &str,
        interval: &str,
        currency: &str,
        description: &str,
    ) {
        let l = &self.labels;
        if l.ticker == ticker
            && l.interval == interval
            && l.currency == currency
            && l.description == description
        {
            return;
        }
        let (logo_key, logo) = (
            std::mem::take(&mut self.labels.logo_key),
            self.labels.logo.take(),
        );
        self.labels = SymbolLabels {
            logo_key,
            logo,
            ticker: ticker.to_owned(),
            interval: interval.to_owned(),
            currency: currency.to_owned(),
            description: description.to_owned(),
            ticker_interval: if interval.is_empty() {
                ticker.to_owned()
            } else {
                format!("{ticker}, {interval}")
            },
        };
    }

    /// Sets the status line's logo: the base asset code (letter badge) and
    /// its image file once cached. Allocates only when something changed.
    pub fn set_symbol_logo(&mut self, key: &str, logo: Option<std::sync::Arc<std::path::Path>>) {
        if self.labels.logo_key != key {
            self.labels.logo_key = key.to_owned();
        }
        if self.labels.logo != logo {
            self.labels.logo = logo;
        }
    }

    pub fn symbol_labels(&self) -> &SymbolLabels {
        &self.labels
    }

    /// Open time at the left edge of the last frame (`None` before one).
    /// `series` is the raw data the chart was drawn from.
    pub fn left_edge_time(&self, series: &CandleSeries) -> Option<i64> {
        let fr = self.frame.as_ref()?;
        let slot = fr.vp.index_at(fr.vp.plot.left) as f64 + 0.5;
        let display = self.derived.series(self.chart_type).unwrap_or(series);
        SlotTimes::new(display.open_times(), fr.bar_interval_ms).time_at_frac(slot)
    }

    /// Times at the left and right edges of the last frame (`None` before
    /// one): the Go to dialog's Custom range defaults.
    pub fn visible_time_range(&self, series: &CandleSeries) -> Option<(i64, i64)> {
        let fr = self.frame.as_ref()?;
        let display = self.derived.series(self.chart_type).unwrap_or(series);
        let slots = SlotTimes::new(display.open_times(), fr.bar_interval_ms);
        let at = |x: f32| slots.time_at_frac(fr.vp.index_at(x) as f64 + 0.5);
        Some((at(fr.vp.plot.left)?, at(fr.vp.plot.right)?))
    }

    /// The slots of the last frame's x axis (`None` before one).
    fn frame_slots<'a>(&'a self, series: &'a CandleSeries) -> Option<SlotTimes<'a>> {
        let fr = self.frame.as_ref()?;
        let display = self.derived.series(self.chart_type).unwrap_or(series);
        Some(SlotTimes::new(display.open_times(), fr.bar_interval_ms))
    }

    /// TradingView's bar number of a drawing anchor's time (the settings
    /// dialog's Coordinates tab); see [`crate::drawings::form::bar_of_time`].
    pub fn bar_of_time(&self, series: &CandleSeries, time_ms: i64) -> Option<i64> {
        crate::drawings::form::bar_of_time(&self.frame_slots(series)?, time_ms)
    }

    /// The anchor time of bar number `bar` (inverse of
    /// [`Self::bar_of_time`]).
    pub fn time_of_bar(&self, series: &CandleSeries, bar: i64) -> Option<i64> {
        crate::drawings::form::time_of_bar(&self.frame_slots(series)?, bar)
    }

    /// Puts `time` at the left edge when the next bars arrive, keeping the
    /// zoom. Call after [`Self::reset_view`] on an interval change when
    /// `scales.keep_left_edge` is on.
    pub fn keep_left_edge(&mut self, time: Option<i64>) {
        self.pending_left_edge = time;
    }

    /// Canvas › Buttons › Navigation: when the zoom / scroll / reset overlay
    /// shows (drawn by the backend).
    pub fn nav_buttons(&self) -> crate::style::ButtonsVisibility {
        self.style.canvas.nav_buttons
    }

    /// Canvas › Buttons › Pane: when the pane buttons (move, collapse,
    /// maximize, remove) show (drawn by the backend).
    pub fn pane_buttons(&self) -> crate::style::ButtonsVisibility {
        self.style.canvas.pane_buttons
    }

    /// Statistics from the most recent [`Self::show`] call.
    pub fn stats(&self) -> ChartStats {
        self.stats
    }

    /// Default zoom, scrolled to the latest bar, auto-fit price axis.
    pub fn reset_view(&mut self) {
        self.drop_target();
        self.bar_spacing = DEFAULT_SPACING;
        self.right_offset = self.style.canvas.margins.right;
        self.applied_right_margin = self.style.canvas.margins.right;
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

    /// Where the last frame drew legend button `button` of row `id` (for
    /// the chart view's tooltip on canvas hit areas).
    pub fn legend_button_rect(&self, id: u64, button: LegendButton) -> Option<Rect> {
        self.legend_hits
            .iter()
            .filter(|h| h.id == id)
            .flat_map(|h| h.buttons.iter())
            .find(|(b, _)| *b == button)
            .map(|(_, r)| *r)
    }

    /// The bar under screen x in the last frame (the crosshair's bar), as
    /// an index into the chart's series. `None` outside the bars, over the
    /// axes, or for chart types whose bars are not the series' (Renko,
    /// line break).
    pub fn bar_at_x(&self, p: Pos2, series_len: usize) -> Option<usize> {
        let fr = self.frame.as_ref()?;
        if !fr.plot.x_range().contains(p.x) || fr.vp.bar_count != series_len {
            return None;
        }
        fr.vp.bar_at(p.x)
    }

    /// The bar slot under screen x in the last frame.
    pub fn slot_at_x(&self, x: f32) -> Option<i64> {
        self.frame.as_ref().map(|f| f.vp.slot_at(x))
    }

    /// Decimals of the price scale in the last frame.
    pub fn price_precision(&self) -> Option<usize> {
        self.frame.as_ref().map(|f| f.precision)
    }

    /// What is at `p` (screen point) in the last frame's layout.
    pub fn hit_test(&self, p: Pos2) -> ChartHit {
        for hit in &self.legend_hits {
            for (button, rect) in &hit.buttons {
                if rect.contains(p) {
                    return ChartHit::LegendButton(hit.id, *button);
                }
            }
        }
        for hit in &self.legend_hits {
            if hit.id != 0 && hit.row.contains(p) {
                return ChartHit::LegendRow(hit.id);
            }
        }
        let l = &self.last_layout;
        for (k, pane) in l.panes.iter().enumerate().skip(1) {
            // Dividers next to a collapsed pane don't resize.
            let fixed = |i: usize| l.collapsed.get(i).copied().unwrap_or(false);
            if !fixed(k - 1)
                && !fixed(k)
                && (p.y - pane.top()).abs() <= 3.0
                && l.panes_x_range().contains(p.x)
            {
                return ChartHit::PaneDivider(k);
            }
        }
        if let Some(main) = l.panes.first()
            && l.price_axis.x_range().contains(p.x)
            && main.y_range().contains(p.y)
        {
            return ChartHit::PriceAxis;
        }
        if l.time_axis.contains(p) {
            return ChartHit::TimeAxis;
        }
        if l.plot.contains(p) {
            return ChartHit::Plot;
        }
        ChartHit::None
    }

    /// Ids of the panes top to bottom: 0 for the price pane, then the
    /// indicator ids of the indicator panes.
    pub fn pane_ids(&self) -> Vec<u64> {
        std::iter::once(0)
            .chain(self.indicators.iter().filter(|i| i.in_pane()).map(|i| i.id))
            .collect()
    }

    /// Whether pane `id` (see [`Self::pane_ids`]) is drawn as a strip.
    pub fn is_pane_collapsed(&self, id: u64) -> bool {
        if let Some(m) = self.maximized
            && self.pane_ids().contains(&m)
        {
            return m != id;
        }
        if id == 0 {
            self.main_collapsed
        } else {
            self.indicators
                .iter()
                .any(|i| i.id == id && i.pane_collapsed)
        }
    }

    /// Collapses pane `id` to a strip, or restores it. Restoring from a
    /// maximized layout un-maximizes. The last open pane can't collapse.
    pub fn toggle_pane_collapsed(&mut self, id: u64) {
        if self.maximized.take().is_some() {
            return;
        }
        let ids = self.pane_ids();
        let open = ids.iter().filter(|&&p| !self.is_pane_collapsed(p)).count();
        let collapsing = !self.is_pane_collapsed(id);
        if collapsing && open <= 1 {
            return;
        }
        if id == 0 {
            self.main_collapsed = collapsing;
        } else if let Some(i) = self.indicator_mut(id) {
            i.pane_collapsed = collapsing;
        }
    }

    /// Lets pane `id` fill the chart (the others collapse), or restores.
    /// A collapsed pane that is maximized opens again.
    pub fn toggle_pane_maximized(&mut self, id: u64) {
        if self.maximized == Some(id) {
            self.maximized = None;
            return;
        }
        self.maximized = Some(id);
        if id == 0 {
            self.main_collapsed = false;
        } else if let Some(i) = self.indicator_mut(id) {
            i.pane_collapsed = false;
        }
    }

    /// Moves indicator pane `id` one pane up or down. The price pane stays
    /// on top. Returns whether it moved.
    pub fn move_pane(&mut self, id: u64, up: bool) -> bool {
        let slots: Vec<usize> = self
            .indicators
            .iter()
            .enumerate()
            .filter(|(_, i)| i.in_pane())
            .map(|(k, _)| k)
            .collect();
        let Some(j) = slots.iter().position(|&k| self.indicators[k].id == id) else {
            return false;
        };
        let other = if up { j.checked_sub(1) } else { Some(j + 1) };
        match other.and_then(|o| slots.get(o)) {
            Some(&o) => {
                self.indicators.swap(slots[j], o);
                true
            }
            None => false,
        }
    }

    /// Removes an indicator (and its pane).
    pub fn remove_indicator(&mut self, id: u64) {
        self.indicators.retain(|i| i.id != id);
        if self.maximized == Some(id) {
            self.maximized = None;
        }
    }

    /// Runs `f` with the drawings and the screen ↔ (time, price) mapping of
    /// the last frame (`None` before the first frame).
    fn with_mapper<R>(
        &mut self,
        data: ChartData<'_>,
        f: impl FnOnce(&mut Drawings, &Mapper<'_>, &CandleSeries) -> R,
    ) -> Option<R> {
        let (vp, interval) = self.frame.as_ref().map(|fr| (fr.vp, fr.bar_interval_ms))?;
        let mut derived = std::mem::take(&mut self.derived);
        derived.update(
            self.chart_type,
            &self.style.symbol,
            data.tick_size,
            data.series,
        );
        let display: &CandleSeries = derived.series(self.chart_type).unwrap_or(data.series);
        let m = Mapper {
            vp,
            slots: SlotTimes::new(display.open_times(), interval),
        };
        let r = f(&mut self.drawings, &m, display);
        self.derived = derived;
        Some(r)
    }

    /// Whether `p` is where drawings live (the open price pane).
    fn in_drawing_area(&self, p: Pos2) -> bool {
        !self.is_pane_collapsed(0)
            && self
                .last_layout
                .panes
                .first()
                .is_some_and(|r| r.contains(p))
    }

    /// A press on the plot: places a point of the active drawing tool, or
    /// selects / starts dragging the drawing under the pointer. `double`:
    /// the second press of a double-click (finishes open-ended drawings).
    pub fn drawing_press(
        &mut self,
        p: Pos2,
        mods: Mods,
        double: bool,
        data: ChartData<'_>,
    ) -> DrawingPress {
        if !self.in_drawing_area(p) {
            return DrawingPress::Ignored;
        }
        self.with_mapper(data, |d, m, s| d.press(p, mods, double, m, s))
            .unwrap_or(DrawingPress::Ignored)
    }

    /// Pointer motion for drawings (placing, dragging, hover).
    pub fn drawing_motion(&mut self, p: Pos2, mods: Mods, data: ChartData<'_>) -> Repaint {
        if !self.drawings.busy() && !self.in_drawing_area(p) {
            return match self.drawings.hovered.take() {
                Some(_) => Repaint::Overlay,
                None => Repaint::None,
            };
        }
        self.with_mapper(data, |d, m, s| d.motion(p, mods, m, s))
            .unwrap_or(Repaint::None)
    }

    /// Pointer released; returns whether drawings changed.
    pub fn drawing_release(&mut self, p: Pos2, mods: Mods) -> bool {
        self.drawings.release(p, mods)
    }

    /// A right-click: what the drawing context menu is for. A drawing not
    /// in the selection becomes the selection; while placing a path or
    /// polyline the click finishes it instead (no menu).
    pub fn drawing_context_press(&mut self, p: Pos2) -> DrawingMenuTarget {
        let ds = &mut self.drawings;
        if ds.placing() {
            return if ds.finish_placing() {
                DrawingMenuTarget::Finished
            } else {
                DrawingMenuTarget::None
            };
        }
        if ds.busy() || !self.in_drawing_area(p) {
            return DrawingMenuTarget::None;
        }
        let ds = &mut self.drawings;
        match ds.hit(p) {
            Some((id, _)) => {
                if !ds.is_selected(id) {
                    ds.select_only(id);
                }
                DrawingMenuTarget::Drawing(id)
            }
            None => DrawingMenuTarget::Empty,
        }
    }

    /// Arrow keys: nudges the selected drawings by `bars` bars and `ticks`
    /// price ticks (the symbol's tick, at least one screen point of price so
    /// a nudge is visible).
    pub fn nudge_drawings(&mut self, bars: i32, ticks: i32, data: ChartData<'_>) -> bool {
        let tick = data.tick_size.filter(|t| *t > 0.0);
        self.with_mapper(data, |d, m, _| {
            let point = (m.vp.price_at(0.0) - m.vp.price_at(1.0)).abs();
            // Whole ticks.
            let step = tick.map_or(point, |t| (point / t).ceil().max(1.0) * t);
            d.nudge(bars, ticks as f64 * step, m)
        })
        .unwrap_or(false)
    }

    /// Pastes copies of `items` (the clipboard) and selects them.
    pub fn paste_drawings(
        &mut self,
        items: &[crate::drawings::Drawing],
        data: ChartData<'_>,
    ) -> bool {
        self.with_mapper(data, |d, m, _| d.paste(items, m))
            .unwrap_or(false)
    }

    /// Clones the selected drawings (context menu "Clone").
    pub fn clone_selected_drawings(&mut self, data: ChartData<'_>) -> bool {
        self.with_mapper(data, |d, m, _| d.clone_selected(m))
            .unwrap_or(false)
    }

    /// Clicks a legend button; returns the indicator whose settings should
    /// open (the gear).
    pub fn press_legend(&mut self, id: u64, button: LegendButton) -> Option<u64> {
        match button {
            LegendButton::Collapse => self.legend_collapsed = !self.legend_collapsed,
            LegendButton::ToggleVisible => {
                if let Some(ind) = self.indicator_mut(id) {
                    ind.visible = !ind.visible;
                }
            }
            LegendButton::Settings => return Some(id),
            LegendButton::Remove => self.remove_indicator(id),
        }
        None
    }

    /// Drags the plot by `delta` points: pans in time, and in price while
    /// the price range is manual.
    pub fn drag_plot(&mut self, delta: Vec2) {
        self.pan(delta.x);
        if let (Some((lo, hi)), Some(main)) = (self.manual_range, self.last_layout.panes.first()) {
            let frac = delta.y as f64 / main.height().max(1.0) as f64;
            self.manual_range = Some(pan_price_range(lo, hi, self.last_price_scale, frac));
        }
    }

    /// Drags the price axis: down expands the range (bars get flatter), up
    /// compresses it. Switches the price scale to manual.
    pub fn drag_price_axis(&mut self, dy: f32) {
        let (lo, hi) = self.manual_range.unwrap_or(self.last_range);
        let factor = (dy as f64 * 0.005).exp();
        let range = zoom_price_range(lo, hi, self.last_price_scale, factor);
        self.manual_range = Some(range);
        self.last_range = range;
    }

    /// Drags the time axis: right zooms in, anchored at the right edge.
    pub fn drag_time_axis(&mut self, dx: f32) {
        let right = self.last_layout.plot.right();
        self.zoom_at((dx * 0.01).exp(), right, right);
    }

    /// Mouse wheel / trackpad over the chart: horizontal scroll pans,
    /// vertical zooms around `anchor_x` (~11% per wheel notch of ~20 points).
    pub fn wheel(&mut self, scroll: Vec2, anchor_x: f32) {
        if scroll.x != 0.0 {
            self.right_offset += scroll.x / self.bar_spacing;
        }
        let factor = (scroll.y * WHEEL_ZOOM_PER_POINT).exp();
        if factor != 1.0 {
            let plot = self.last_layout.plot;
            let x = if plot.x_range().contains(anchor_x) {
                anchor_x
            } else {
                plot.right()
            };
            self.zoom_at(factor, x, plot.right());
        }
    }

    /// Moves pane divider `k` (see [`ChartHit::PaneDivider`]) by `dy` points.
    pub fn drag_pane_divider(&mut self, k: usize, dy: f32) {
        let h = self.last_layout.plot.height();
        self.drag_divider(k, dy, h);
    }

    /// Back to auto-fitting the visible bars vertically.
    pub fn auto_scale(&mut self) {
        self.manual_range = None;
    }

    /// Stops auto-fitting: keeps the current price range ("A" off).
    pub fn freeze_scale(&mut self) {
        self.manual_range = Some(self.last_range);
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
        derived.update(self.chart_type, &self.style.symbol, data.tick_size, series);
        let display: &CandleSeries = derived.series(self.chart_type).unwrap_or(series);
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
                main_collapsed: fr.main_collapsed,
                legend_hover: input.legend_hover,
                legend_hover_button: input.legend_hover_button,
                price_axis: fr.price_axis,
                time_axis: fr.time_axis,
                display,
                projection_from: derived.projection_from(self.chart_type),
                last_raw: series.last(),
                raw: series,
                labels: &self.labels,
                time_fmt: crate::scale::TimeFormat::from_style(&self.style.scales),
                bars: &fr.bars,
                decimated: fr.decimated,
                crosshair: input.crosshair.filter(|p| fr.plot.contains(*p)),
                precision: fr.precision,
                tick_size: data.tick_size,
                bar_interval_ms: fr.bar_interval_ms,
                now_ms: data.now_ms,
                bar_close_ms: data.bar_close_ms,
                title: data.title,
                drawings: &self.drawings,
                tz: &self.tz,
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
        // Visibility tab: indicators outside their interval ranges are not
        // computed, drawn or given a pane.
        for ind in &mut self.indicators {
            ind.set_interval(self.timeframe);
        }
        // Some chart types display a derived series (Heikin Ashi, Renko, Line
        // break) whose bar count can differ from the raw one.
        let mut derived = std::mem::take(&mut self.derived);
        derived.update(self.chart_type, &self.style.symbol, data.tick_size, series);
        let display: &CandleSeries = derived.series(self.chart_type).unwrap_or(series);
        let n = display.len();
        if self.scale_mode != self.last_scale_mode {
            self.manual_range = None;
            self.last_scale_mode = self.scale_mode;
        }
        let precision = precision_for(&data);
        let (plot, price_axis, time_axis) = self.layout(full, canvas, series, precision);
        let pane_rects = self.pane_rects(plot);
        let price_rect = pane_rects[0];

        // Zoomed or scrolled since the target was applied: it is done.
        if self
            .target_view
            .is_some_and(|v| v != (self.bar_spacing, self.right_offset))
        {
            self.drop_target();
        }
        if let Some(bars) = self.pending_zoom
            && n > 0
        {
            self.pending_zoom = None;
            let bars = bars.min(n) as f32;
            self.bar_spacing = (plot.width() / bars).clamp(MIN_SPACING, MAX_SPACING);
            self.right_offset = 0.0;
            self.manual_range = None;
        }
        // Right margin edited in the settings: move the view with it.
        let margin = self.style.canvas.margins.right;
        if margin != self.applied_right_margin {
            self.right_offset += margin - self.applied_right_margin;
            self.applied_right_margin = margin;
        }
        let interval = data
            .bar_interval_ms
            .unwrap_or_else(|| bar_interval_ms(series));
        if n > 0
            && let Some(t) = self.pending_left_edge.take()
            && let Some(slot) = SlotTimes::new(display.open_times(), interval).slot_of(t)
        {
            // Slot `slot` at the left edge (not before the first bar).
            self.right_offset = slot.max(0.0) as f32 + plot.width() / self.bar_spacing - n as f32;
        }
        let target_loading = n > 0 && self.apply_target(display, interval, plot.width());
        self.clamp_offset(plot, n);
        if self.target.is_some() || self.active_range.is_some() {
            self.target_view = Some((self.bar_spacing, self.right_offset));
        }

        // ---- bars to draw ---------------------------------------------------
        let lod_kind = match self.chart_type {
            ChartType::HeikinAshi | ChartType::Renko | ChartType::LineBreak => {
                Some(self.chart_type)
            }
            _ => None,
        };
        for ind in self.indicators.iter_mut().filter(|i| !i.interval_hidden()) {
            ind.sync(display);
        }
        // Volume overlays share the bottom of the price pane.
        let main_collapsed = self.is_pane_collapsed(0);
        let volume_rect = self
            .indicators
            .iter()
            .any(|i| i.shown() && i.placement() == Placement::VolumeOverlay)
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
            scale: PriceScale {
                inverted: self.invert_scale,
                ..PriceScale::new(self.scale_mode, base)
            },
        };
        self.last_price_scale = vp.scale;
        // Bars narrower than ~2 px are drawn as per-pixel min/max columns.
        let decimated = self.bar_spacing * ppp < DECIMATE_BELOW_PX;
        let source = price_source(&self.style.symbol, self.chart_type);
        let bars = if decimated {
            if self.lod_kind != lod_kind {
                self.lod = Lod::new();
                self.lod_kind = lod_kind;
            }
            self.lod.sync(display, source);
            self.lod.columns(display, range.clone(), &vp, ppp)
        } else {
            collect_bars(display, range.clone(), &vp, source)
        };

        // ---- vertical range -------------------------------------------------
        // Indicator lines are drawn with about one point per pixel column;
        // fit to the same samples.
        let stride = (1.0 / (self.bar_spacing * ppp)).ceil().max(1.0) as usize;
        let source_only = self.chart_type.uses_source_only();
        // Price-pane overlays (moving averages, bands…) take part in auto-fit.
        let mut extent = price_extent(&bars, source_only);
        for ind in self.indicators.iter().filter(|i| i.shown()) {
            if ind.placement() == Placement::Overlay
                && let Some((lo, hi)) = ind.extent_sampled(range.clone(), stride)
            {
                extent = Some(extent.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
            }
        }
        let (top, bottom) = (
            (self.style.canvas.margins.top as f64 / 100.0).clamp(0.0, 0.45),
            (self.style.canvas.margins.bottom as f64 / 100.0).clamp(0.0, 0.45),
        );
        let auto = extent.map_or((0.0, 1.0), |(lo, hi)| {
            fit_price_range_margins(lo, hi, vp.scale, top, bottom)
        });
        self.last_range = self.manual_range.unwrap_or(auto);
        if let Some(ratio) = self.style.scales.lock_ratio {
            self.last_range = locked_range(
                self.last_range,
                ratio,
                price_rect.height(),
                self.bar_spacing,
                self.scale_mode,
            );
        }
        (vp.price_min, vp.price_max) = self.last_range;

        // Indicator panes: same x mapping, own auto-fit value range.
        let panes: Vec<Pane> = self
            .indicators
            .iter()
            .enumerate()
            .filter(|(_, i)| i.in_pane())
            .zip(pane_rects.iter().skip(1))
            .map(|((index, ind), rect)| {
                let (min, max) = pane_range(ind, range.clone(), stride);
                Pane {
                    index,
                    collapsed: self.is_pane_collapsed(ind.id),
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
            main_collapsed,
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
            bar_interval_ms: interval,
        };
        let slots = SlotTimes::new(display.open_times(), frame.bar_interval_ms);
        self.drawings.layout(
            &Mapper {
                vp: frame.vp,
                slots,
            },
            price_rect,
            plot,
        );
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
            pane_ids: self.pane_ids(),
            collapsed: self
                .pane_ids()
                .iter()
                .map(|&id| self.is_pane_collapsed(id))
                .collect(),
        };
        self.stats = ChartStats {
            visible_bars: range.len(),
            total_bars: n,
            draw_time: started.elapsed(),
        };
        RenderOutput {
            needs_older_history: target_loading
                || (n > 0
                    && range.start < BACKFILL_MARGIN_BARS
                // Brick charts compress many bars into few bricks, so the oldest
                // brick is usually on screen; only load more once the user has
                // scrolled back, or every view would page through all history.
                && (!matches!(self.chart_type, ChartType::Renko | ChartType::LineBreak)
                    || self.right_offset < 0.0)),
        }
    }

    /// Height of a collapsed pane strip.
    fn collapsed_height(&self) -> f32 {
        self.style.scales.text_size + COLLAPSED_PANE_EXTRA
    }

    /// Per pane: collapsed; plus the strip height and the height left for
    /// the open panes.
    fn pane_split(&self, plot_h: f32) -> (Vec<bool>, f32, f32) {
        let ids = self.pane_ids();
        let mut collapsed: Vec<bool> = ids.iter().map(|&id| self.is_pane_collapsed(id)).collect();
        if collapsed.iter().all(|&c| c) {
            collapsed[0] = false;
        }
        let strip = self.collapsed_height().min(plot_h / ids.len() as f32);
        let n = collapsed.iter().filter(|&&c| c).count() as f32;
        (collapsed, strip, (plot_h - strip * n).max(0.0))
    }

    /// Price pane first, then one rect per indicator pane: collapsed panes
    /// get a strip, the open ones split the rest by weight.
    fn pane_rects(&self, plot: Rect) -> Vec<Rect> {
        let weights: Vec<f32> = std::iter::once(self.main_weight)
            .chain(
                self.indicators
                    .iter()
                    .filter(|i| i.in_pane())
                    .map(|i| i.pane_weight),
            )
            .collect();
        let (collapsed, strip, free) = self.pane_split(plot.height());
        let total: f32 = weights
            .iter()
            .zip(&collapsed)
            .filter(|(_, c)| !**c)
            .map(|(w, _)| w)
            .sum();
        let mut top = plot.top();
        weights
            .iter()
            .enumerate()
            .map(|(k, w)| {
                let h = if collapsed[k] {
                    strip
                } else {
                    free * w / total
                };
                let bottom = if k + 1 == weights.len() {
                    plot.bottom()
                } else {
                    (top + h).round()
                };
                let r = Rect::from_x_y_ranges(plot.x_range(), top..=bottom);
                top = bottom;
                r
            })
            .collect()
    }

    /// Moves divider `k` (above pane `k`; 0 is the price pane) by `dy` points.
    fn drag_divider(&mut self, k: usize, dy: f32, plot_h: f32) {
        let (collapsed, _, free) = self.pane_split(plot_h);
        let mut weights: Vec<&mut f32> = std::iter::once(&mut self.main_weight)
            .chain(
                self.indicators
                    .iter_mut()
                    .filter(|i| i.in_pane())
                    .map(|i| &mut i.pane_weight),
            )
            .collect();
        if k == 0 || k >= weights.len() || free <= 0.0 || collapsed[k - 1] || collapsed[k] {
            return;
        }
        // Weights of the open panes share the height the strips leave.
        let total: f32 = weights
            .iter()
            .zip(&collapsed)
            .filter(|(_, c)| !**c)
            .map(|(w, _)| **w)
            .sum();
        let plot_h = free;
        let min = total * MIN_PANE_HEIGHT / plot_h;
        let (above, below) = (*weights[k - 1], *weights[k]);
        let dw = (dy / plot_h * total).clamp(min - above, below - min);
        if above + dw >= min && below - dw >= min {
            *weights[k - 1] = above + dw;
            *weights[k] = below - dw;
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
            ScaleMode::IndexedTo100 => "10000.00".to_owned(),
            _ => crate::scale::format_price(sample_price, precision),
        };
        let axis_w = (canvas.text_size(&sample, self.style.scales.text_size).x + 22.0).max(64.0);
        let plot_bottom = full.bottom() - self.style.time_axis_height;
        let time_axis = Rect::from_min_max(pos2(full.left(), plot_bottom), full.max);
        if self.style.scales.placement == crate::style::ScalePlacement::Left {
            let plot_left = full.left() + axis_w;
            let plot =
                Rect::from_min_max(pos2(plot_left, full.top()), pos2(full.right(), plot_bottom));
            let price_axis = Rect::from_min_max(full.min, pos2(plot_left, plot_bottom));
            return (plot, price_axis, time_axis);
        }
        // Auto = right (one price scale).
        let plot_right = full.right() - axis_w;
        let plot = Rect::from_min_max(full.min, pos2(plot_right, plot_bottom));
        let price_axis = Rect::from_min_max(
            pos2(plot_right, full.top()),
            pos2(full.right(), plot_bottom),
        );
        (plot, price_axis, time_axis)
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

/// Range buttons and Go to (docs/specs/bottom-panel.md).
impl ChartView {
    /// Scrolls / zooms to `target` on the next frames, re-applied as older
    /// history arrives (the render asks for it via
    /// [`RenderOutput::needs_older_history`]) until the target's start is
    /// loaded or [`Self::settle_target`] is called. `range` is the button
    /// to highlight until the user zooms, scrolls or changes the interval.
    pub fn show_target(
        &mut self,
        target: crate::range::ViewTarget,
        range: Option<crate::range::DateRange>,
    ) {
        self.target = Some(target);
        self.active_range = range;
        self.target_view = None;
        self.pending_zoom = None;
        self.pending_left_edge = None;
        self.manual_range = None;
    }

    /// The range button whose span is on screen.
    pub fn active_range(&self) -> Option<crate::range::DateRange> {
        self.active_range
    }

    /// A target is still waiting for older bars.
    pub fn target_pending(&self) -> bool {
        self.target.is_some()
    }

    /// No more history will come (the venue has none, or the bar cap is
    /// reached): keep the view as last applied.
    pub fn settle_target(&mut self) {
        self.target = None;
    }

    fn drop_target(&mut self) {
        self.target = None;
        self.active_range = None;
        self.target_view = None;
    }

    /// Applies the target to `display`; returns true while its start is
    /// older than the first loaded bar (more history needed).
    fn apply_target(&mut self, display: &CandleSeries, interval: i64, plot_w: f32) -> bool {
        use crate::range::{ViewTarget, center_offset, fit_span};
        let Some(target) = self.target else {
            return false;
        };
        let times = display.open_times();
        let (Some(&first), Some(&last)) = (times.first(), times.last()) else {
            return false;
        };
        let slots = SlotTimes::new(times, interval);
        let n = times.len();
        let end = last + interval;
        // (from, to) of a span, or the date to center.
        let (from, to) = match target {
            ViewTarget::Range(r) => (r.start(end, &self.tz), end),
            ViewTarget::Span { from, to } => (Some(from.min(to)), from.max(to)),
            ViewTarget::Date(t) => {
                let slot = slots.slot_of(t).unwrap_or(0.0).max(0.0);
                self.right_offset = center_offset(slot, n, plot_w, self.bar_spacing);
                self.manual_range = None;
                return self.target_needs_history(t < first);
            }
        };
        let slot_from = from.and_then(|f| slots.slot_of(f)).unwrap_or(0.0).max(0.0);
        let slot_to = slots.slot_of(to).unwrap_or(n as f64);
        let (spacing, offset) = fit_span(slot_from, slot_to, n, plot_w, MIN_SPACING, MAX_SPACING);
        self.bar_spacing = spacing;
        self.right_offset = offset;
        self.manual_range = None;
        // All: until the venue has nothing older.
        self.target_needs_history(from.is_none_or(|f| f < first))
    }

    /// Keeps the target while history is missing, else drops it (the
    /// highlight stays).
    fn target_needs_history(&mut self, missing: bool) -> bool {
        if !missing {
            self.target = None;
        }
        missing
    }
}

/// Lock price to bar ratio: the price range of a pane `height` points tall
/// at `ratio` price units per bar, centered on `range`. Linear scale only
/// (log and percent scales keep `range`).
fn locked_range(
    range: (f64, f64),
    ratio: f64,
    height: f32,
    bar_spacing: f32,
    mode: ScaleMode,
) -> (f64, f64) {
    if mode != ScaleMode::Linear || !(ratio > 0.0 && ratio.is_finite()) || bar_spacing <= 0.0 {
        return range;
    }
    let span = ratio * height as f64 / bar_spacing as f64;
    let mid = (range.0 + range.1) / 2.0;
    (mid - span / 2.0, mid + span / 2.0)
}

/// Value range of an indicator pane: the visible values (plus its visible
/// levels, e.g. RSI 30/70) with some padding.
fn pane_range(ind: &Indicator, range: std::ops::Range<usize>, stride: usize) -> (f64, f64) {
    let mut ext = ind.extent_sampled(range, stride);
    if let Some((a, b)) = ind.hline_extent() {
        ext = Some(ext.map_or((a, b), |(lo, hi)| (lo.min(a), hi.max(b))));
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
        fn fill_rect(&mut self, _: Rect, _: ecolor::Color32) {}
        fn fill_rounded_rect(&mut self, _: Rect, _: f32, _: ecolor::Color32) {}
        fn stroke_rect(&mut self, _: Rect, _: f32, _: ecolor::Color32) {}
        fn line(&mut self, _: emath::Pos2, _: emath::Pos2, _: f32, _: ecolor::Color32) {}
        fn dashed_line(
            &mut self,
            _: emath::Pos2,
            _: emath::Pos2,
            _: f32,
            _: ecolor::Color32,
            _: f32,
            _: f32,
        ) {
        }
        fn polyline(&mut self, _: &[emath::Pos2], _: f32, _: ecolor::Color32) {}
        fn fill_area(
            &mut self,
            _: &[emath::Pos2],
            _: f32,
            _: f32,
            _: ecolor::Color32,
            _: ecolor::Color32,
        ) {
        }
        fn fill_between(&mut self, _: &[emath::Pos2], _: &[emath::Pos2], _: ecolor::Color32) {}
        fn fill_circle(&mut self, _: emath::Pos2, _: f32, _: ecolor::Color32) {}
        fn text_styled(
            &mut self,
            pos: emath::Pos2,
            anchor: emath::Align2,
            text: &str,
            size: f32,
            _: ecolor::Color32,
            bold: bool,
        ) -> Rect {
            self.0.push(text.to_owned());
            anchor.anchor_size(pos, self.text_size_styled(text, size, bold))
        }
        fn text_size_styled(&mut self, text: &str, size: f32, _: bool) -> emath::Vec2 {
            emath::vec2(text.len() as f32 * size * 0.6, size)
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
        let full = Rect::from_min_size(pos2(0.0, 0.0), emath::vec2(900.0, 500.0));
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
                    ..Default::default()
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
    fn hit_test_and_input_api() {
        let mut series = CandleSeries::new();
        for i in 0..200 {
            let c = 100.0 + (i as f64 * 0.2).sin();
            series.upsert(tt_core::Candle {
                open_time_ms: i * 60_000,
                open: c,
                high: c + 1.0,
                low: c - 1.0,
                close: c,
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
        let full = Rect::from_min_size(pos2(0.0, 0.0), emath::vec2(900.0, 600.0));
        let mut v = ChartView {
            indicators: vec![
                Indicator::new(tt_core::StudyKind::Volume.default_study()),
                Indicator::new(tt_core::StudyKind::Rsi.default_study()),
            ],
            ..Default::default()
        };
        let rsi = v.indicators[1].id;
        v.render(
            &mut Texts::default(),
            full,
            1.0,
            data,
            &theme,
            RenderInput::default(),
        );

        let l = v.last_layout().clone();
        assert_eq!(v.hit_test(l.plot.center()), ChartHit::Plot);
        assert_eq!(
            v.hit_test(pos2(l.price_axis.center().x, l.panes[0].center().y)),
            ChartHit::PriceAxis
        );
        assert_eq!(v.hit_test(l.time_axis.center()), ChartHit::TimeAxis);
        assert_eq!(
            v.hit_test(pos2(l.plot.center().x, l.panes[1].top())),
            ChartHit::PaneDivider(1)
        );
        // The pane legend row (top left of the RSI pane).
        let row = pos2(l.panes[1].left() + 20.0, l.panes[1].top() + 12.0);
        assert_eq!(v.hit_test(row), ChartHit::LegendRow(rsi));

        // Hovering the row shows its buttons; the gear asks for settings and
        // the trash removes the indicator.
        v.render(
            &mut Texts::default(),
            full,
            1.0,
            data,
            &theme,
            RenderInput {
                crosshair: Some(row),
                legend_hover: Some(rsi),
                ..Default::default()
            },
        );
        let buttons: Vec<_> = v
            .legend_hits
            .iter()
            .find(|h| h.id == rsi)
            .unwrap()
            .buttons
            .clone();
        assert_eq!(buttons.len(), 3);
        let gear = buttons
            .iter()
            .find(|(b, _)| *b == LegendButton::Settings)
            .unwrap()
            .1;
        assert_eq!(
            v.hit_test(gear.center()),
            ChartHit::LegendButton(rsi, LegendButton::Settings)
        );
        assert_eq!(v.press_legend(rsi, LegendButton::Settings), Some(rsi));
        assert_eq!(v.press_legend(rsi, LegendButton::Remove), None);
        assert!(v.indicators.iter().all(|i| i.id != rsi));

        // Dragging the price axis switches to a manual range; auto_scale undoes it.
        assert!(v.is_auto_scale());
        v.drag_price_axis(40.0);
        assert!(!v.is_auto_scale());
        v.auto_scale();
        assert!(v.is_auto_scale());
    }

    #[test]
    fn pane_collapse_maximize_and_move() {
        let mut series = CandleSeries::new();
        for i in 0..100 {
            let c = 100.0 + (i as f64 * 0.3).sin();
            series.upsert(tt_core::Candle {
                open_time_ms: i * 60_000,
                open: c,
                high: c + 1.0,
                low: c - 1.0,
                close: c,
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
        let full = Rect::from_min_size(pos2(0.0, 0.0), emath::vec2(900.0, 600.0));
        let mut v = ChartView {
            indicators: vec![
                Indicator::new(tt_core::StudyKind::Rsi.default_study()),
                Indicator::new(tt_core::StudyKind::Macd.default_study()),
            ],
            ..Default::default()
        };
        let (rsi, macd) = (v.indicators[0].id, v.indicators[1].id);
        let draw = |v: &mut ChartView| {
            v.render(
                &mut Texts::default(),
                full,
                1.0,
                data,
                &theme,
                RenderInput::default(),
            );
            v.last_layout().clone()
        };
        let l = draw(&mut v);
        assert_eq!(l.pane_ids, vec![0, rsi, macd]);
        let strip = v.style.scales.text_size + COLLAPSED_PANE_EXTRA;

        // Collapse RSI: a strip; its divider no longer resizes.
        v.toggle_pane_collapsed(rsi);
        let l = draw(&mut v);
        assert!((l.panes[1].height() - strip).abs() < 1.0);
        assert_ne!(
            v.hit_test(pos2(l.plot.center().x, l.panes[1].top())),
            ChartHit::PaneDivider(1)
        );
        assert_eq!(
            v.hit_test(pos2(l.plot.center().x, l.panes[2].top())),
            ChartHit::Plot
        );
        v.toggle_pane_collapsed(rsi);
        assert!(!v.is_pane_collapsed(rsi));

        // The last open pane can't collapse.
        v.toggle_pane_collapsed(0);
        v.toggle_pane_collapsed(rsi);
        v.toggle_pane_collapsed(macd);
        assert!(!v.is_pane_collapsed(macd));
        v.toggle_pane_collapsed(0);
        v.toggle_pane_collapsed(rsi);

        // Maximize MACD: the others become strips; again restores.
        v.toggle_pane_maximized(macd);
        let l = draw(&mut v);
        assert_eq!(l.collapsed, vec![true, true, false]);
        assert!(l.panes[2].height() > 400.0);
        v.toggle_pane_maximized(macd);
        assert_eq!(draw(&mut v).collapsed, vec![false, false, false]);
        // Maximizing a collapsed pane opens it for good.
        v.toggle_pane_collapsed(rsi);
        v.toggle_pane_maximized(rsi);
        v.toggle_pane_maximized(rsi);
        assert!(!v.is_pane_collapsed(rsi));

        // Move MACD up; RSI can't move above the price pane.
        assert!(v.move_pane(macd, true));
        assert_eq!(v.pane_ids(), vec![0, macd, rsi]);
        assert!(!v.move_pane(macd, true));
        assert!(!v.move_pane(rsi, false));
        v.remove_indicator(macd);
        assert_eq!(v.pane_ids(), vec![0, rsi]);
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

    #[test]
    fn indicators_outside_their_intervals_are_not_computed_drawn_or_given_a_pane() {
        use tt_core::{StudyKind, TimeUnit, Timeframe};
        let mut series = CandleSeries::new();
        for i in 0..100 {
            let c = 100.0 + (i as f64 * 0.3).sin();
            series.upsert(tt_core::Candle {
                open_time_ms: i * 60_000,
                open: c,
                high: c + 1.0,
                low: c - 1.0,
                close: c,
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
        let full = Rect::from_min_size(pos2(0.0, 0.0), emath::vec2(900.0, 600.0));
        let mut v = ChartView::default();
        let mut rsi = Indicator::new(StudyKind::Rsi.default_study());
        rsi.style.visibility.set(TimeUnit::Minute, Some((5, 59)));
        let mut sma = Indicator::new(StudyKind::Sma.default_study());
        sma.style.visibility.set(TimeUnit::Minute, None);
        v.indicators = vec![sma, rsi];

        // 1 minute: both are outside their ranges.
        v.timeframe = Some(Timeframe::M1);
        v.render_base(&mut Texts::default(), full, 1.0, data, &theme);
        assert_eq!(v.last_layout().panes.len(), 1, "the RSI pane is gone");
        assert!(v.indicators.iter().all(|i| !i.shown() && i.visible));
        assert!(
            v.indicators.iter().all(|i| i.outputs().is_empty()),
            "not computed"
        );
        let mut over = Texts::default();
        v.render_overlay(&mut over, data, &theme, RenderInput::default());
        assert!(!over.0.iter().any(|t| t.starts_with("RSI")), "{:?}", over.0);

        // 5 minutes: RSI is back with its pane; the SMA stays hidden.
        v.timeframe = Some(Timeframe::M5);
        v.render_base(&mut Texts::default(), full, 1.0, data, &theme);
        assert_eq!(v.last_layout().panes.len(), 2);
        assert!(v.indicators[1].shown() && !v.indicators[1].outputs().is_empty());
        assert!(!v.indicators[0].shown());

        // Unknown interval: everything is shown.
        v.timeframe = None;
        v.render_base(&mut Texts::default(), full, 1.0, data, &theme);
        assert!(v.indicators.iter().all(|i| i.shown()));
    }

    #[test]
    fn range_target_fits_waits_for_history_and_clears_on_pan() {
        use crate::range::{DateRange, ViewTarget};
        const DAY: i64 = 86_400_000;
        // Daily bars for 2026-01-01 + 300 days.
        let start = tt_core::days_from_civil(2026, 1, 1) * DAY;
        let bars = |from: i64, n: i64| {
            let mut s = CandleSeries::new();
            for i in 0..n {
                s.upsert(tt_core::Candle {
                    open_time_ms: start + (from + i) * DAY,
                    open: 100.0,
                    high: 101.0,
                    low: 99.0,
                    close: 100.0,
                    volume: 1.0,
                });
            }
            s
        };
        let theme = Theme::dark();
        let full = Rect::from_min_size(pos2(0.0, 0.0), emath::vec2(900.0, 600.0));
        let render = |v: &mut ChartView, s: &CandleSeries| {
            let data = ChartData {
                series: s,
                title: "T",
                tick_size: Some(0.01),
                now_ms: None,
                bar_interval_ms: Some(DAY),
                bar_close_ms: None,
            };
            v.render_base(&mut Texts::default(), full, 1.0, data, &theme)
        };
        let all = bars(0, 300);
        let mut v = ChartView::default();
        render(&mut v, &all);

        // YTD: 1 January to the newest close fills the plot.
        v.show_target(ViewTarget::Range(DateRange::Ytd), Some(DateRange::Ytd));
        render(&mut v, &all);
        let w = v.last_layout().plot.width();
        assert!((v.bar_spacing - w / 300.0).abs() < 1e-3);
        assert_eq!(v.right_offset, 0.0);
        assert!(!v.target_pending(), "all of it is loaded");
        assert_eq!(v.active_range(), Some(DateRange::Ytd));
        // Live: a new bar keeps the highlight.
        render(&mut v, &bars(0, 301));
        assert_eq!(v.active_range(), Some(DateRange::Ytd));

        // Only the last 100 days loaded: fit what is there, ask for more.
        let recent = bars(200, 100);
        v.show_target(ViewTarget::Range(DateRange::Ytd), Some(DateRange::Ytd));
        let out = render(&mut v, &recent);
        assert!(out.needs_older_history && v.target_pending());
        // The older page arrives: re-fitted to the full span.
        render(&mut v, &all);
        assert!(!v.target_pending());
        assert!((v.bar_spacing - w / 300.0).abs() < 1e-3);

        // Scrolling drops the highlight.
        v.pan(40.0);
        render(&mut v, &all);
        assert_eq!(v.active_range(), None);

        // Go to a date: that bar in the middle, zoom kept.
        let spacing = v.bar_spacing;
        v.show_target(ViewTarget::Date(start + 150 * DAY + DAY / 2), None);
        render(&mut v, &all);
        assert_eq!(v.bar_spacing, spacing);
        let mid = (v.last_layout().plot.left() + v.last_layout().plot.right()) / 2.0;
        let frame = v.frame.as_ref().unwrap();
        assert!((frame.vp.index_at(mid) - 150.0).abs() < 0.6);
    }
}
