//! Market-data adapters.
//!
//! Every adapter implements [`MarketDataSource`]: metadata lookups, paged
//! history, and a live stream of [`tt_core::MarketEvent`]s for one
//! symbol/timeframe pushed into an [`EventSink`] until the sink's receiver is
//! dropped or the task is cancelled. Adapters handle reconnects themselves.

pub mod binance;

pub use binance::BinanceSource;

use std::future::Future;
use tt_core::{Candle, EventSink, Symbol, SymbolInfo, Timeframe};

/// A source of candle data (exchange, broker, replay file, …).
pub trait MarketDataSource: Send + Sync {
    /// Short display name, e.g. `"Binance"`.
    fn name(&self) -> &'static str;

    /// Instrument metadata (tick size, assets, timezone).
    fn symbol_info(
        &self,
        symbol: &Symbol,
    ) -> impl Future<Output = anyhow::Result<SymbolInfo>> + Send;

    /// Instruments matching `query`, best matches first.
    fn search_symbols(
        &self,
        query: &str,
    ) -> impl Future<Output = anyhow::Result<Vec<SymbolInfo>>> + Send;

    /// Up to `limit` bars, oldest first. With `before = Some(t)` only bars
    /// that opened strictly before `t` are returned (for backfilling);
    /// with `None` the most recent bars.
    fn fetch_history(
        &self,
        symbol: &Symbol,
        timeframe: Timeframe,
        before: Option<i64>,
        limit: usize,
    ) -> impl Future<Output = anyhow::Result<Vec<Candle>>> + Send;

    /// Streams history and live candles for `symbol`/`timeframe` into `sink`.
    ///
    /// Runs until the sink is closed (returns `Ok`) or the task is aborted.
    /// Transient network errors are retried internally and reported as
    /// [`tt_core::ConnectionStatus`] events rather than returned.
    fn run(
        &self,
        symbol: Symbol,
        timeframe: Timeframe,
        sink: EventSink,
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
}
