//! Live 24 h quotes for the watchlist: one background task per symbol set
//! (a REST snapshot so the list fills at once, then Binance's combined
//! mini-ticker stream, reconnecting with backoff). Updates land in a shared
//! map and bump a generation counter; the UI reads the map when it renders
//! and is woken (coalesced) by `wake`. Nothing here blocks the UI thread.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use tt_brokers::BinanceSource;
use tt_brokers::binance::{Backoff, Ticker24h};
use tt_core::Symbol;

pub use tt_brokers::binance::Ticker24h as Quote;

struct Inner {
    quotes: Mutex<HashMap<String, Ticker24h>>,
    generation: AtomicU64,
}

/// Shared quote map plus the task feeding it.
pub struct QuoteBoard {
    inner: Arc<Inner>,
    handle: Handle,
    source: Arc<BinanceSource>,
    wake: Arc<dyn Fn() + Send + Sync>,
    symbols: Vec<String>,
    task: Option<JoinHandle<()>>,
}

impl QuoteBoard {
    pub fn new(
        handle: Handle,
        source: Arc<BinanceSource>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                quotes: Mutex::new(HashMap::new()),
                generation: AtomicU64::new(0),
            }),
            handle,
            source,
            wake,
            symbols: Vec::new(),
            task: None,
        }
    }

    /// Bumped on every update.
    pub fn generation(&self) -> u64 {
        self.inner.generation.load(Ordering::Relaxed)
    }

    /// The latest quote of `symbol`.
    pub fn get(&self, symbol: &str) -> Option<Ticker24h> {
        self.inner.quotes.lock().ok()?.get(symbol).copied()
    }

    /// Runs `f` with the quote map locked once (for a whole render).
    pub fn with<R>(&self, f: impl FnOnce(&HashMap<String, Ticker24h>) -> R) -> R {
        match self.inner.quotes.lock() {
            Ok(m) => f(&m),
            Err(e) => f(&e.into_inner()),
        }
    }

    /// Streams `symbols` (restarts the task only when the set changed).
    pub fn set_symbols(&mut self, mut symbols: Vec<String>) {
        symbols.sort();
        symbols.dedup();
        symbols.retain(|s| Symbol::new(s.as_str()).is_valid());
        if symbols == self.symbols && self.task.is_some() {
            return;
        }
        if let Some(t) = self.task.take() {
            t.abort();
        }
        self.symbols = symbols.clone();
        if symbols.is_empty() {
            return;
        }
        let (inner, source, wake) = (self.inner.clone(), self.source.clone(), self.wake.clone());
        self.task = Some(self.handle.spawn(async move {
            run(symbols, inner, source, wake).await;
        }));
    }
}

impl Drop for QuoteBoard {
    fn drop(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
        }
    }
}

async fn run(
    symbols: Vec<String>,
    inner: Arc<Inner>,
    source: Arc<BinanceSource>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    let syms: Vec<Symbol> = symbols.iter().map(|s| Symbol::new(s.as_str())).collect();
    let put = |s: String, t: Ticker24h| {
        if let Ok(mut m) = inner.quotes.lock() {
            m.insert(s, t);
        }
        inner.generation.fetch_add(1, Ordering::Relaxed);
    };
    let mut backoff = Backoff::new(Duration::from_millis(500), Duration::from_secs(30));
    info!(symbols = syms.len(), "watchlist quotes starting");
    loop {
        // A snapshot first (also after a reconnect: the stream only sends
        // symbols that change).
        match source.fetch_tickers(&syms).await {
            Ok(list) => {
                for (s, t) in list {
                    put(s, t);
                }
                backoff.reset();
                wake();
            }
            Err(e) => warn!("watchlist snapshot failed: {e:#}"),
        }
        let r = source
            .run_mini_tickers(&syms, |s, t| {
                put(s, t);
                wake();
                true
            })
            .await;
        if let Err(e) = r {
            let delay = backoff.next_delay();
            warn!(?delay, "watchlist quotes disconnected: {e:#}");
            tokio::time::sleep(delay).await;
        } else {
            return;
        }
    }
}
