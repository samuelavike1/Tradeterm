use std::fmt;

/// An instrument identifier such as `BTCUSDT`.
///
/// Stored upper-case with surrounding whitespace removed. Venue-specific
/// formatting (e.g. Binance's lower-case stream names) is the adapter's job.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(String);

impl Symbol {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(s.as_ref().trim().to_ascii_uppercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `true` if the symbol is non-empty and only ASCII letters/digits.
    pub fn is_valid(&self) -> bool {
        !self.0.is_empty() && self.0.bytes().all(|b| b.is_ascii_alphanumeric())
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// One OHLCV bar. `open_time_ms` is the Unix epoch time (UTC, milliseconds)
/// at which the bar opened and uniquely identifies it within a series.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candle {
    pub open_time_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl Candle {
    pub fn is_bullish(&self) -> bool {
        self.close >= self.open
    }
}

/// A fixed-point price expressed as an integer number of instrument ticks.
///
/// Charting uses `f64` for speed; order code should use `PriceTicks` so that
/// prices are exact and comparisons never suffer from float rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PriceTicks(pub i64);

impl PriceTicks {
    /// Converts a float price to ticks, rounding to the nearest tick.
    /// Returns `None` for non-finite input, a non-positive tick size, or overflow.
    pub fn from_f64(price: f64, tick_size: f64) -> Option<Self> {
        if !price.is_finite() || !tick_size.is_finite() || tick_size <= 0.0 {
            return None;
        }
        let ticks = (price / tick_size).round();
        if ticks.abs() >= i64::MAX as f64 {
            return None;
        }
        Some(Self(ticks as i64))
    }

    pub fn to_f64(self, tick_size: f64) -> f64 {
        self.0 as f64 * tick_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_normalises() {
        let s = Symbol::new("  btcusdt ");
        assert_eq!(s.as_str(), "BTCUSDT");
        assert!(s.is_valid());
        assert!(!Symbol::new("BTC/USDT").is_valid());
        assert!(!Symbol::new("").is_valid());
    }

    #[test]
    fn price_ticks_round_trip() {
        let t = PriceTicks::from_f64(101.23, 0.01).unwrap();
        assert_eq!(t, PriceTicks(10123));
        assert!((t.to_f64(0.01) - 101.23).abs() < 1e-9);
        assert_eq!(PriceTicks::from_f64(f64::NAN, 0.01), None);
        assert_eq!(PriceTicks::from_f64(1.0, 0.0), None);
    }
}
