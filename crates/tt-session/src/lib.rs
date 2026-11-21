//! UI-independent application state shared by tradeterm front ends:
//! chart tabs with their live streams and backfill ([`tab`]), persisted
//! settings ([`settings`], written via [`fsutil`]), chart-settings templates ([`templates`]), the background tokio runtime ([`runtime`]) and
//! synthetic stress-test data ([`synthetic`]), asset names and logos
//! ([`assets`]).

pub mod assets;
pub mod fsutil;
pub mod indicator_templates;
pub mod layouts;
pub mod quotes;
pub mod runtime;
pub mod settings;
pub mod symbols;
pub mod synthetic;
pub mod tab;
pub mod templates;
pub mod tz;
pub mod watchlist;
