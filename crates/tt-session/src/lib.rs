//! UI-independent application state shared by tradeterm front ends:
//! chart tabs with their live streams and backfill ([`tab`]), persisted
//! settings ([`settings`], written via [`fsutil`]), the background tokio runtime ([`runtime`]) and
//! synthetic stress-test data ([`synthetic`]).

pub mod fsutil;
pub mod runtime;
pub mod settings;
pub mod synthetic;
pub mod tab;
