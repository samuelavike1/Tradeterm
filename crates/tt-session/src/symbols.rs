//! The exchange's symbol list for the symbol search, cached on disk.
//!
//! Binance's full `exchangeInfo` is ~6 MB and takes 8–45 s to download, so
//! the first search used to wait that long. Now the list lives in
//! `symbols.json` in the app's data directory (next to the settings file,
//! like `assets.json`): it is read at startup on the runtime, installed into
//! the [`BinanceSource`] (searches and symbol lookups then use it), and
//! re-downloaded in the background at most once per [`REFRESH_EVERY`].
//! Without a cache the first launch downloads it right away, still before
//! the user opens the search.
//!
//! Searches ([`SymbolList::search`]) wait for the list to be loaded (from
//! disk: milliseconds) and rank it in memory.

use crate::assets::{due_in, now_ms};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::{Notify, watch};
use tracing::{debug, info, warn};
use tt_brokers::BinanceSource;
use tt_brokers::binance::rank_symbols;
use tt_core::{Symbol, SymbolInfo, precision_from_tick_size};

pub const SYMBOLS_FILE: &str = "symbols.json";
/// The list is re-downloaded at most this often.
pub const REFRESH_EVERY: Duration = crate::assets::REFRESH_EVERY;
/// After a failed refresh with a list in hand, try again this much later.
const RETRY_AFTER: Duration = Duration::from_secs(60 * 60);
/// After a failed download with no list at all.
const RETRY_EMPTY: Duration = Duration::from_secs(30);
/// Search results returned.
pub const SEARCH_LIMIT: usize = 100;

/// `symbols.json`: compact rows, the venue's timezone once.
#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheFile {
    /// When the list was downloaded (Unix ms).
    #[serde(default)]
    fetched_at_ms: Option<i64>,
    #[serde(default)]
    timezone: String,
    #[serde(default)]
    symbols: Vec<CachedSymbol>,
}

/// One symbol: `["BTCUSDT", "BTC", "USDT", 0.01]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedSymbol(String, String, String, f64);

fn to_file(list: &[SymbolInfo], fetched_at_ms: i64) -> CacheFile {
    CacheFile {
        fetched_at_ms: Some(fetched_at_ms),
        timezone: list
            .first()
            .map_or_else(|| "UTC".to_owned(), |s| s.timezone.clone()),
        symbols: list
            .iter()
            .map(|s| {
                CachedSymbol(
                    s.symbol.to_string(),
                    s.base.clone(),
                    s.quote.clone(),
                    s.tick_size,
                )
            })
            .collect(),
    }
}

fn from_file(file: CacheFile) -> Vec<SymbolInfo> {
    let tz = if file.timezone.is_empty() {
        "UTC".to_owned()
    } else {
        file.timezone
    };
    file.symbols
        .into_iter()
        .filter(|s| !s.0.is_empty())
        .map(|CachedSymbol(symbol, base, quote, tick)| {
            let tick_size = if tick > 0.0 { tick } else { 0.01 };
            SymbolInfo {
                symbol: Symbol::new(&symbol),
                tick_size,
                price_precision: precision_from_tick_size(tick_size),
                timezone: tz.clone(),
                base,
                quote,
                session: None,
            }
        })
        .collect()
}

/// Reads `symbols.json`; `None` when missing or unreadable.
fn load(dir: &Path) -> Option<(Option<i64>, Vec<SymbolInfo>)> {
    let bytes = std::fs::read(dir.join(SYMBOLS_FILE)).ok()?;
    match serde_json::from_slice::<CacheFile>(&bytes) {
        Ok(file) => {
            let at = file.fetched_at_ms;
            let list = from_file(file);
            (!list.is_empty()).then_some((at, list))
        }
        Err(e) => {
            warn!("ignoring unreadable {SYMBOLS_FILE}: {e}");
            None
        }
    }
}

fn save(dir: &Path, list: &[SymbolInfo], fetched_at_ms: i64) -> std::io::Result<()> {
    let json = serde_json::to_vec(&to_file(list, fetched_at_ms)).map_err(std::io::Error::other)?;
    crate::fsutil::atomic_write(&dir.join(SYMBOLS_FILE), &json)
}

/// Where the list is.
#[derive(Clone, Debug)]
enum State {
    /// Reading the cache, or downloading without one.
    Loading,
    Ready(Arc<Vec<SymbolInfo>>),
    /// No list at all: the download failed (retried in the background).
    Failed(String),
}

struct Inner {
    state: watch::Sender<State>,
    /// Wakes the background task to retry a failed download now.
    retry: Notify,
}

/// The symbol list for searches; cheap to clone.
#[derive(Clone)]
pub struct SymbolList(Arc<Inner>);

impl SymbolList {
    fn new(state: State) -> Self {
        Self(Arc::new(Inner {
            state: watch::channel(state).0,
            retry: Notify::new(),
        }))
    }

    /// Starts loading on the runtime (cache in `dir`, then keeps it fresh)
    /// and installs every list it gets into `source`. Never blocks.
    pub fn start(dir: Option<PathBuf>, source: Arc<BinanceSource>, handle: &Handle) -> Self {
        let this = Self::new(State::Loading);
        let task = this.clone();
        handle.spawn(async move { task.run(dir, source).await });
        this
    }

    /// A fixed list (tests and tools).
    pub fn fixed(list: Vec<SymbolInfo>) -> Self {
        Self::new(State::Ready(Arc::new(list)))
    }

    fn set(&self, state: State) {
        self.0.state.send_replace(state);
    }

    async fn run(self, dir: Option<PathBuf>, source: Arc<BinanceSource>) {
        let mut fetched_at = None;
        let mut have = false;
        if let Some(d) = dir.clone() {
            let t0 = std::time::Instant::now();
            let disk = tokio::task::spawn_blocking(move || load(&d))
                .await
                .ok()
                .flatten();
            if let Some((at, list)) = disk {
                info!(
                    symbols = list.len(),
                    ms = t0.elapsed().as_millis() as u64,
                    "symbol list loaded from cache"
                );
                let list = Arc::new(list);
                source.set_symbols(list.clone());
                self.set(State::Ready(list));
                fetched_at = at;
                have = true;
            }
        }
        let mut wait = due_in(fetched_at, now_ms());
        loop {
            if !wait.is_zero() {
                tokio::select! {
                    _ = tokio::time::sleep(wait) => {}
                    _ = self.0.retry.notified(), if !have => {}
                }
            }
            if !have {
                self.set(State::Loading);
            }
            let t0 = std::time::Instant::now();
            wait = match source.fetch_symbol_list().await {
                Ok(list) if !list.is_empty() => {
                    debug!(
                        ms = t0.elapsed().as_millis() as u64,
                        "symbol list refreshed"
                    );
                    let list = Arc::new(list);
                    if let Some(d) = dir.clone() {
                        let l = list.clone();
                        let saved =
                            tokio::task::spawn_blocking(move || save(&d, &l, now_ms())).await;
                        if let Ok(Err(e)) = saved {
                            warn!("could not save {SYMBOLS_FILE}: {e}");
                        }
                    }
                    source.set_symbols(list.clone());
                    self.set(State::Ready(list));
                    have = true;
                    REFRESH_EVERY
                }
                Ok(_) => {
                    warn!("symbol list is empty; keeping the cached one");
                    if !have {
                        self.set(State::Failed("the exchange returned no symbols".into()));
                    }
                    if have { RETRY_AFTER } else { RETRY_EMPTY }
                }
                Err(e) => {
                    warn!("symbol list unavailable ({e:#})");
                    if !have {
                        self.set(State::Failed(format!("{e:#}")));
                        RETRY_EMPTY
                    } else {
                        RETRY_AFTER
                    }
                }
            };
        }
    }

    /// The list, if loaded.
    pub fn get(&self) -> Option<Arc<Vec<SymbolInfo>>> {
        match &*self.0.state.borrow() {
            State::Ready(l) => Some(l.clone()),
            _ => None,
        }
    }

    /// Ranked matches for `query` (see [`rank_symbols`]), once the list is
    /// loaded. Errors when there is no list (the download failed; a retry
    /// starts now).
    pub async fn search(&self, query: &str) -> anyhow::Result<Vec<SymbolInfo>> {
        let mut rx = self.0.state.subscribe();
        let state = rx
            .wait_for(|s| !matches!(s, State::Loading))
            .await
            .map(|s| s.clone())
            .map_err(|_| anyhow::anyhow!("symbol list stopped"))?;
        match state {
            State::Ready(list) => Ok(rank_symbols(&list, query, SEARCH_LIMIT)),
            State::Failed(e) => {
                self.0.retry.notify_one();
                Err(anyhow::anyhow!(e))
            }
            State::Loading => unreachable!("waited for a settled state"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(sym: &str, base: &str, quote: &str, tick: f64) -> SymbolInfo {
        SymbolInfo {
            symbol: Symbol::new(sym),
            tick_size: tick,
            price_precision: precision_from_tick_size(tick),
            timezone: "UTC".into(),
            base: base.into(),
            quote: quote.into(),
            session: None,
        }
    }

    #[test]
    fn cache_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let list = vec![
            info("BTCUSDT", "BTC", "USDT", 0.01),
            info("ETHBTC", "ETH", "BTC", 0.00001),
        ];
        save(tmp.path(), &list, 42).unwrap();
        let (at, back) = load(tmp.path()).unwrap();
        assert_eq!(at, Some(42));
        assert_eq!(back, list);
    }

    #[test]
    fn missing_or_corrupt_cache_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).is_none());
        std::fs::write(tmp.path().join(SYMBOLS_FILE), "{nope").unwrap();
        assert!(load(tmp.path()).is_none());
        std::fs::write(tmp.path().join(SYMBOLS_FILE), r#"{"symbols":[]}"#).unwrap();
        assert!(load(tmp.path()).is_none());
    }

    #[test]
    fn searches_rank_the_list() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let l = SymbolList::fixed(vec![
            info("ETHBTC", "ETH", "BTC", 0.00001),
            info("BTCUSDT", "BTC", "USDT", 0.01),
        ]);
        let hits = rt.block_on(l.search("btc")).unwrap();
        assert_eq!(hits[0].symbol.as_str(), "BTCUSDT");
        assert_eq!(hits.len(), 2);
        assert!(l.get().is_some());
    }

    #[test]
    fn startup_installs_the_cached_list() {
        let tmp = tempfile::tempdir().unwrap();
        save(tmp.path(), &[info("BNBUSDT", "BNB", "USDT", 0.1)], now_ms()).unwrap();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        // An unreachable endpoint: the fresh cache must not need it.
        let source = Arc::new(
            BinanceSource::with_endpoints("http://127.0.0.1:9", "ws://127.0.0.1:9").unwrap(),
        );
        let l = SymbolList::start(Some(tmp.path().to_path_buf()), source.clone(), rt.handle());
        let hits = rt.block_on(l.search("BNB")).unwrap();
        assert_eq!(hits[0].symbol.as_str(), "BNBUSDT");
        assert_eq!(source.symbols().unwrap().len(), 1);
    }
}
