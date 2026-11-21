//! Runs the alert [`Evaluator`] on a task of the background runtime.
//!
//! Prices come from two places:
//! - chart tabs: the UI forwards a tab's last price after it drained new
//!   bars ([`AlertEngine::tab_price`]; a set lookup and a channel send when
//!   the symbol has alerts, nothing otherwise);
//! - for alert symbols no tab streams, the watchlist's quote feed
//!   (`BinanceSource::run_mini_tickers`, one combined `@miniTicker`
//!   connection), restarted only when that symbol set changes.
//!
//! The task also runs a 1 s clock (expirations, bar closes without
//! trades). Fired alerts come back through a channel plus the UI wake-up
//! ([`AlertEngine::drain`]).

use crate::alerts::{Alert, Evaluator, Fired};
use crate::tab::Wake;
use crossbeam_channel::Receiver;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Handle;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;
use tracing::{info, warn};
use tt_brokers::BinanceSource;
use tt_brokers::binance::Backoff;
use tt_core::Symbol;

enum Msg {
    Price { symbol: String, price: f64 },
    Alerts(Vec<Alert>),
    TabSymbols(HashSet<String>),
}

/// Unix milliseconds now.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// The watchlist's quote feed ([`crate::quotes`]: Binance's combined
/// `@miniTicker` stream, `BinanceSource::run_mini_tickers`) for the alert
/// symbols no tab streams; reconnects with backoff until the receiver is
/// gone.
async fn run_feed(source: Arc<BinanceSource>, symbols: Vec<Symbol>, tx: UnboundedSender<Msg>) {
    let mut backoff = Backoff::new(Duration::from_millis(500), Duration::from_secs(30));
    loop {
        let mut open = true;
        let r = source
            .run_mini_tickers(&symbols, |symbol, t| {
                open = tx
                    .send(Msg::Price {
                        symbol,
                        price: t.last,
                    })
                    .is_ok();
                backoff.reset();
                open
            })
            .await;
        if !open || tx.is_closed() {
            return;
        }
        let delay = backoff.next_delay();
        if let Err(e) = r {
            warn!(?delay, "alert feed disconnected: {e:#}");
        }
        tokio::time::sleep(delay).await;
    }
}

/// Handle to the alert task (UI side). Dropping it stops the task and its
/// feed.
pub struct AlertEngine {
    tx: UnboundedSender<Msg>,
    fired: Receiver<Fired>,
    /// Symbols with active alerts (tab prices for others are not sent).
    watched: HashSet<String>,
    tab_symbols: HashSet<String>,
}

impl AlertEngine {
    pub fn start(handle: &Handle, source: Arc<BinanceSource>, wake: Wake) -> Self {
        let (tx, rx) = unbounded_channel::<Msg>();
        let (fired_tx, fired) = crossbeam_channel::unbounded();
        let feed_tx = tx.clone();
        handle.spawn(async move {
            let mut rx = rx;
            let mut ev = Evaluator::default();
            let mut tab_symbols = HashSet::new();
            let mut feed: Option<(Vec<String>, JoinHandle<()>)> = None;
            let mut clock = tokio::time::interval(Duration::from_secs(1));
            clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut out = Vec::new();
            loop {
                let mut refeed = false;
                tokio::select! {
                    msg = rx.recv() => match msg {
                        None => break,
                        Some(Msg::Price { symbol, price }) => {
                            ev.on_tick(&symbol, price, now_ms(), &mut out);
                        }
                        Some(Msg::Alerts(a)) => {
                            ev.set_alerts(a);
                            refeed = true;
                        }
                        Some(Msg::TabSymbols(s)) => {
                            tab_symbols = s;
                            refeed = true;
                        }
                    },
                    _ = clock.tick() => ev.on_timer(now_ms(), &mut out),
                }
                if !out.is_empty() {
                    // A done alert may no longer need its feed.
                    refeed = true;
                    for f in out.drain(..) {
                        let _ = fired_tx.send(f);
                    }
                    wake();
                }
                if refeed {
                    let want: Vec<String> = ev
                        .symbols()
                        .into_iter()
                        .filter(|s| !tab_symbols.contains(s))
                        .collect();
                    if feed.as_ref().map(|(s, _)| s) != Some(&want) {
                        if let Some((_, task)) = feed.take() {
                            task.abort();
                        }
                        if !want.is_empty() {
                            info!(symbols = ?want, "alert feed");
                            let tx = feed_tx.clone();
                            let source = source.clone();
                            let symbols: Vec<Symbol> = want.iter().map(Symbol::new).collect();
                            let task = tokio::spawn(run_feed(source, symbols, tx));
                            feed = Some((want, task));
                        }
                    }
                }
            }
            if let Some((_, task)) = feed {
                task.abort();
            }
        });
        Self {
            tx,
            fired,
            watched: HashSet::new(),
            tab_symbols: HashSet::new(),
        }
    }

    /// The alert list changed (only active alerts are evaluated).
    pub fn set_alerts(&mut self, alerts: &[Alert]) {
        let active: Vec<Alert> = alerts
            .iter()
            .filter(|a| a.status.is_active())
            .cloned()
            .collect();
        self.watched = active.iter().map(|a| a.symbol.clone()).collect();
        let _ = self.tx.send(Msg::Alerts(active));
    }

    /// Symbols the chart tabs stream live (their prices come through
    /// [`Self::tab_price`]); sent only when the set changes.
    pub fn set_tab_symbols(&mut self, symbols: HashSet<String>) {
        if symbols != self.tab_symbols {
            self.tab_symbols = symbols.clone();
            let _ = self.tx.send(Msg::TabSymbols(symbols));
        }
    }

    /// A tab's last price (after new bars arrived). Cheap: a set lookup.
    pub fn tab_price(&self, symbol: &str, price: f64) {
        if self.watched.contains(symbol) {
            let _ = self.tx.send(Msg::Price {
                symbol: symbol.to_owned(),
                price,
            });
        }
    }

    /// Whether `symbol` has active alerts.
    pub fn watches(&self, symbol: &str) -> bool {
        self.watched.contains(symbol)
    }

    /// Events since the last call. Never blocks.
    pub fn drain(&self) -> Vec<Fired> {
        self.fired.try_iter().collect()
    }
}
