//! One chart tab: its symbol, data stream, series and view state.
//!
//! Every tab keeps its own live stream, so background tabs stay current and
//! the tab strip can show their change %.

use crate::settings::{IndicatorConfig, TabSettings};
use crate::synthetic;
use crossbeam_channel::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};
use tt_brokers::binance::MAX_KLINES;
use tt_brokers::{BinanceSource, MarketDataSource};
use tt_chart::ChartView;
use tt_core::{
    CandleSeries, ConnectionStatus, EventSink, MarketEvent, Symbol, SymbolInfo, Timeframe,
};

/// Backfill stops once this many bars are loaded, so zooming far out can't
/// page through years of 1m data.
pub const MAX_HISTORY_BARS: usize = 250_000;
/// Wait before retrying a failed backfill request.
const BACKFILL_RETRY: Duration = Duration::from_secs(5);
const DAY_MS: i64 = 86_400_000;

/// Wakes the UI thread (from any thread), e.g. to repaint after new data.
pub type Wake = Arc<dyn Fn() + Send + Sync>;
/// Wakes the UI thread after a delay.
pub type WakeAfter = Arc<dyn Fn(Duration) + Send + Sync>;

/// Things every tab needs to start work: a way to wake the UI, the tokio
/// runtime and the data source.
#[derive(Clone)]
pub struct Shared {
    pub wake: Wake,
    pub wake_after: WakeAfter,
    pub handle: Handle,
    pub source: Arc<BinanceSource>,
    /// The user default style: new charts start with it and tabs without
    /// their own style follow it.
    pub chart_style: tt_chart::ChartStyle,
    /// Drawings per symbol, shared by all tabs.
    pub drawings: DrawingStore,
    /// Asset names and logos (tab badges, search rows, status line).
    pub assets: crate::assets::AssetCache,
}

/// Drawings per symbol key (see [`ChartTab::drawings_key`]).
pub type DrawingStore = Arc<std::sync::Mutex<tt_chart::DrawingMap>>;

/// Loading of older history when the user scrolls back.
#[derive(Debug, Default)]
pub struct Backfill {
    pub loading: bool,
    /// The venue has no bars older than what we hold.
    pub exhausted: bool,
    retry_at: Option<Instant>,
}

/// Series this tab showed recently, so switching back is instant.
struct CachedSeries {
    symbol: Symbol,
    timeframe: Timeframe,
    series: CandleSeries,
    info: Option<SymbolInfo>,
    at: Instant,
}

/// Recently shown series kept per tab.
const CACHE_ENTRIES: usize = 6;
/// Cached bars older than this are not shown (history would leave a gap).
const CACHE_MAX_AGE: Duration = Duration::from_secs(10 * 60);

pub struct ChartTab {
    /// Stable id for UI element ids (survives reordering).
    pub id: u64,
    stream: Option<JoinHandle<()>>,
    rx: Receiver<MarketEvent>,
    /// Sink of the current stream, for side requests (symbol info, backfill).
    sink: Option<EventSink>,

    pub symbol: Symbol,
    pub symbol_info: Option<SymbolInfo>,
    /// `Some(n)`: showing `n` synthetic one-minute bars instead of live data.
    pub synthetic: Option<usize>,
    pub timeframe: Timeframe,

    pub series: CandleSeries,
    pub status: ConnectionStatus,
    pub backfill: Backfill,
    pub chart: ChartView,
    /// `chart.style` is this tab's own style; false = it follows the user
    /// default (`Shared::chart_style`), see [`TabSettings::style`].
    pub own_style: bool,
    /// Zone setting and exchange zone the chart's offset table was built
    /// for (see [`Self::sync_time_zone`]).
    tz_key: Option<(tt_chart::style::TimeZoneSetting, Option<String>)>,
    /// What `series` currently holds (live data), for the cache.
    loaded: Option<(Symbol, Timeframe)>,
    cache: Vec<CachedSeries>,
    /// When the current stream started, and whether first bars were logged.
    restarted_at: Instant,
    first_bars_logged: bool,
}

impl ChartTab {
    pub fn new(id: u64, s: &TabSettings, synthetic: Option<usize>, shared: &Shared) -> Self {
        let mut symbol = Symbol::new(&s.symbol);
        if !symbol.is_valid() {
            symbol = Symbol::new(TabSettings::default().symbol);
        }
        let mut chart = ChartView::default();
        chart.style = s.effective_style(&shared.chart_style).clone();
        chart.chart_type = s.chart_type;
        chart.scale_mode = s.scale_mode;
        chart.invert_scale = s.invert_scale;
        chart.main_collapsed = s.main_collapsed;
        chart.indicators = s.indicators.iter().map(|i| i.to_indicator()).collect();
        chart.legend_collapsed = s.legend_collapsed;
        chart.drawings.set_timeframe(Some(s.timeframe));
        let (_, rx) = crossbeam_channel::unbounded();
        let mut tab = Self {
            id,
            stream: None,
            rx,
            sink: None,
            symbol,
            symbol_info: None,
            synthetic,
            timeframe: s.timeframe,
            series: CandleSeries::new(),
            status: ConnectionStatus::Connecting,
            backfill: Backfill::default(),
            chart,
            own_style: s.style.is_some(),
            tz_key: None,
            loaded: None,
            cache: Vec::new(),
            restarted_at: Instant::now(),
            first_bars_logged: false,
        };
        tab.load_drawings(shared);
        tab.restart(shared);
        tab
    }

    /// Sets this tab's own style (`Some`) or makes it follow `default`
    /// (`None`).
    pub fn set_style(&mut self, own: Option<tt_chart::ChartStyle>, default: &tt_chart::ChartStyle) {
        self.own_style = own.is_some();
        self.chart.style = own.unwrap_or_else(|| default.clone());
    }

    /// Rebuilds the chart's time zone table when the style's zone or the
    /// exchange zone changed. Cheap when nothing changed (call per frame).
    pub fn sync_time_zone(&mut self) {
        let setting = &self.chart.style.symbol.data.time_zone;
        let exchange = self.symbol_info.as_ref().map(|i| i.timezone.as_str());
        if let Some((s, e)) = &self.tz_key
            && s == setting
            && e.as_deref() == exchange
        {
            return;
        }
        let tz = crate::tz::offsets_for(setting, exchange);
        self.tz_key = Some((setting.clone(), exchange.map(str::to_owned)));
        self.chart.set_time_zone(tz);
    }

    /// Key of this tab's drawings in the store.
    pub fn drawings_key(&self) -> String {
        self.display_symbol()
    }

    /// Writes changed drawings back to the shared store.
    pub fn stash_drawings(&mut self, shared: &Shared) {
        if self.chart.drawings.take_dirty() {
            let key = self.drawings_key();
            let items = self.chart.drawings.items.clone();
            if let Ok(mut store) = shared.drawings.lock() {
                // Keeps the drawings this build can't read (`unknown`).
                store.entry(key).or_default().items = items;
            }
        }
    }

    /// Shows the store's drawings for the current symbol (after stashing
    /// this tab's changes), e.g. when the tab becomes active again.
    pub fn load_drawings(&mut self, shared: &Shared) {
        self.stash_drawings(shared);
        let items = shared
            .drawings
            .lock()
            .ok()
            .and_then(|s| s.get(&self.drawings_key()).map(|l| l.items.clone()))
            .unwrap_or_default();
        self.chart.drawings.set_items(items);
    }

    pub fn settings(&self) -> TabSettings {
        TabSettings {
            symbol: self.symbol.to_string(),
            timeframe: self.timeframe,
            chart_type: self.chart.chart_type,
            scale_mode: self.chart.scale_mode,
            indicators: self
                .chart
                .indicators
                .iter()
                .map(IndicatorConfig::from_indicator)
                .collect(),
            legend_collapsed: self.chart.legend_collapsed,
            invert_scale: self.chart.invert_scale,
            main_collapsed: self.chart.main_collapsed,
            style: self.own_style.then(|| self.chart.style.clone()),
            show_volume: None,
        }
    }

    /// Name shown in the tab strip and chart title.
    pub fn display_symbol(&self) -> String {
        if self.synthetic.is_some() {
            "SYNTH".to_owned()
        } else {
            self.symbol.to_string()
        }
    }

    pub fn source_name(&self, shared: &Shared) -> &'static str {
        if self.synthetic.is_some() {
            "Synthetic"
        } else {
            shared.source.name()
        }
    }

    /// Base asset (e.g. `BTC`), or the symbol itself until metadata arrives.
    pub fn base_asset(&self) -> String {
        self.symbol_info
            .as_ref()
            .map(|i| i.base.clone())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| self.display_symbol())
    }

    /// Letter shown in the tab badge.
    pub fn badge(&self) -> String {
        self.base_asset().chars().take(1).collect()
    }

    pub fn price_precision(&self) -> usize {
        match &self.symbol_info {
            Some(i) => i.price_precision,
            None => self
                .series
                .last()
                .map_or(2, |c| tt_chart::scale::price_decimals(c.close)),
        }
    }

    /// Change of the last close vs. 24 hours earlier (vs. the previous bar on
    /// daily charts), in percent.
    pub fn change_pct(&self) -> Option<f64> {
        let last = self.series.last()?;
        let times = self.series.open_times();
        let reference = if self.timeframe.duration_ms() >= DAY_MS {
            self.series.len().checked_sub(2)?
        } else {
            let target = last.open_time_ms - DAY_MS;
            times.partition_point(|t| *t <= target).checked_sub(1)?
        };
        let base = self.series.closes()[reference];
        (base != 0.0).then(|| (last.close / base - 1.0) * 100.0)
    }

    pub fn set_symbol(&mut self, symbol: Symbol, shared: &Shared) {
        if symbol.is_valid() && (symbol != self.symbol || self.synthetic.is_some()) {
            self.stash_drawings(shared);
            self.symbol = symbol;
            self.synthetic = None;
            self.load_drawings(shared);
            self.restart(shared);
        }
    }

    pub fn set_timeframe(&mut self, tf: Timeframe, shared: &Shared) {
        if tf != self.timeframe {
            // Save chart left edge position when changing interval.
            let left = self
                .chart
                .style
                .scales
                .keep_left_edge
                .then(|| self.chart.left_edge_time(&self.series))
                .flatten();
            self.timeframe = tf;
            self.chart.drawings.set_timeframe(Some(tf));
            self.restart(shared);
            self.chart.keep_left_edge(left);
        }
    }

    /// Switches to `n` synthetic bars, or back to live data with `None`.
    pub fn set_synthetic(&mut self, n: Option<usize>, shared: &Shared) {
        self.stash_drawings(shared);
        self.synthetic = n;
        self.load_drawings(shared);
        self.restart(shared);
    }

    /// Cancels the current stream and starts one for the current settings.
    pub fn restart(&mut self, shared: &Shared) {
        if let Some(task) = self.stream.take() {
            task.abort();
        }
        // A fresh channel per stream: dropping the old receiver makes any
        // in-flight sends from old tasks fail, so stale data can't leak in.
        let (tx, rx) = crossbeam_channel::unbounded();
        self.rx = rx;
        self.status = ConnectionStatus::Connecting;
        self.backfill = Backfill::default();
        self.chart.reset_view();
        self.restarted_at = Instant::now();
        self.first_bars_logged = false;

        // Keep what we were showing; show the new key's bars right away if we
        // saw them recently (fresh history and the stream merge in).
        let previous = std::mem::take(&mut self.series);
        let previous_info = self.symbol_info.take();
        if let Some((symbol, timeframe)) = self.loaded.take()
            && !previous.is_empty()
        {
            self.cache
                .retain(|c| !(c.symbol == symbol && c.timeframe == timeframe));
            self.cache.push(CachedSeries {
                symbol,
                timeframe,
                series: previous,
                info: previous_info,
                at: Instant::now(),
            });
            if self.cache.len() > CACHE_ENTRIES {
                self.cache.remove(0);
            }
        }
        if self.synthetic.is_none()
            && let Some(i) = self.cache.iter().position(|c| {
                c.symbol == self.symbol
                    && c.timeframe == self.timeframe
                    && c.at.elapsed() < CACHE_MAX_AGE
            })
        {
            let cached = self.cache.remove(i);
            self.series = cached.series;
            self.symbol_info = cached.info;
            self.loaded = Some((self.symbol.clone(), self.timeframe));
            info!(symbol = %self.symbol, tf = %self.timeframe, bars = self.series.len(), "showing cached bars");
        }

        let wake = shared.wake.clone();
        let sink = EventSink::new(tx, move || wake());

        if let Some(n) = self.synthetic {
            self.sink = None;
            let tf = self.timeframe;
            info!(bars = n, %tf, "loading synthetic data");
            self.chart.fit_all();
            self.stream = Some(shared.handle.spawn_blocking(move || {
                let started = Instant::now();
                let candles = synthetic::generate(n, tf);
                info!(bars = candles.len(), elapsed = ?started.elapsed(), "synthetic data ready");
                sink.send(MarketEvent::SymbolInfo(synthetic::symbol_info()));
                sink.send(MarketEvent::History(candles));
                sink.send(MarketEvent::Status(ConnectionStatus::Connected));
            }));
            return;
        }

        self.sink = Some(sink.clone());
        let (symbol, tf) = (self.symbol.clone(), self.timeframe);
        info!(%symbol, %tf, source = shared.source.name(), "starting stream");

        // Metadata in parallel with the stream; the chart works without it.
        let source = shared.source.clone();
        let (info_sink, info_symbol) = (sink.clone(), symbol.clone());
        shared.handle.spawn(async move {
            match source.symbol_info(&info_symbol).await {
                Ok(info) => {
                    info_sink.send(MarketEvent::SymbolInfo(info));
                }
                Err(e) => warn!(symbol = %info_symbol, "symbol info unavailable: {e:#}"),
            }
        });

        let source = shared.source.clone();
        self.stream = Some(shared.handle.spawn(async move {
            if let Err(e) = source.run(symbol, tf, sink).await {
                error!("market data stream failed: {e:#}");
            }
        }));
    }

    /// Requests the page of bars before the oldest one we hold.
    pub fn request_backfill(&mut self, shared: &Shared) {
        let b = &self.backfill;
        let cooling = b.retry_at.is_some_and(|t| Instant::now() < t);
        if self.synthetic.is_some()
            || b.loading
            || b.exhausted
            || cooling
            || self.series.len() >= MAX_HISTORY_BARS
        {
            return;
        }
        let (Some(sink), Some(&oldest)) = (self.sink.clone(), self.series.open_times().first())
        else {
            return;
        };
        self.backfill.loading = true;
        let source = shared.source.clone();
        let (symbol, tf) = (self.symbol.clone(), self.timeframe);
        info!(%symbol, %tf, before = oldest, "loading older history");
        shared.handle.spawn(async move {
            let event = match source
                .fetch_history(&symbol, tf, Some(oldest), MAX_KLINES)
                .await
            {
                Ok(candles) => MarketEvent::OlderHistory {
                    exhausted: candles.len() < MAX_KLINES,
                    candles,
                },
                Err(e) => {
                    warn!("backfill failed: {e:#}");
                    MarketEvent::OlderHistory {
                        candles: Vec::new(),
                        exhausted: false,
                    }
                }
            };
            sink.send(event);
        });
    }

    /// Applies every pending event; returns how many there were. Never blocks.
    pub fn drain_events(&mut self, shared: &Shared) -> u32 {
        let mut count = 0;
        for event in self.rx.try_iter() {
            count += 1;
            match event {
                // Reconnects deliver recent history; keep backfilled bars before it.
                MarketEvent::History(candles) => {
                    self.series.replace_tail(candles);
                    if self.synthetic.is_none() {
                        self.loaded = Some((self.symbol.clone(), self.timeframe));
                    }
                    if !self.first_bars_logged {
                        self.first_bars_logged = true;
                        info!(
                            symbol = %self.symbol,
                            tf = %self.timeframe,
                            elapsed_ms = self.restarted_at.elapsed().as_millis() as u64,
                            "history on screen"
                        );
                    }
                }
                MarketEvent::CandleUpdate { candle, .. } => {
                    self.series.upsert(candle);
                }
                MarketEvent::Status(s) => self.status = s,
                MarketEvent::SymbolInfo(info) => self.symbol_info = Some(info),
                MarketEvent::OlderHistory { candles, exhausted } => {
                    self.backfill.loading = false;
                    if candles.is_empty() && !exhausted {
                        self.backfill.retry_at = Some(Instant::now() + BACKFILL_RETRY);
                        (shared.wake_after)(BACKFILL_RETRY);
                    } else {
                        let added = self.series.prepend(candles);
                        self.backfill.exhausted = exhausted || added == 0;
                    }
                }
            }
        }
        count
    }
}

impl Drop for ChartTab {
    fn drop(&mut self) {
        if let Some(task) = self.stream.take() {
            task.abort();
        }
    }
}
