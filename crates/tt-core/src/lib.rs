//! Core market-data types shared by every tradeterm crate.
//!
//! This crate deliberately has no UI and no network dependencies so it can be
//! used by data adapters, the chart widget, future order code and tests alike.

mod event;
mod heikin_ashi;
mod series;
mod types;

pub use event::{ConnectionStatus, EventSink, MarketEvent};
pub use heikin_ashi::{HeikinAshi, heikin_ashi, heikin_ashi_bar};
pub use series::{CandleSeries, UpsertOutcome};
pub use types::{Candle, PriceTicks, Symbol, Timeframe};
