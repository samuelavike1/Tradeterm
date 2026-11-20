//! Core market-data types shared by every tradeterm crate.
//!
//! This crate deliberately has no UI and no network dependencies so it can be
//! used by data adapters, the chart widget, future order code and tests alike.

mod event;
mod series;
mod types;

pub use event::{ConnectionStatus, EventSink, MarketEvent};
pub use series::{CandleSeries, UpsertOutcome};
pub use types::{Candle, PriceTicks, Symbol, Timeframe};
