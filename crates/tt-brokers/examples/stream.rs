//! Headless smoke test: prints Binance events to stdout.
//!
//! `cargo run -p tt-brokers --example stream -- ETHUSDT 1m`

use std::time::Duration;
use tt_brokers::{BinanceSource, MarketDataSource};
use tt_core::{EventSink, MarketEvent, Symbol, Timeframe};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let symbol = Symbol::new(args.next().unwrap_or_else(|| "BTCUSDT".into()));
    let tf = match args.next().as_deref() {
        Some(s) => Timeframe::ALL
            .into_iter()
            .find(|t| t.label().eq_ignore_ascii_case(s))
            .unwrap_or(Timeframe::M1),
        None => Timeframe::M1,
    };
    let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(15);

    let (tx, rx) = crossbeam_channel::unbounded();
    let sink = EventSink::new(tx, || {});
    let source = BinanceSource::new()?;
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
                MarketEvent::CandleUpdate { candle, closed } => {
                    println!(
                        "update: t={} close={} closed={closed}",
                        candle.open_time_ms, candle.close
                    )
                }
                MarketEvent::Status(s) => println!("status: {s}"),
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    task.abort();
    Ok(())
}
