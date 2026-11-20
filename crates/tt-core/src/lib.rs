//! Core market-data types shared by every tradeterm crate.
//!
//! This crate deliberately has no UI and no network dependencies so it can be
//! used by data adapters, the chart widget, future order code and tests alike.

mod bricks;
mod event;
mod heikin_ashi;
mod resample;
mod series;
mod symbol_info;
mod timeframe;
mod types;

pub use bricks::{atr, line_break, renko, renko_box_size};
pub use event::{ConnectionStatus, EventSink, MarketEvent};
pub use heikin_ashi::{HeikinAshi, heikin_ashi, heikin_ashi_bar};
pub use resample::{ResampleError, Resampler, bucket_start, can_resample, resample};
pub use series::{CandleSeries, UpsertOutcome};
pub use symbol_info::{SymbolInfo, precision_from_tick_size};
pub use timeframe::{TimeUnit, Timeframe, civil_from_days, days_from_civil};
pub use types::{Candle, PriceTicks, Symbol};
