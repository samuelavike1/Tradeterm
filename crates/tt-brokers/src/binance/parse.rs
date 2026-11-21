//! Parsing of Binance kline payloads (REST and WebSocket).

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::Value;
use tt_core::{
    Candle, Symbol, SymbolInfo, TimeUnit, Timeframe, can_resample, precision_from_tick_size,
};

/// Intervals Binance serves natively, with their API names.
const NATIVE: [(TimeUnit, u32, &str); 16] = [
    (TimeUnit::Second, 1, "1s"),
    (TimeUnit::Minute, 1, "1m"),
    (TimeUnit::Minute, 3, "3m"),
    (TimeUnit::Minute, 5, "5m"),
    (TimeUnit::Minute, 15, "15m"),
    (TimeUnit::Minute, 30, "30m"),
    (TimeUnit::Hour, 1, "1h"),
    (TimeUnit::Hour, 2, "2h"),
    (TimeUnit::Hour, 4, "4h"),
    (TimeUnit::Hour, 6, "6h"),
    (TimeUnit::Hour, 8, "8h"),
    (TimeUnit::Hour, 12, "12h"),
    (TimeUnit::Day, 1, "1d"),
    (TimeUnit::Day, 3, "3d"),
    (TimeUnit::Week, 1, "1w"),
    (TimeUnit::Month, 1, "1M"),
];

/// Binance's name for `tf` if it is served natively.
pub fn interval(tf: Timeframe) -> Option<&'static str> {
    NATIVE
        .iter()
        .find(|(u, c, _)| Timeframe::new(*u, *c) == tf)
        .map(|(_, _, name)| *name)
}

/// The native interval to download for `tf`: `tf` itself when native,
/// otherwise the largest native interval that can be resampled into it
/// (e.g. 45m ← 15m, 3h ← 1h, 2D ← 1d, 3M ← 1M, 5s ← 1s).
pub fn base_timeframe(tf: Timeframe) -> Timeframe {
    NATIVE
        .iter()
        .rev()
        .map(|(u, c, _)| Timeframe::new(*u, *c))
        .find(|base| can_resample(*base, tf))
        .unwrap_or(Timeframe::M1)
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

#[derive(Deserialize)]
struct ExchangeInfo {
    #[serde(default)]
    timezone: String,
    symbols: Vec<RawSymbol>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSymbol {
    symbol: String,
    status: String,
    base_asset: String,
    quote_asset: String,
    #[serde(default)]
    filters: Vec<Value>,
}

/// Parses `GET /api/v3/exchangeInfo`. With `only_trading`, halted/delisted
/// symbols are dropped.
pub fn parse_exchange_info(body: &str, only_trading: bool) -> Result<Vec<SymbolInfo>> {
    let info: ExchangeInfo = match serde_json::from_str(body) {
        Ok(i) => i,
        Err(e) => {
            if let Ok(err) = serde_json::from_str::<ApiError>(body) {
                bail!("Binance error {}: {}", err.code, err.msg);
            }
            return Err(e).context("invalid exchangeInfo response");
        }
    };
    let tz = if info.timezone.is_empty() {
        "UTC".to_owned()
    } else {
        info.timezone
    };
    Ok(info
        .symbols
        .into_iter()
        .filter(|s| !only_trading || s.status == "TRADING")
        .map(|s| {
            let tick_size = s
                .filters
                .iter()
                .find(|f| f.get("filterType").and_then(Value::as_str) == Some("PRICE_FILTER"))
                .and_then(|f| f.get("tickSize"))
                .and_then(|t| num(t).ok())
                .filter(|t| *t > 0.0)
                .unwrap_or(0.01);
            SymbolInfo {
                symbol: Symbol::new(&s.symbol),
                tick_size,
                price_precision: precision_from_tick_size(tick_size),
                timezone: tz.clone(),
                base: s.base_asset,
                quote: s.quote_asset,
                session: None,
            }
        })
        .collect())
}

/// Ranks symbols for a search box: exact match, then symbol prefix, then
/// base-asset prefix, then substring; within a rank USDT/USDC/BTC/FDUSD
/// quotes first, then shorter symbols.
pub fn rank_symbols(all: &[SymbolInfo], query: &str, limit: usize) -> Vec<SymbolInfo> {
    let q = query.trim().to_ascii_uppercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, &SymbolInfo)> = all
        .iter()
        .filter_map(|s| {
            let sym = s.symbol.as_str();
            let rank = if sym == q {
                0
            } else if sym.starts_with(&q) {
                1
            } else if s.base.starts_with(&q) {
                2
            } else if sym.contains(&q) {
                3
            } else {
                return None;
            };
            Some((rank, s))
        })
        .collect();
    // Within a rank, prefer the most liquid quote currencies, then shorter names.
    let quote_rank = |s: &SymbolInfo| match s.quote.as_str() {
        "USDT" => 0,
        "USDC" => 1,
        "BTC" => 2,
        "FDUSD" => 3,
        _ => 4,
    };
    hits.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(quote_rank(a.1).cmp(&quote_rank(b.1)))
            .then(a.1.symbol.as_str().len().cmp(&b.1.symbol.as_str().len()))
            .then(a.1.symbol.cmp(&b.1.symbol))
    });
    hits.into_iter()
        .take(limit)
        .map(|(_, s)| s.clone())
        .collect()
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

    const EXCHANGE_INFO: &str = r#"{
      "timezone": "UTC", "serverTime": 1, "rateLimits": [], "exchangeFilters": [],
      "symbols": [
        {"symbol": "BTCUSDT", "status": "TRADING", "baseAsset": "BTC", "quoteAsset": "USDT",
         "baseAssetPrecision": 8, "quotePrecision": 8,
         "filters": [
           {"filterType": "PRICE_FILTER", "minPrice": "0.01000000",
            "maxPrice": "1000000.00000000", "tickSize": "0.01000000"},
           {"filterType": "LOT_SIZE", "minQty": "0.00001000", "stepSize": "0.00001000"}
         ]},
        {"symbol": "ETHBTC", "status": "TRADING", "baseAsset": "ETH", "quoteAsset": "BTC",
         "filters": [{"filterType": "PRICE_FILTER", "tickSize": "0.00001000"}]},
        {"symbol": "OLDCOINBTC", "status": "BREAK", "baseAsset": "OLDCOIN", "quoteAsset": "BTC",
         "filters": []}
      ]
    }"#;

    #[test]
    fn parses_exchange_info() {
        let all = parse_exchange_info(EXCHANGE_INFO, false).unwrap();
        assert_eq!(all.len(), 3);
        let btc = &all[0];
        assert_eq!(btc.symbol.as_str(), "BTCUSDT");
        assert_eq!(btc.tick_size, 0.01);
        assert_eq!(btc.price_precision, 2);
        assert_eq!((btc.base.as_str(), btc.quote.as_str()), ("BTC", "USDT"));
        assert_eq!(btc.timezone, "UTC");
        assert_eq!(all[1].price_precision, 5);
        // Missing PRICE_FILTER falls back to a sane tick size.
        assert_eq!(all[2].tick_size, 0.01);

        let trading = parse_exchange_info(EXCHANGE_INFO, true).unwrap();
        assert_eq!(trading.len(), 2);
    }

    #[test]
    fn exchange_info_error_is_reported() {
        let e = parse_exchange_info(r#"{"code":-1121,"msg":"Invalid symbol."}"#, true).unwrap_err();
        assert!(e.to_string().contains("Invalid symbol"));
    }

    #[test]
    fn ranks_search_results() {
        let mk = |sym: &str, base: &str, quote: &str| SymbolInfo {
            base: base.into(),
            quote: quote.into(),
            ..SymbolInfo::fallback(Symbol::new(sym))
        };
        let all = vec![
            mk("WBTCUSDT", "WBTC", "USDT"),
            mk("BTCEUR", "BTC", "EUR"),
            mk("BTCUSDT", "BTC", "USDT"),
            mk("BTCDOMUSDT", "BTCDOM", "USDT"),
            mk("ETHBTC", "ETH", "BTC"),
            mk("ETHU", "ETHU", "SDT"),
            mk("ETHUSDT", "ETH", "USDT"),
            mk("SOLUSDT", "SOL", "USDT"),
        ];
        let names = |q: &str| -> Vec<String> {
            rank_symbols(&all, q, 10)
                .iter()
                .map(|s| s.symbol.to_string())
                .collect()
        };
        assert_eq!(
            names("btc"),
            ["BTCUSDT", "BTCDOMUSDT", "BTCEUR", "WBTCUSDT", "ETHBTC"]
        );
        assert_eq!(names("eth"), ["ETHUSDT", "ETHBTC", "ETHU"]);
        assert_eq!(names("btcusdt")[0], "BTCUSDT");
        assert_eq!(names("sol"), ["SOLUSDT"]);
        assert!(names("").is_empty());
        assert_eq!(rank_symbols(&all, "usdt", 2).len(), 2);
    }

    #[test]
    fn intervals_match_binance() {
        let all: Vec<_> = Timeframe::ALL
            .iter()
            .map(|&t| interval(t).unwrap())
            .collect();
        assert_eq!(all, ["1m", "5m", "15m", "1h", "4h", "1d"]);
        assert_eq!(interval(Timeframe::MN1), Some("1M"));
        assert_eq!(interval(Timeframe::W1), Some("1w"));
        assert_eq!(interval(Timeframe::new(TimeUnit::Minute, 45)), None);
    }

    #[test]
    fn non_native_intervals_pick_the_largest_dividing_base() {
        let base = |code: &str| base_timeframe(Timeframe::parse(code).unwrap()).code();
        assert_eq!(base("45m"), "15m");
        assert_eq!(base("10m"), "5m");
        assert_eq!(base("7m"), "1m");
        assert_eq!(base("3h"), "1h");
        assert_eq!(base("2D"), "1D");
        assert_eq!(base("6D"), "3D");
        assert_eq!(base("2W"), "1W");
        assert_eq!(base("3M"), "1M");
        assert_eq!(base("5s"), "1s");
        assert_eq!(base("4h"), "4h");
    }
}
