//! Binance spot market data via the public, key-less `data-*.binance.vision` endpoints.

mod parse;

pub use parse::{KlineUpdate, interval, parse_rest_klines, parse_ws_kline};

use crate::MarketDataSource;
use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};
use tt_core::{Candle, ConnectionStatus, EventSink, MarketEvent, Symbol, Timeframe};

pub const DEFAULT_REST_BASE: &str = "https://data-api.binance.vision";
pub const DEFAULT_WS_BASE: &str = "wss://data-stream.binance.vision/ws";

/// Number of historical bars requested on every (re)connect (Binance max is 1000).
const HISTORY_LIMIT: u32 = 1000;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
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
            .timeout(HTTP_TIMEOUT)
            .build()
            .context("building HTTP client")?;
        Ok(Self {
            http,
            rest_base: rest_base.into().trim_end_matches('/').to_owned(),
            ws_base: ws_base.into().trim_end_matches('/').to_owned(),
        })
    }

    /// Fetches the most recent `HISTORY_LIMIT` bars over REST.
    pub async fn fetch_history(&self, symbol: &Symbol, tf: Timeframe) -> Result<Vec<Candle>> {
        let url = format!("{}/api/v3/klines", self.rest_base);
        let limit = HISTORY_LIMIT.to_string();
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("symbol", symbol.as_str()),
                ("interval", interval(tf)),
                ("limit", limit.as_str()),
            ])
            .send()
            .await
            .context("history request failed")?;
        let status = resp.status();
        let body = resp.text().await.context("reading history body")?;
        let parsed = parse_rest_klines(&body);
        if status.is_success() {
            parsed
        } else {
            match parsed {
                Err(e) => Err(e.context(format!("HTTP {status}"))),
                Ok(_) => bail!("HTTP {status}"),
            }
        }
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

        let history = self.fetch_history(symbol, tf).await?;
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
