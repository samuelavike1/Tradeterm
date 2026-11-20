//! One chart tab: its symbol, data stream, series and view state.
//!
//! Every tab keeps its own live stream, so background tabs stay current and
//! the tab strip can show their change %.

use crate::search::SymbolSearch;
use crate::settings::TabSettings;
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

/// Things every tab needs to start work: the UI context (for repaints),
/// the tokio runtime and the data source.
#[derive(Clone)]
pub struct Shared {
    pub ctx: egui::Context,
    pub handle: Handle,
    pub source: Arc<BinanceSource>,
    /// Semibold font family for emphasized chart labels, if available.
    pub bold_family: Option<egui::FontFamily>,
}

/// Loading of older history when the user scrolls back.
#[derive(Debug, Default)]
pub struct Backfill {
    pub loading: bool,
    /// The venue has no bars older than what we hold.
    pub exhausted: bool,
    retry_at: Option<Instant>,
}

pub struct ChartTab {
    /// Stable id for egui widget ids (survives reordering).
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
    pub search: SymbolSearch,

    pub series: CandleSeries,
    pub status: ConnectionStatus,
    pub backfill: Backfill,
    pub chart: ChartView,
}

impl ChartTab {
    pub fn new(id: u64, s: &TabSettings, synthetic: Option<usize>, shared: &Shared) -> Self {
        let mut symbol = Symbol::new(&s.symbol);
        if !symbol.is_valid() {
            symbol = Symbol::new(TabSettings::default().symbol);
        }
        let mut chart = ChartView::default();
        chart.style.bold_family = shared.bold_family.clone();
        chart.chart_type = s.chart_type;
        chart.scale_mode = s.scale_mode;
        chart.show_volume = s.show_volume;
        let (_, rx) = crossbeam_channel::unbounded();
        let mut tab = Self {
            id,
            stream: None,
            rx,
            sink: None,
            search: SymbolSearch::new(symbol.as_str()),
            symbol,
            symbol_info: None,
            synthetic,
            timeframe: s.timeframe,
            series: CandleSeries::new(),
            status: ConnectionStatus::Connecting,
            backfill: Backfill::default(),
            chart,
        };
        tab.restart(shared);
        tab
    }

    pub fn settings(&self) -> TabSettings {
        TabSettings {
            symbol: self.symbol.to_string(),
            timeframe: self.timeframe,
            chart_type: self.chart.chart_type,
            scale_mode: self.chart.scale_mode,
            show_volume: self.chart.show_volume,
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

    /// Base asset for the tab badge (falls back to the symbol's first letters).
    pub fn badge(&self) -> String {
        let base = self
            .symbol_info
            .as_ref()
            .map(|i| i.base.clone())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| self.display_symbol());
        base.chars().take(1).collect()
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
            self.symbol = symbol;
            self.synthetic = None;
            self.restart(shared);
        }
        self.search.input = self.symbol.to_string();
    }

    pub fn set_timeframe(&mut self, tf: Timeframe, shared: &Shared) {
        if tf != self.timeframe {
            self.timeframe = tf;
            self.restart(shared);
        }
    }

    /// Switches to `n` synthetic bars, or back to live data with `None`.
    pub fn set_synthetic(&mut self, n: Option<usize>, shared: &Shared) {
        self.synthetic = n;
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
        self.series.clear();
        self.status = ConnectionStatus::Connecting;
        self.backfill = Backfill::default();
        self.symbol_info = None;
        self.chart.reset_view();

        let ctx = shared.ctx.clone();
        let sink = EventSink::new(tx, move || ctx.request_repaint());

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
                MarketEvent::History(candles) => self.series.replace_tail(candles),
                MarketEvent::CandleUpdate { candle, .. } => {
                    self.series.upsert(candle);
                }
                MarketEvent::Status(s) => self.status = s,
                MarketEvent::SymbolInfo(info) => self.symbol_info = Some(info),
                MarketEvent::OlderHistory { candles, exhausted } => {
                    self.backfill.loading = false;
                    if candles.is_empty() && !exhausted {
                        self.backfill.retry_at = Some(Instant::now() + BACKFILL_RETRY);
                        shared.ctx.request_repaint_after(BACKFILL_RETRY);
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
