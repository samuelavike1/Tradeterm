//! Market-data adapters.
//!
//! Every adapter implements [`MarketDataSource`]: it streams [`tt_core::MarketEvent`]s
//! for one symbol/timeframe into an [`EventSink`] until the sink's receiver is
//! dropped or the task is cancelled. Adapters handle reconnects themselves.

pub mod binance;

pub use binance::BinanceSource;

use std::future::Future;
use tt_core::{EventSink, Symbol, Timeframe};

/// A source of candle data (exchange, broker, replay file, …).
pub trait MarketDataSource: Send + Sync {
    /// Short display name, e.g. `"Binance"`.
    fn name(&self) -> &'static str;

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
