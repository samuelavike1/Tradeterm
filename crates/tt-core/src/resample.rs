//! Building higher timeframes from lower ones (e.g. 1m → 5m / 15m / 1h / 1D).
//!
//! Buckets are aligned to multiples of the target interval since the Unix
//! epoch (UTC): 5m bars start at :00, :05, …; 4h bars at 00:00, 04:00, …;
//! daily bars at 00:00 UTC. A target bar takes the first open, highest high,
//! lowest low, last close and summed volume of the source bars in its bucket.
//! Missing source bars (gaps) simply contribute nothing; empty buckets
//! produce no bar.

use crate::{Candle, CandleSeries, Timeframe};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResampleError {
    /// `to` is not a whole multiple of `from` (or is smaller).
    Incompatible { from: Timeframe, to: Timeframe },
}

impl fmt::Display for ResampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResampleError::Incompatible { from, to } => {
                write!(f, "cannot build {to} bars from {from} bars")
            }
        }
    }
}

impl std::error::Error for ResampleError {}

/// Start of the bucket containing `t` for buckets of `bucket_ms`.
pub fn bucket_start(t: i64, bucket_ms: i64) -> i64 {
    t.div_euclid(bucket_ms) * bucket_ms
}

/// `true` if `to` bars can be built from `from` bars.
pub fn can_resample(from: Timeframe, to: Timeframe) -> bool {
    let (f, t) = (from.duration_ms(), to.duration_ms());
    t >= f && t % f == 0
}

fn check(from: Timeframe, to: Timeframe) -> Result<i64, ResampleError> {
    if can_resample(from, to) {
        Ok(to.duration_ms())
    } else {
        Err(ResampleError::Incompatible { from, to })
    }
}

/// Merges `c` into the bucket aggregate `agg`.
fn merge(agg: &mut Candle, c: &Candle) {
    agg.high = agg.high.max(c.high);
    agg.low = agg.low.min(c.low);
    agg.close = c.close;
    agg.volume += c.volume;
}

/// Batch resampling of a whole series (assumed time-ordered, as
/// [`CandleSeries`] guarantees).
pub fn resample(
    src: &CandleSeries,
    from: Timeframe,
    to: Timeframe,
) -> Result<CandleSeries, ResampleError> {
    let bucket_ms = check(from, to)?;
    let mut out = CandleSeries::new();
    let (t, o, h, l, c, v) = (
        src.open_times(),
        src.opens(),
        src.highs(),
        src.lows(),
        src.closes(),
        src.volumes(),
    );
    let mut agg: Option<Candle> = None;
    for i in 0..src.len() {
        let bar = Candle {
            open_time_ms: t[i],
            open: o[i],
            high: h[i],
            low: l[i],
            close: c[i],
            volume: v[i],
        };
        let b = bucket_start(bar.open_time_ms, bucket_ms);
        match &mut agg {
            Some(a) if a.open_time_ms == b => merge(a, &bar),
            _ => {
                if let Some(done) = agg.take() {
                    out.upsert(done);
                }
                agg = Some(Candle {
                    open_time_ms: b,
                    ..bar
                });
            }
        }
    }
    if let Some(done) = agg {
        out.upsert(done);
    }
    Ok(out)
}

/// Incremental resampler for live data.
///
/// Feed every lower-timeframe update (including repeated updates of the
/// forming bar); it returns the up-to-date higher-timeframe bar to
/// [`CandleSeries::upsert`] into the target series.
#[derive(Debug, Clone)]
pub struct Resampler {
    bucket_ms: i64,
    bucket: Option<i64>,
    /// Source bars of the current bucket, by open time.
    parts: Vec<Candle>,
}

impl Resampler {
    pub fn new(from: Timeframe, to: Timeframe) -> Result<Self, ResampleError> {
        Ok(Self {
            bucket_ms: check(from, to)?,
            bucket: None,
            parts: Vec::new(),
        })
    }

    /// Applies a source bar; returns the aggregated target bar, or `None`
    /// for updates older than the current bucket.
    pub fn push(&mut self, c: Candle) -> Option<Candle> {
        let b = bucket_start(c.open_time_ms, self.bucket_ms);
        match self.bucket {
            Some(cur) if b < cur => return None,
            Some(cur) if b == cur => {
                match self
                    .parts
                    .binary_search_by_key(&c.open_time_ms, |p| p.open_time_ms)
                {
                    Ok(i) => self.parts[i] = c,
                    Err(i) => self.parts.insert(i, c),
                }
            }
            _ => {
                self.bucket = Some(b);
                self.parts.clear();
                self.parts.push(c);
            }
        }
        let mut agg = Candle {
            open_time_ms: b,
            ..self.parts[0]
        };
        for p in &self.parts[1..] {
            merge(&mut agg, p);
        }
        Some(agg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;
    const HOUR: i64 = 60 * MIN;
    /// 2026-11-20 00:00 UTC
    const DAY0: i64 = 1_795_132_800_000;

    fn bar(t: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Candle {
        Candle {
            open_time_ms: t,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: v,
        }
    }

    /// 1m bars from `start`, prices i, i+2, i-2, i+1 and volume 1.
    fn minutes(start: i64, n: i64) -> CandleSeries {
        let mut s = CandleSeries::new();
        for i in 0..n {
            let p = i as f64;
            s.upsert(bar(start + i * MIN, p, p + 2.0, p - 2.0, p + 1.0, 1.0));
        }
        s
    }

    #[test]
    fn compatibility() {
        assert!(can_resample(Timeframe::M1, Timeframe::M5));
        assert!(can_resample(Timeframe::M5, Timeframe::M15));
        assert!(can_resample(Timeframe::M15, Timeframe::H4));
        assert!(can_resample(Timeframe::H4, Timeframe::D1));
        assert!(can_resample(Timeframe::H1, Timeframe::H1));
        assert!(!can_resample(Timeframe::H1, Timeframe::M15));
        let e = resample(&CandleSeries::new(), Timeframe::D1, Timeframe::H4).unwrap_err();
        assert_eq!(e.to_string(), "cannot build 4h bars from 1D bars");
    }

    #[test]
    fn one_minute_to_five_minutes() {
        let src = minutes(DAY0, 10);
        let out = resample(&src, Timeframe::M1, Timeframe::M5).unwrap();
        assert_eq!(out.open_times(), &[DAY0, DAY0 + 5 * MIN]);
        let b = out.get(0).unwrap();
        assert_eq!(
            (b.open, b.high, b.low, b.close, b.volume),
            (0.0, 6.0, -2.0, 5.0, 5.0)
        );
        let b = out.get(1).unwrap();
        assert_eq!((b.open, b.high, b.low, b.close), (5.0, 11.0, 3.0, 10.0));
    }

    #[test]
    fn buckets_align_to_the_clock_not_the_first_bar() {
        // Data starts at 10:03 → first 5m bar is 10:00 (partial), then 10:05.
        let start = DAY0 + 10 * HOUR + 3 * MIN;
        let out = resample(&minutes(start, 4), Timeframe::M1, Timeframe::M5).unwrap();
        assert_eq!(
            out.open_times(),
            &[DAY0 + 10 * HOUR, DAY0 + 10 * HOUR + 5 * MIN]
        );
        assert_eq!(out.get(0).unwrap().volume, 2.0); // 10:03, 10:04
        assert_eq!(out.get(1).unwrap().volume, 2.0); // 10:05, 10:06

        // 15m and 1h behave the same way.
        let out = resample(&minutes(start, 120), Timeframe::M1, Timeframe::H1).unwrap();
        assert_eq!(out.open_times()[0], DAY0 + 10 * HOUR);
        assert_eq!(out.get(0).unwrap().volume, 57.0); // 10:03..10:59
        assert_eq!(out.get(1).unwrap().volume, 60.0);
    }

    #[test]
    fn daily_buckets_split_at_utc_midnight_and_skip_gaps() {
        let mut src = CandleSeries::new();
        // 23:58, 23:59 on day 0; 00:00, 00:01 on day 1; then a 2-day gap.
        for (k, t) in [-2, -1, 0, 1].iter().enumerate() {
            let p = k as f64;
            src.upsert(bar(DAY0 + 24 * HOUR + t * MIN, p, p, p, p, 1.0));
        }
        src.upsert(bar(DAY0 + 4 * 24 * HOUR, 9.0, 9.0, 9.0, 9.0, 1.0));
        let out = resample(&src, Timeframe::M1, Timeframe::D1).unwrap();
        assert_eq!(
            out.open_times(),
            &[DAY0, DAY0 + 24 * HOUR, DAY0 + 4 * 24 * HOUR]
        );
        assert_eq!(out.get(0).unwrap().close, 1.0);
        assert_eq!(out.get(1).unwrap().open, 2.0);
    }

    #[test]
    fn four_hour_buckets() {
        let src = minutes(DAY0 + 3 * HOUR, 120); // 03:00..04:59
        let out = resample(&src, Timeframe::M1, Timeframe::H4).unwrap();
        assert_eq!(out.open_times(), &[DAY0, DAY0 + 4 * HOUR]);
    }

    #[test]
    fn incremental_matches_batch() {
        let mut live = CandleSeries::new();
        let mut target = CandleSeries::new();
        let mut r = Resampler::new(Timeframe::M1, Timeframe::M5).unwrap();
        for i in 0..23 {
            let t = DAY0 + i * MIN;
            // Several updates of each forming 1m bar, as a live feed sends them.
            for tick in 0..3 {
                let p = i as f64 + tick as f64 * 0.1;
                let c = bar(t, i as f64, p + 2.0, p - 2.0, p, 1.0 + tick as f64);
                live.upsert(c);
                target.upsert(r.push(c).unwrap());
            }
        }
        let batch = resample(&live, Timeframe::M1, Timeframe::M5).unwrap();
        assert_eq!(target.len(), batch.len());
        for i in 0..batch.len() {
            assert_eq!(target.get(i), batch.get(i), "bucket {i}");
        }
        // Stale input for an earlier bucket is ignored.
        assert_eq!(r.push(bar(DAY0, 0.0, 0.0, 0.0, 0.0, 0.0)), None);
    }
}
