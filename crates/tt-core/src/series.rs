use crate::Candle;
use std::ops::Range;

/// What [`CandleSeries::upsert`] did with a candle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
    /// The candle replaced the last (still forming) bar.
    Updated,
    /// The candle opened a new bar at the end of the series.
    Appended,
    /// The candle was older than the last bar and was ignored.
    Stale,
}

/// A time-ordered candle series stored as a structure of arrays.
///
/// Each field lives in its own contiguous `Vec`, which keeps per-field scans
/// (min/max for auto-scaling, volume sums, future GPU uploads) cache friendly.
/// All vectors always have the same length and `open_time_ms` is strictly
/// increasing.
///
/// [`Self::version`] changes whenever existing bars move or disappear
/// (anything other than updating the last bar or appending), so derived
/// caches know when an incremental update is not enough.
#[derive(Debug, Clone, Default)]
pub struct CandleSeries {
    version: u64,
    open_time_ms: Vec<i64>,
    open: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    volume: Vec<f64>,
}

impl CandleSeries {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.open_time_ms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.open_time_ms.is_empty()
    }

    /// Structural version; see the type docs.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn clear(&mut self) {
        self.version += 1;
        self.open_time_ms.clear();
        self.open.clear();
        self.high.clear();
        self.low.clear();
        self.close.clear();
        self.volume.clear();
    }

    /// Replaces the whole series. Input is sorted by open time and, for
    /// duplicate open times, the last occurrence wins.
    pub fn set_history(&mut self, candles: impl IntoIterator<Item = Candle>) {
        let mut candles: Vec<Candle> = candles.into_iter().collect();
        // Stable sort keeps arrival order for equal keys, so "last wins" below is well defined.
        candles.sort_by_key(|c| c.open_time_ms);

        self.clear();
        self.reserve(candles.len());
        for c in candles {
            if self.open_time_ms.last() == Some(&c.open_time_ms) {
                self.set_last(&c);
            } else {
                self.push(&c);
            }
        }
    }

    /// Applies a live update:
    /// - same open time as the last bar → replace it (forming bar update),
    /// - newer open time → append a new bar,
    /// - older open time → ignore (stale message).
    pub fn upsert(&mut self, c: Candle) -> UpsertOutcome {
        match self.open_time_ms.last().copied() {
            Some(last) if c.open_time_ms == last => {
                self.set_last(&c);
                UpsertOutcome::Updated
            }
            Some(last) if c.open_time_ms < last => UpsertOutcome::Stale,
            _ => {
                self.push(&c);
                UpsertOutcome::Appended
            }
        }
    }

    /// Keeps only the first `len` bars.
    pub fn truncate(&mut self, len: usize) {
        if len >= self.len() {
            return;
        }
        self.version += 1;
        self.open_time_ms.truncate(len);
        self.open.truncate(len);
        self.high.truncate(len);
        self.low.truncate(len);
        self.close.truncate(len);
        self.volume.truncate(len);
    }

    pub fn get(&self, i: usize) -> Option<Candle> {
        (i < self.len()).then(|| Candle {
            open_time_ms: self.open_time_ms[i],
            open: self.open[i],
            high: self.high[i],
            low: self.low[i],
            close: self.close[i],
            volume: self.volume[i],
        })
    }

    pub fn last(&self) -> Option<Candle> {
        self.len().checked_sub(1).and_then(|i| self.get(i))
    }

    /// Lowest low and highest high over `range` (clamped to the series).
    /// Returns `None` if the clamped range is empty.
    pub fn price_range(&self, range: Range<usize>) -> Option<(f64, f64)> {
        let end = range.end.min(self.len());
        let start = range.start.min(end);
        if start == end {
            return None;
        }
        let lo = self.low[start..end]
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let hi = self.high[start..end]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        Some((lo, hi))
    }

    pub fn open_times(&self) -> &[i64] {
        &self.open_time_ms
    }
    pub fn opens(&self) -> &[f64] {
        &self.open
    }
    pub fn highs(&self) -> &[f64] {
        &self.high
    }
    pub fn lows(&self) -> &[f64] {
        &self.low
    }
    pub fn closes(&self) -> &[f64] {
        &self.close
    }
    pub fn volumes(&self) -> &[f64] {
        &self.volume
    }

    fn reserve(&mut self, n: usize) {
        self.open_time_ms.reserve(n);
        self.open.reserve(n);
        self.high.reserve(n);
        self.low.reserve(n);
        self.close.reserve(n);
        self.volume.reserve(n);
    }

    fn push(&mut self, c: &Candle) {
        self.open_time_ms.push(c.open_time_ms);
        self.open.push(c.open);
        self.high.push(c.high);
        self.low.push(c.low);
        self.close.push(c.close);
        self.volume.push(c.volume);
    }

    /// Overwrites the last bar. Caller guarantees the series is non-empty.
    fn set_last(&mut self, c: &Candle) {
        let i = self.len() - 1;
        self.open_time_ms[i] = c.open_time_ms;
        self.open[i] = c.open;
        self.high[i] = c.high;
        self.low[i] = c.low;
        self.close[i] = c.close;
        self.volume[i] = c.volume;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candle(t: i64, close: f64) -> Candle {
        Candle {
            open_time_ms: t,
            open: 10.0,
            high: close.max(10.0) + 1.0,
            low: close.min(10.0) - 1.0,
            close,
            volume: 1.0,
        }
    }

    #[test]
    fn upsert_into_empty_appends() {
        let mut s = CandleSeries::new();
        assert_eq!(s.upsert(candle(1000, 11.0)), UpsertOutcome::Appended);
        assert_eq!(s.len(), 1);
        assert_eq!(s.last().unwrap().close, 11.0);
    }

    #[test]
    fn upsert_same_time_updates_forming_bar() {
        let mut s = CandleSeries::new();
        s.set_history([candle(1000, 11.0), candle(2000, 12.0)]);
        assert_eq!(s.upsert(candle(2000, 13.5)), UpsertOutcome::Updated);
        assert_eq!(s.len(), 2);
        assert_eq!(s.last().unwrap().close, 13.5);
        // Earlier bars untouched.
        assert_eq!(s.get(0).unwrap().close, 11.0);
    }

    #[test]
    fn upsert_newer_time_appends() {
        let mut s = CandleSeries::new();
        s.set_history([candle(1000, 11.0)]);
        assert_eq!(s.upsert(candle(2000, 9.0)), UpsertOutcome::Appended);
        assert_eq!(s.len(), 2);
        assert_eq!(s.open_times(), &[1000, 2000]);
        assert_eq!(s.closes(), &[11.0, 9.0]);
    }

    #[test]
    fn upsert_older_time_is_ignored() {
        let mut s = CandleSeries::new();
        s.set_history([candle(1000, 11.0), candle(2000, 12.0)]);
        // Older than last, even if it matches an earlier bar's time.
        assert_eq!(s.upsert(candle(1000, 99.0)), UpsertOutcome::Stale);
        assert_eq!(s.upsert(candle(500, 99.0)), UpsertOutcome::Stale);
        assert_eq!(s.len(), 2);
        assert_eq!(s.closes(), &[11.0, 12.0]);
    }

    #[test]
    fn set_history_sorts_and_dedups_last_wins() {
        let mut s = CandleSeries::new();
        s.upsert(candle(9999, 1.0));
        s.set_history([candle(3000, 3.0), candle(1000, 1.0), candle(3000, 3.5)]);
        assert_eq!(s.open_times(), &[1000, 3000]);
        assert_eq!(s.closes(), &[1.0, 3.5]);
    }

    #[test]
    fn soa_columns_stay_in_sync() {
        let mut s = CandleSeries::new();
        for t in 0..10 {
            s.upsert(candle(t * 60_000, t as f64));
        }
        s.upsert(candle(9 * 60_000, 42.0));
        let n = s.len();
        assert_eq!(n, 10);
        for col in [s.opens(), s.highs(), s.lows(), s.closes(), s.volumes()] {
            assert_eq!(col.len(), n);
        }
    }

    #[test]
    fn version_tracks_structural_changes_only() {
        let mut s = CandleSeries::new();
        let v0 = s.version();
        s.upsert(candle(1, 1.0));
        s.upsert(candle(1, 2.0));
        s.upsert(candle(2, 3.0));
        assert_eq!(s.version(), v0, "update/append keep the version");
        s.truncate(1);
        assert_eq!(s.len(), 1);
        assert_ne!(s.version(), v0);
        let v1 = s.version();
        s.truncate(5); // no-op
        assert_eq!(s.version(), v1);
        s.set_history([candle(1, 1.0)]);
        assert_ne!(s.version(), v1);
    }

    #[test]
    fn price_range_over_slice_and_clamped() {
        let mut s = CandleSeries::new();
        s.set_history([candle(1, 10.0), candle(2, 20.0), candle(3, 5.0)]);
        // candle(t, c): high = max(c,10)+1, low = min(c,10)-1
        assert_eq!(s.price_range(0..3), Some((4.0, 21.0)));
        assert_eq!(s.price_range(0..1), Some((9.0, 11.0)));
        assert_eq!(s.price_range(1..100), Some((4.0, 21.0)));
        assert_eq!(s.price_range(3..5), None);
        assert_eq!(CandleSeries::new().price_range(0..10), None);
    }
}
