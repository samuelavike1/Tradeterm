//! Binance spot market data via the public, key-less `data-*.binance.vision` endpoints.

mod parse;

pub use parse::{
    KlineUpdate, interval, parse_exchange_info, parse_rest_klines, parse_ws_kline, rank_symbols,
};

use crate::MarketDataSource;
use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};
use tt_core::{Candle, ConnectionStatus, EventSink, MarketEvent, Symbol, SymbolInfo, Timeframe};

pub const DEFAULT_REST_BASE: &str = "https://data-api.binance.vision";
pub const DEFAULT_WS_BASE: &str = "wss://data-stream.binance.vision/ws";

/// Bars per klines request (Binance maximum).
pub const MAX_KLINES: usize = 1000;
/// Search results returned by `search_symbols`.
const SEARCH_LIMIT: usize = 100;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
/// The full symbol list is ~6 MB; give it more time on slow links.
const SYMBOL_LIST_TIMEOUT: Duration = Duration::from_secs(90);
/// Binance pings every ~20s, so a minute of silence means the link is dead.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const BACKOFF_MIN: Duration = Duration::from_millis(500);
const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// Binance spot klines adapter. Cheap to share; holds a pooled HTTP client.
#[derive(Debug, Clone)]
pub struct BinanceSource {
    http: reqwest::Client,
    rest_base: String,
    ws_base: String,
    /// All trading symbols, downloaded once on first search (~6 MB).
    symbols: Arc<OnceCell<Arc<Vec<SymbolInfo>>>>,
}

impl BinanceSource {
    pub fn new() -> Result<Self> {
        Self::with_endpoints(DEFAULT_REST_BASE, DEFAULT_WS_BASE)
    }

    /// Custom endpoints, e.g. a mock server in tests.
    pub fn with_endpoints(
        rest_base: impl Into<String>,
        ws_base: impl Into<String>,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("tradeterm/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building HTTP client")?;
        Ok(Self {
            http,
            rest_base: rest_base.into().trim_end_matches('/').to_owned(),
            ws_base: ws_base.into().trim_end_matches('/').to_owned(),
            symbols: Arc::new(OnceCell::new()),
        })
    }

    /// GET helper: returns the body, turning HTTP errors into readable messages
    /// (Binance error JSON is parsed by the caller's parser).
    async fn get(
        &self,
        path: &str,
        query: &[(&str, String)],
        timeout: Duration,
    ) -> Result<(reqwest::StatusCode, String)> {
        let url = format!("{}{path}", self.rest_base);
        let resp = self
            .http
            .get(&url)
            .timeout(timeout)
            .query(query)
            .send()
            .await
            .with_context(|| format!("request to {path} failed"))?;
        let status = resp.status();
        let body = resp.text().await.context("reading response body")?;
        Ok((status, body))
    }

    /// `GET /api/v3/klines`; `before` maps to `endTime = before - 1`.
    async fn fetch_klines(
        &self,
        symbol: &Symbol,
        tf: Timeframe,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<Candle>> {
        let mut query = vec![
            ("symbol", symbol.as_str().to_owned()),
            ("interval", interval(tf).to_owned()),
            ("limit", limit.clamp(1, MAX_KLINES).to_string()),
        ];
        if let Some(t) = before {
            query.push(("endTime", (t - 1).to_string()));
        }
        let (status, body) = self.get("/api/v3/klines", &query, HTTP_TIMEOUT).await?;
        with_status(status, parse_rest_klines(&body))
    }

    /// Full list of trading symbols, fetched once and cached.
    async fn all_symbols(&self) -> Result<Arc<Vec<SymbolInfo>>> {
        self.symbols
            .get_or_try_init(|| async {
                let query = [("symbolStatus", "TRADING".to_owned())];
                let (status, body) = self
                    .get("/api/v3/exchangeInfo", &query, SYMBOL_LIST_TIMEOUT)
                    .await?;
                let list = with_status(status, parse_exchange_info(&body, true))?;
                info!(symbols = list.len(), "symbol list loaded");
                Ok::<_, anyhow::Error>(Arc::new(list))
            })
            .await
            .cloned()
    }

    /// One connection lifetime: connect WS, load history, then forward live bars.
    async fn session(
        &self,
        symbol: &Symbol,
        tf: Timeframe,
        sink: &EventSink,
        backoff: &mut Backoff,
    ) -> Result<SessionEnd> {
        let url = format!(
            "{}/{}@kline_{}",
            self.ws_base,
            symbol.as_str().to_ascii_lowercase(),
            interval(tf)
        );
        debug!(%url, "connecting websocket");

        // Connect the stream *first* so that bars produced while history is
        // downloading are buffered on the socket instead of being lost.
        let (mut ws, _) = timeout(
            CONNECT_TIMEOUT,
            tokio_tungstenite::connect_async(url.as_str()),
        )
        .await
        .context("websocket connect timed out")?
        .context("websocket connect failed")?;

        let history = self.fetch_klines(symbol, tf, None, MAX_KLINES).await?;
        info!(%symbol, %tf, bars = history.len(), "history loaded");
        if !sink.send(MarketEvent::History(history)) {
            return Ok(SessionEnd::SinkClosed);
        }
        if !sink.send(MarketEvent::Status(ConnectionStatus::Connected)) {
            return Ok(SessionEnd::SinkClosed);
        }
        backoff.reset();

        loop {
            let msg = match timeout(READ_TIMEOUT, ws.next()).await {
                Err(_) => bail!("no data for {}s", READ_TIMEOUT.as_secs()),
                Ok(None) => return Ok(SessionEnd::ServerClosed(None)),
                Ok(Some(msg)) => msg.context("websocket read failed")?,
            };
            match msg {
                Message::Text(text) => match parse_ws_kline(&text) {
                    Ok(Some(u)) => {
                        let ev = MarketEvent::CandleUpdate {
                            candle: u.candle,
                            closed: u.closed,
                        };
                        if !sink.send(ev) {
                            return Ok(SessionEnd::SinkClosed);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => warn!("ignoring bad kline message: {e:#}"),
                },
                // tungstenite queues the Pong automatically; flush so it goes out now.
                Message::Ping(_) => ws.flush().await.context("sending pong")?,
                Message::Close(frame) => {
                    let reason = frame
                        .map(|f| f.reason.to_string())
                        .filter(|r| !r.is_empty());
                    return Ok(SessionEnd::ServerClosed(reason));
                }
                _ => {}
            }
        }
    }
}

/// Adds the HTTP status to errors from non-2xx responses.
fn with_status<T>(status: reqwest::StatusCode, parsed: Result<T>) -> Result<T> {
    if status.is_success() {
        parsed
    } else {
        match parsed {
            Err(e) => Err(e.context(format!("HTTP {status}"))),
            Ok(_) => bail!("HTTP {status}"),
        }
    }
}

enum SessionEnd {
    /// The consumer went away; stop for good.
    SinkClosed,
    /// The server ended the stream (Binance does so every 24h); reconnect.
    ServerClosed(Option<String>),
}

impl MarketDataSource for BinanceSource {
    fn name(&self) -> &'static str {
        "Binance"
    }

    async fn symbol_info(&self, symbol: &Symbol) -> Result<SymbolInfo> {
        // Use the cached list if a search already loaded it.
        if let Some(all) = self.symbols.get()
            && let Some(s) = all.iter().find(|s| s.symbol == *symbol)
        {
            return Ok(s.clone());
        }
        let query = [("symbol", symbol.as_str().to_owned())];
        let (status, body) = self
            .get("/api/v3/exchangeInfo", &query, HTTP_TIMEOUT)
            .await?;
        with_status(status, parse_exchange_info(&body, false))?
            .into_iter()
            .next()
            .with_context(|| format!("unknown symbol {symbol}"))
    }

    async fn search_symbols(&self, query: &str) -> Result<Vec<SymbolInfo>> {
        let all = self.all_symbols().await?;
        Ok(rank_symbols(&all, query, SEARCH_LIMIT))
    }

    async fn fetch_history(
        &self,
        symbol: &Symbol,
        timeframe: Timeframe,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<Candle>> {
        self.fetch_klines(symbol, timeframe, before, limit).await
    }

    async fn run(&self, symbol: Symbol, timeframe: Timeframe, sink: EventSink) -> Result<()> {
        let mut backoff = Backoff::new(BACKOFF_MIN, BACKOFF_MAX);
        loop {
            if !sink.send(MarketEvent::Status(ConnectionStatus::Connecting)) {
                return Ok(());
            }
            let reason = match self.session(&symbol, timeframe, &sink, &mut backoff).await {
                Ok(SessionEnd::SinkClosed) => return Ok(()),
                Ok(SessionEnd::ServerClosed(r)) => {
                    Some(r.unwrap_or_else(|| "server closed the stream".to_owned()))
                }
                Err(e) => Some(format!("{e:#}")),
            };
            let delay = backoff.next_delay();
            warn!(%symbol, %timeframe, ?reason, ?delay, "stream disconnected; retrying");
            if !sink.send(MarketEvent::Status(ConnectionStatus::Disconnected {
                reason,
            })) {
                return Ok(());
            }
            tokio::time::sleep(delay).await;
        }
    }
}

/// Exponential backoff: `min, 2·min, 4·min, …` capped at `max`.
#[derive(Debug, Clone)]
pub struct Backoff {
    min: Duration,
    max: Duration,
    current: Duration,
}

impl Backoff {
    pub fn new(min: Duration, max: Duration) -> Self {
        Self {
            min,
            max,
            current: min,
        }
    }

    /// Returns the delay to wait now and doubles the next one.
    pub fn next_delay(&mut self) -> Duration {
        let d = self.current;
        self.current = (self.current * 2).min(self.max);
        d
    }

    pub fn reset(&mut self) {
        self.current = self.min;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_caps_and_resets() {
        let mut b = Backoff::new(BACKOFF_MIN, BACKOFF_MAX);
        let ms: Vec<u128> = (0..9).map(|_| b.next_delay().as_millis()).collect();
        assert_eq!(
            ms,
            [500, 1000, 2000, 4000, 8000, 16000, 30000, 30000, 30000]
        );
        b.reset();
        assert_eq!(b.next_delay(), BACKOFF_MIN);
    }
}
