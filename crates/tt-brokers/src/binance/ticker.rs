//! 24 h tickers for the watchlist: a REST snapshot
//! (`GET /api/v3/ticker/24hr?symbols=[…]&type=MINI`) and the combined
//! `<symbol>@miniTicker` WebSocket stream (one message per symbol per
//! second at most, only when it changed).

use super::{BinanceSource, CONNECT_TIMEOUT, HTTP_TIMEOUT, READ_TIMEOUT, with_status};
use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, warn};
use tt_core::Symbol;

/// Rolling 24 h statistics of one symbol.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Ticker24h {
    pub last: f64,
    /// Price 24 h ago (the change reference).
    pub open: f64,
    pub high: f64,
    pub low: f64,
    /// Base-asset volume.
    pub volume: f64,
    /// Quote-asset volume.
    pub quote_volume: f64,
}

impl Ticker24h {
    pub fn change(&self) -> f64 {
        self.last - self.open
    }

    /// Change in percent (`None` without a reference price).
    pub fn change_pct(&self) -> Option<f64> {
        (self.open != 0.0).then(|| (self.last / self.open - 1.0) * 100.0)
    }
}

fn price(s: &str, name: &'static str) -> Result<f64> {
    let x: f64 = s.parse().with_context(|| format!("{name} = {s:?}"))?;
    if !x.is_finite() {
        bail!("{name} is not finite");
    }
    Ok(x)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestTicker {
    symbol: String,
    last_price: String,
    open_price: String,
    high_price: String,
    low_price: String,
    volume: String,
    quote_volume: String,
}

/// Parses `GET /api/v3/ticker/24hr` (an array, `FULL` or `MINI`). Entries
/// with unreadable numbers are skipped.
pub fn parse_ticker_24h(body: &str) -> Result<Vec<(String, Ticker24h)>> {
    let raw: Vec<RestTicker> = serde_json::from_str(body).context("invalid ticker JSON")?;
    Ok(raw
        .into_iter()
        .filter_map(|t| {
            let read = || -> Result<Ticker24h> {
                Ok(Ticker24h {
                    last: price(&t.last_price, "lastPrice")?,
                    open: price(&t.open_price, "openPrice")?,
                    high: price(&t.high_price, "highPrice")?,
                    low: price(&t.low_price, "lowPrice")?,
                    volume: price(&t.volume, "volume")?,
                    quote_volume: price(&t.quote_volume, "quoteVolume")?,
                })
            };
            read()
                .map_err(|e| warn!(symbol = %t.symbol, "skipping ticker: {e:#}"))
                .ok()
                .map(|q| (t.symbol, q))
        })
        .collect())
}

#[derive(Deserialize)]
struct MiniTicker {
    #[serde(rename = "e")]
    event: String,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "c")]
    close: String,
    #[serde(rename = "o")]
    open: String,
    #[serde(rename = "h")]
    high: String,
    #[serde(rename = "l")]
    low: String,
    #[serde(rename = "v")]
    volume: String,
    #[serde(rename = "q")]
    quote_volume: String,
}

#[derive(Deserialize)]
struct Combined {
    data: serde_json::Value,
}

/// Parses a `24hrMiniTicker` message, raw or wrapped by the combined
/// stream (`{"stream":…,"data":{…}}`). `Ok(None)` for other messages.
pub fn parse_ws_mini_ticker(text: &str) -> Result<Option<(String, Ticker24h)>> {
    let v: serde_json::Value = serde_json::from_str(text).context("invalid websocket JSON")?;
    let v = match serde_json::from_value::<Combined>(v.clone()) {
        Ok(c) => c.data,
        Err(_) => v,
    };
    if v.get("e").and_then(|e| e.as_str()) != Some("24hrMiniTicker") {
        return Ok(None);
    }
    let m: MiniTicker = serde_json::from_value(v).context("mini ticker")?;
    debug_assert_eq!(m.event, "24hrMiniTicker");
    Ok(Some((
        m.symbol,
        Ticker24h {
            last: price(&m.close, "c")?,
            open: price(&m.open, "o")?,
            high: price(&m.high, "h")?,
            low: price(&m.low, "l")?,
            volume: price(&m.volume, "v")?,
            quote_volume: price(&m.quote_volume, "q")?,
        },
    )))
}

/// Combined-stream URL for `symbols` from the raw-stream base
/// (`…/ws` → `…/stream?streams=a@miniTicker/b@miniTicker`).
pub fn mini_ticker_url(ws_base: &str, symbols: &[Symbol]) -> String {
    let base = ws_base.strip_suffix("/ws").unwrap_or(ws_base);
    let streams: Vec<String> = symbols
        .iter()
        .map(|s| format!("{}@miniTicker", s.as_str().to_ascii_lowercase()))
        .collect();
    format!("{base}/stream?streams={}", streams.join("/"))
}

impl BinanceSource {
    /// 24 h tickers of `symbols` in one request.
    pub async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<Vec<(String, Ticker24h)>> {
        if symbols.is_empty() {
            return Ok(Vec::new());
        }
        let list: Vec<String> = symbols
            .iter()
            .map(|s| format!("\"{}\"", s.as_str()))
            .collect();
        let query = [
            ("symbols", format!("[{}]", list.join(","))),
            ("type", "MINI".to_owned()),
        ];
        let (status, body) = self
            .get("/api/v3/ticker/24hr", &query, HTTP_TIMEOUT)
            .await?;
        with_status(status, parse_ticker_24h(&body))
    }

    /// One connection of the combined mini-ticker stream: calls `on` with
    /// every update until it returns `false` (`Ok`), or the connection
    /// fails or closes (`Err`; the caller reconnects).
    pub async fn run_mini_tickers(
        &self,
        symbols: &[Symbol],
        mut on: impl FnMut(String, Ticker24h) -> bool,
    ) -> Result<()> {
        if symbols.is_empty() {
            return Ok(());
        }
        let url = mini_ticker_url(&self.ws_base, symbols);
        debug!(%url, "connecting mini tickers");
        let (mut ws, _) = timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(url))
            .await
            .context("websocket connect timed out")?
            .context("websocket connect failed")?;
        loop {
            let msg = match timeout(READ_TIMEOUT, ws.next()).await {
                Err(_) => bail!("no data for {}s", READ_TIMEOUT.as_secs()),
                Ok(None) => bail!("server closed the stream"),
                Ok(Some(msg)) => msg.context("websocket read failed")?,
            };
            match msg {
                Message::Text(text) => match parse_ws_mini_ticker(&text) {
                    Ok(Some((symbol, t))) => {
                        if !on(symbol, t) {
                            return Ok(());
                        }
                    }
                    Ok(None) => {}
                    Err(e) => warn!("ignoring bad mini ticker: {e:#}"),
                },
                Message::Ping(_) => ws.flush().await.context("sending pong")?,
                Message::Close(_) => bail!("server closed the stream"),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rest_tickers_and_skips_bad_entries() {
        let body = r#"[
          {"symbol":"BTCUSDT","openPrice":"100.0","highPrice":"110","lowPrice":"90",
           "lastPrice":"105.5","volume":"12.5","quoteVolume":"1300","openTime":0,"closeTime":1},
          {"symbol":"BAD","openPrice":"x","highPrice":"1","lowPrice":"1",
           "lastPrice":"1","volume":"1","quoteVolume":"1"}
        ]"#;
        let t = parse_ticker_24h(body).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].0, "BTCUSDT");
        assert_eq!(t[0].1.last, 105.5);
        assert_eq!(t[0].1.change(), 5.5);
        assert!((t[0].1.change_pct().unwrap() - 5.5).abs() < 1e-9);
    }

    #[test]
    fn parses_combined_and_raw_mini_tickers() {
        let raw = r#"{"e":"24hrMiniTicker","E":1,"s":"ETHUSDT","c":"2","o":"1","h":"3","l":"0.5","v":"10","q":"20"}"#;
        let combined = format!(r#"{{"stream":"ethusdt@miniTicker","data":{raw}}}"#);
        for text in [raw.to_owned(), combined] {
            let (s, t) = parse_ws_mini_ticker(&text).unwrap().unwrap();
            assert_eq!(s, "ETHUSDT");
            assert_eq!((t.last, t.open, t.high, t.low), (2.0, 1.0, 3.0, 0.5));
            assert_eq!(t.change_pct(), Some(100.0));
        }
        assert!(
            parse_ws_mini_ticker(r#"{"result":null,"id":1}"#)
                .unwrap()
                .is_none()
        );
        assert!(parse_ws_mini_ticker("nope").is_err());
    }

    #[test]
    fn combined_stream_url() {
        let url = mini_ticker_url(
            "wss://x/ws",
            &[Symbol::new("BTCUSDT"), Symbol::new("ETHUSDT")],
        );
        assert_eq!(
            url,
            "wss://x/stream?streams=btcusdt@miniTicker/ethusdt@miniTicker"
        );
    }
}
