//! Parsing of Binance kline payloads (REST and WebSocket).

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::Value;
use tt_core::{Candle, Timeframe};

/// Binance interval string for a timeframe.
pub fn interval(tf: Timeframe) -> &'static str {
    match tf {
        Timeframe::M1 => "1m",
        Timeframe::M5 => "5m",
        Timeframe::M15 => "15m",
        Timeframe::H1 => "1h",
        Timeframe::H4 => "4h",
        Timeframe::D1 => "1d",
    }
}

/// Parses the body of `GET /api/v3/klines`.
///
/// Each row is a JSON array:
/// `[openTime, "open", "high", "low", "close", "volume", closeTime, …]`.
/// Extra trailing fields are ignored so the parser survives API additions.
pub fn parse_rest_klines(body: &str) -> Result<Vec<Candle>> {
    let rows: Vec<Vec<Value>> = match serde_json::from_str(body) {
        Ok(rows) => rows,
        Err(e) => {
            // Binance errors look like {"code":-1121,"msg":"Invalid symbol."}
            if let Ok(err) = serde_json::from_str::<ApiError>(body) {
                bail!("Binance error {}: {}", err.code, err.msg);
            }
            return Err(e).context("klines response is not an array of arrays");
        }
    };

    rows.iter()
        .enumerate()
        .map(|(i, row)| parse_rest_row(row).with_context(|| format!("kline row {i}")))
        .collect()
}

fn parse_rest_row(row: &[Value]) -> Result<Candle> {
    if row.len() < 6 {
        bail!("expected at least 6 fields, got {}", row.len());
    }
    let open_time_ms = row[0]
        .as_i64()
        .ok_or_else(|| anyhow!("open time is not an integer"))?;
    Ok(Candle {
        open_time_ms,
        open: num(&row[1]).context("open")?,
        high: num(&row[2]).context("high")?,
        low: num(&row[3]).context("low")?,
        close: num(&row[4]).context("close")?,
        volume: num(&row[5]).context("volume")?,
    })
}

/// Binance encodes decimals as strings; accept plain numbers too.
fn num(v: &Value) -> Result<f64> {
    let x = match v {
        Value::String(s) => s.parse::<f64>()?,
        Value::Number(n) => n.as_f64().ok_or_else(|| anyhow!("not representable"))?,
        other => bail!("expected number or numeric string, got {other}"),
    };
    if !x.is_finite() {
        bail!("non-finite value");
    }
    Ok(x)
}

#[derive(Deserialize)]
struct ApiError {
    code: i64,
    msg: String,
}

/// A parsed WebSocket kline update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KlineUpdate {
    pub candle: Candle,
    pub closed: bool,
}

#[derive(Deserialize)]
struct WsEnvelope<'a> {
    #[serde(rename = "e", borrow)]
    event_type: Option<&'a str>,
    #[serde(rename = "k")]
    kline: Option<WsKline>,
}

#[derive(Deserialize)]
struct WsKline {
    #[serde(rename = "t")]
    open_time: i64,
    #[serde(rename = "o")]
    open: String,
    #[serde(rename = "h")]
    high: String,
    #[serde(rename = "l")]
    low: String,
    #[serde(rename = "c")]
    close: String,
    #[serde(rename = "v")]
    volume: String,
    #[serde(rename = "x")]
    closed: bool,
}

/// Parses a raw-stream WebSocket text frame.
///
/// Returns `Ok(None)` for well-formed messages that are not kline events
/// (e.g. subscription acks), and `Err` for malformed kline payloads.
pub fn parse_ws_kline(text: &str) -> Result<Option<KlineUpdate>> {
    let env: WsEnvelope<'_> = serde_json::from_str(text).context("invalid websocket JSON")?;
    let (Some("kline"), Some(k)) = (env.event_type, env.kline) else {
        return Ok(None);
    };
    let p = |s: &str, name: &'static str| -> Result<f64> {
        let x: f64 = s.parse().with_context(|| format!("{name} = {s:?}"))?;
        if !x.is_finite() {
            bail!("{name} is not finite");
        }
        Ok(x)
    };
    Ok(Some(KlineUpdate {
        candle: Candle {
            open_time_ms: k.open_time,
            open: p(&k.open, "open")?,
            high: p(&k.high, "high")?,
            low: p(&k.low, "low")?,
            close: p(&k.close, "close")?,
            volume: p(&k.volume, "volume")?,
        },
        closed: k.closed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shape taken from the Binance spot API docs for GET /api/v3/klines.
    const REST_SAMPLE: &str = r#"[
      [1499040000000,"0.01634790","0.80000000","0.01575800","0.01577100","148976.11427815",
       1499644799999,"2434.19055334",308,"1756.87402397","28.46694368","0"],
      [1499040060000,"0.01577100","0.01600000","0.01570000","0.01590000","100.5",
       1499040119999,"1.6",12,"50.0","0.8","0"]
    ]"#;

    #[test]
    fn parses_rest_klines() {
        let c = parse_rest_klines(REST_SAMPLE).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].open_time_ms, 1_499_040_000_000);
        assert_eq!(c[0].open, 0.0163479);
        assert_eq!(c[0].high, 0.8);
        assert_eq!(c[0].low, 0.015758);
        assert_eq!(c[0].close, 0.015771);
        assert_eq!(c[0].volume, 148_976.114_278_15);
        assert_eq!(c[1].open_time_ms, 1_499_040_060_000);
        assert_eq!(c[1].close, 0.0159);
    }

    #[test]
    fn parses_empty_rest_response() {
        assert!(parse_rest_klines("[]").unwrap().is_empty());
    }

    #[test]
    fn rest_api_error_is_reported() {
        let err = parse_rest_klines(r#"{"code":-1121,"msg":"Invalid symbol."}"#).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("-1121") && msg.contains("Invalid symbol"),
            "{msg}"
        );
    }

    #[test]
    fn rest_rejects_malformed_rows() {
        assert!(parse_rest_klines(r#"[[1499040000000,"1","2"]]"#).is_err());
        assert!(parse_rest_klines(r#"[[1499040000000,"x","2","3","4","5"]]"#).is_err());
        assert!(parse_rest_klines(r#"[["t","1","2","3","4","5"]]"#).is_err());
        assert!(parse_rest_klines("not json").is_err());
    }

    // Shape taken from the Binance WebSocket streams docs (<symbol>@kline_<interval>).
    const WS_SAMPLE: &str = r#"{
      "e": "kline", "E": 1672515782136, "s": "BNBBTC",
      "k": {
        "t": 1672515780000, "T": 1672515839999, "s": "BNBBTC", "i": "1m",
        "f": 100, "L": 200,
        "o": "0.0010", "c": "0.0020", "h": "0.0025", "l": "0.0015",
        "v": "1000", "n": 100, "x": false,
        "q": "1.0000", "V": "500", "Q": "0.500", "B": "123456"
      }
    }"#;

    #[test]
    fn parses_ws_kline() {
        let u = parse_ws_kline(WS_SAMPLE).unwrap().unwrap();
        assert!(!u.closed);
        assert_eq!(
            u.candle,
            Candle {
                open_time_ms: 1_672_515_780_000,
                open: 0.001,
                high: 0.0025,
                low: 0.0015,
                close: 0.002,
                volume: 1000.0,
            }
        );
    }

    #[test]
    fn parses_closed_ws_kline() {
        let closed = WS_SAMPLE.replace(r#""x": false"#, r#""x": true"#);
        assert!(parse_ws_kline(&closed).unwrap().unwrap().closed);
    }

    #[test]
    fn ws_non_kline_messages_are_skipped() {
        assert_eq!(parse_ws_kline(r#"{"result":null,"id":1}"#).unwrap(), None);
        assert_eq!(parse_ws_kline(r#"{"e":"trade","p":"1"}"#).unwrap(), None);
    }

    #[test]
    fn ws_malformed_is_error() {
        assert!(parse_ws_kline("{").is_err());
        let bad = WS_SAMPLE.replace(r#""o": "0.0010""#, r#""o": "abc""#);
        assert!(parse_ws_kline(&bad).is_err());
    }

    #[test]
    fn intervals_match_binance() {
        let all: Vec<_> = Timeframe::ALL.iter().map(|&t| interval(t)).collect();
        assert_eq!(all, ["1m", "5m", "15m", "1h", "4h", "1d"]);
    }
}
