//! Deterministic synthetic market data for stress-testing the chart.

use tt_core::{Candle, CandleSeries, Symbol, SymbolInfo, Timeframe, resample};

pub const DEFAULT_BARS: usize = 1_000_000;
/// 2024-01-01 00:00 UTC (a Monday).
const START_MS: i64 = 1_704_067_200_000;
const MINUTE: i64 = 60_000;
const DAY: i64 = 1440 * MINUTE;

pub fn symbol_info() -> SymbolInfo {
    SymbolInfo {
        base: "SYN".into(),
        quote: "USD".into(),
        ..SymbolInfo::fallback(Symbol::new("SYNTH"))
    }
}

/// xorshift64* — tiny, fast, deterministic.
struct Rng(u64);

impl Rng {
    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Roughly standard-normal (sum of uniforms).
    fn normal(&mut self) -> f64 {
        (0..4).map(|_| self.next_f64()).sum::<f64>() * 1.732 - 3.464
    }
}

/// `n` one-minute bars of a random walk, weekdays only (weekends are gaps,
/// which the index-based time axis must hide).
pub fn generate_minutes(n: usize, seed: u64) -> CandleSeries {
    let mut rng = Rng(seed | 1);
    let mut series = CandleSeries::new();
    let mut t = START_MS;
    let mut price = 100.0_f64;
    for _ in 0..n {
        // Skip Saturday and Sunday (1970-01-01 was a Thursday).
        while matches!(t.div_euclid(DAY).rem_euclid(7), 2 | 3) {
            t += DAY;
        }
        let open = price;
        let close = open * (rng.normal() * 0.0009).exp();
        let wiggle = open * 0.0006;
        let high = open.max(close) + rng.next_f64() * wiggle;
        let low = (open.min(close) - rng.next_f64() * wiggle).max(0.01);
        let spike = if rng.next_f64() > 0.995 { 20.0 } else { 1.0 };
        series.upsert(Candle {
            open_time_ms: t,
            open,
            high,
            low,
            close,
            volume: (5.0 + rng.next_f64() * 50.0) * spike,
        });
        price = close;
        t += MINUTE;
    }
    series
}

/// Synthetic bars for `tf`, resampled locally from one-minute data.
pub fn generate(n: usize, tf: Timeframe) -> Vec<Candle> {
    let minutes = generate_minutes(n, 0x5EED);
    let series = if tf == Timeframe::M1 {
        minutes
    } else {
        resample(&minutes, Timeframe::M1, tf).expect("1m divides every timeframe")
    };
    (0..series.len()).filter_map(|i| series.get(i)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_weekday_only_and_valid() {
        let a = generate_minutes(8000, 1);
        let b = generate_minutes(8000, 1);
        assert_eq!(a.closes(), b.closes());
        for i in 0..a.len() {
            let c = a.get(i).unwrap();
            assert!(c.low <= c.open.min(c.close) && c.high >= c.open.max(c.close));
            let weekday = c.open_time_ms.div_euclid(DAY).rem_euclid(7);
            assert!(!matches!(weekday, 2 | 3), "weekend bar at {i}");
        }
        // 8000 minutes (> 5 weekdays) from Monday 00:00 crosses the weekend gap.
        let t = a.open_times();
        assert!(t.windows(2).any(|w| w[1] - w[0] > DAY));
    }

    #[test]
    fn resampled_timeframes() {
        let h = generate(3000, Timeframe::H1);
        assert_eq!(h.len(), 50);
        assert!(h.iter().all(|c| c.open_time_ms % (60 * MINUTE) == 0));
    }
}
