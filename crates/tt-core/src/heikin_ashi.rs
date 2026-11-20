//! Heikin Ashi candles.
//!
//! ```text
//! HA close = (open + high + low + close) / 4
//! HA open  = (prev HA open + prev HA close) / 2   (first bar: (open + close) / 2)
//! HA high  = max(high, HA open, HA close)
//! HA low   = min(low,  HA open, HA close)
//! ```

use crate::{Candle, CandleSeries};

/// Computes one HA bar from the raw candle and the previous HA bar.
pub fn heikin_ashi_bar(raw: &Candle, prev_ha: Option<&Candle>) -> Candle {
    let close = (raw.open + raw.high + raw.low + raw.close) / 4.0;
    let open = match prev_ha {
        Some(p) => (p.open + p.close) / 2.0,
        None => (raw.open + raw.close) / 2.0,
    };
    Candle {
        open_time_ms: raw.open_time_ms,
        open,
        high: raw.high.max(open).max(close),
        low: raw.low.min(open).min(close),
        close,
        volume: raw.volume,
    }
}

/// Full Heikin Ashi transform of a series.
pub fn heikin_ashi(src: &CandleSeries) -> CandleSeries {
    let mut out = CandleSeries::new();
    let mut prev: Option<Candle> = None;
    for i in 0..src.len() {
        let bar = heikin_ashi_bar(&src.get(i).expect("index in range"), prev.as_ref());
        out.upsert(bar);
        prev = Some(bar);
    }
    out
}

/// Heikin Ashi series kept in sync with a source series incrementally.
///
/// Live updates touch only the last bar, so [`HeikinAshi::sync`] usually
/// recomputes one bar; it rebuilds fully only when the source's structural
/// [`CandleSeries::version`] changes (history load, prepend, clear).
#[derive(Debug, Clone, Default)]
pub struct HeikinAshi {
    series: CandleSeries,
    src_version: Option<u64>,
    src_len: usize,
    src_last: Option<Candle>,
}

impl HeikinAshi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn series(&self) -> &CandleSeries {
        &self.series
    }

    /// Brings the HA series up to date with `src` and returns it.
    pub fn sync(&mut self, src: &CandleSeries) -> &CandleSeries {
        let unchanged = self.src_version == Some(src.version())
            && self.src_len == src.len()
            && self.src_last == src.last();
        if unchanged {
            return &self.series;
        }

        if self.src_version != Some(src.version()) || src.len() < self.series.len() {
            self.series = heikin_ashi(src);
        } else {
            // Recompute from the (possibly updated) last known bar onward.
            let start = self.series.len().saturating_sub(1);
            for i in start..src.len() {
                let prev = i.checked_sub(1).and_then(|j| self.series.get(j));
                let bar = heikin_ashi_bar(&src.get(i).expect("index in range"), prev.as_ref());
                self.series.upsert(bar);
            }
        }
        self.src_version = Some(src.version());
        self.src_len = src.len();
        self.src_last = src.last();
        &self.series
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(t: i64, o: f64, h: f64, l: f64, cl: f64) -> Candle {
        Candle {
            open_time_ms: t,
            open: o,
            high: h,
            low: l,
            close: cl,
            volume: t as f64,
        }
    }

    #[test]
    fn known_values() {
        let first = heikin_ashi_bar(&c(0, 10.0, 14.0, 8.0, 12.0), None);
        assert_eq!(first.close, 11.0); // (10+14+8+12)/4
        assert_eq!(first.open, 11.0); // (10+12)/2
        assert_eq!(first.high, 14.0);
        assert_eq!(first.low, 8.0);

        let second = heikin_ashi_bar(&c(1, 12.0, 13.0, 11.5, 12.5), Some(&first));
        assert_eq!(second.open, 11.0); // (11+11)/2
        assert_eq!(second.close, 12.25); // (12+13+11.5+12.5)/4
        assert_eq!(second.high, 13.0);
        assert_eq!(second.low, 11.0); // HA open below raw low
        assert_eq!(second.volume, 1.0);
        assert_eq!(second.open_time_ms, 1);
    }

    fn sample(n: i64) -> CandleSeries {
        let mut s = CandleSeries::new();
        for t in 0..n {
            let base = 100.0 + (t as f64 * 0.7).sin() * 5.0;
            s.upsert(c(
                t,
                base,
                base + 2.0,
                base - 1.5,
                base + (t % 3) as f64 - 1.0,
            ));
        }
        s
    }

    fn same(a: &CandleSeries, b: &CandleSeries) -> bool {
        a.len() == b.len() && (0..a.len()).all(|i| a.get(i) == b.get(i))
    }

    #[test]
    fn incremental_matches_full_rebuild() {
        let mut src = sample(50);
        let mut ha = HeikinAshi::new();
        assert!(same(ha.sync(&src), &heikin_ashi(&src)));

        // Forming-bar updates and appends.
        for step in 0..20 {
            let t = 49 + step / 3;
            let px = 101.0 + step as f64;
            src.upsert(c(t, 100.0, px + 1.0, 99.0, px));
            assert!(same(ha.sync(&src), &heikin_ashi(&src)), "step {step}");
        }

        // Structural change → full rebuild.
        src.set_history((0..10).map(|t| c(t * 2, 1.0, 2.0, 0.5, 1.5)));
        assert!(same(ha.sync(&src), &heikin_ashi(&src)));
        src.clear();
        assert!(ha.sync(&src).is_empty());
    }

    #[test]
    fn unchanged_source_is_noop() {
        let src = sample(5);
        let mut ha = HeikinAshi::new();
        ha.sync(&src);
        let v = ha.series().version();
        ha.sync(&src);
        assert_eq!(ha.series().version(), v);
    }
}
