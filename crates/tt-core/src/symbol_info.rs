use crate::Symbol;

/// Static description of an instrument.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolInfo {
    pub symbol: Symbol,
    /// Minimum price increment.
    pub tick_size: f64,
    /// Decimals needed to display prices (derived from `tick_size`).
    pub price_precision: usize,
    /// IANA name of the exchange timezone (e.g. `"UTC"`).
    pub timezone: String,
    pub base: String,
    pub quote: String,
    /// Trading session description, if the venue has one (crypto: `None`, 24/7).
    pub session: Option<String>,
}

impl SymbolInfo {
    /// Best guess when the venue's metadata is unavailable.
    pub fn fallback(symbol: Symbol) -> Self {
        Self {
            symbol,
            tick_size: 0.01,
            price_precision: 2,
            timezone: "UTC".to_owned(),
            base: String::new(),
            quote: String::new(),
            session: None,
        }
    }
}

/// Decimals implied by a tick size: `0.01` → 2, `0.5` → 1, `1` → 0.
pub fn precision_from_tick_size(tick_size: f64) -> usize {
    if !(tick_size > 0.0 && tick_size.is_finite()) {
        return 2;
    }
    for d in 0..=12usize {
        let scaled = tick_size * 10f64.powi(d as i32);
        if (scaled - scaled.round()).abs() < 1e-9 * scaled.max(1.0) {
            return d;
        }
    }
    12
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precision() {
        assert_eq!(precision_from_tick_size(0.01), 2);
        assert_eq!(precision_from_tick_size(0.00001), 5);
        assert_eq!(precision_from_tick_size(1.0), 0);
        assert_eq!(precision_from_tick_size(0.25), 2);
        assert_eq!(precision_from_tick_size(-1.0), 2);
    }
}
