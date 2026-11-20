//! Headless smoke test against the live Binance API.
//!
//! `cargo run -p tt-brokers --example stream -- ETHUSDT 1m 15`
//!
//! Prints symbol info, a symbol search, one backfill page, then streams
//! events for the given number of seconds.

use std::time::Duration;
use tt_brokers::{BinanceSource, MarketDataSource};
use tt_core::{EventSink, MarketEvent, Symbol, Timeframe};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let symbol = Symbol::new(args.next().unwrap_or_else(|| "BTCUSDT".into()));
    // Any interval code: 1m, 45m, 4h, 1D, 2W, 3M, 5s (non-native ones are resampled).
    let tf = args
        .next()
        .and_then(|s| Timeframe::parse(&s))
        .unwrap_or(Timeframe::M1);
    let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(15);
    let source = BinanceSource::new()?;

    match source.symbol_info(&symbol).await {
        Ok(i) => println!(
            "info: {} {}/{} tick={} precision={} tz={}",
            i.symbol, i.base, i.quote, i.tick_size, i.price_precision, i.timezone
        ),
        Err(e) => println!("info error: {e:#}"),
    }
    let latest = source.fetch_history(&symbol, tf, None, 3).await?;
    if let Some(first) = latest.first() {
        let older = source
            .fetch_history(&symbol, tf, Some(first.open_time_ms), 3)
            .await?;
        println!(
            "paging: latest starts {} | page before ends {:?} ({} bars)",
            first.open_time_ms,
            older.last().map(|c| c.open_time_ms),
            older.len()
        );
    }
    let t = std::time::Instant::now();
    let hits = source.search_symbols("eth").await?;
    println!(
        "search 'eth' ({:.1}s): {:?}",
        t.elapsed().as_secs_f32(),
        hits.iter()
            .take(6)
            .map(|s| s.symbol.to_string())
            .collect::<Vec<_>>()
    );

    let (tx, rx) = crossbeam_channel::unbounded();
    let sink = EventSink::new(tx, || {});
    let task = tokio::spawn(async move { source.run(symbol, tf, sink).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while tokio::time::Instant::now() < deadline {
        while let Ok(ev) = rx.try_recv() {
            match ev {
                MarketEvent::History(c) => println!(
                    "history: {} bars, last close {:?}",
                    c.len(),
                    c.last().map(|c| c.close)
                ),
                MarketEvent::CandleUpdate { candle, closed } => println!(
                    "update: t={} close={} closed={closed}",
                    candle.open_time_ms, candle.close
                ),
                MarketEvent::Status(s) => println!("status: {s}"),
                MarketEvent::SymbolInfo(i) => println!("symbol info: {}", i.symbol),
                MarketEvent::OlderHistory { candles, exhausted } => {
                    println!("older: {} bars, exhausted={exhausted}", candles.len())
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    task.abort();
    Ok(())
}
