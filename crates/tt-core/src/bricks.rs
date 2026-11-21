//! Price-only chart transforms: Renko and Line break.
//!
//! Both turn time bars into "bricks" that form only when price moves enough,
//! so the output has fewer, irregularly timed elements. Each brick becomes a
//! [`Candle`] (open/close = brick bounds, high/low = brick extremes) stamped
//! with the open time of the source bar that completed it; several bricks
//! completed by one bar get +1 ms each so times stay strictly increasing.
//! Volume is the source volume accumulated since the previous brick.

use crate::{Candle, CandleSeries};

/// Average true range over the last `period` bars ending at `end` (exclusive).
pub fn atr(src: &CandleSeries, period: usize, end: usize) -> Option<f64> {
    let end = end.min(src.len());
    if end < 2 || period == 0 {
        return None;
    }
    let start = end.saturating_sub(period).max(1);
    let (h, l, c) = (src.highs(), src.lows(), src.closes());
    let trs: Vec<f64> = (start..end)
        .map(|i| {
            (h[i] - l[i])
                .max((h[i] - c[i - 1]).abs())
                .max((l[i] - c[i - 1]).abs())
        })
        .collect();
    (!trs.is_empty()).then(|| trs.iter().sum::<f64>() / trs.len() as f64)
}

/// Collects bricks with unique, increasing times.
struct Builder {
    out: CandleSeries,
    last_time: i64,
    pending_volume: f64,
}

impl Builder {
    fn new() -> Self {
        Self {
            out: CandleSeries::new(),
            last_time: i64::MIN,
            pending_volume: 0.0,
        }
    }

    /// A brick from `open` to `close`; `high` / `low` include its wicks.
    fn push(&mut self, time: i64, open: f64, close: f64, high: f64, low: f64) {
        let t = time.max(self.last_time.saturating_add(1));
        self.last_time = t;
        self.out.upsert(Candle {
            open_time_ms: t,
            open,
            high: high.max(open.max(close)),
            low: low.min(open.min(close)),
            close,
            volume: std::mem::take(&mut self.pending_volume),
        });
    }
}

/// Bricks built from a time series (Renko, Line break).
#[derive(Debug, Clone, Default)]
pub struct Bricks {
    pub series: CandleSeries,
    /// Index of the first brick completed by the last source bar (the
    /// forming one): the "projection" bricks, which may still change.
    /// `series.len()` when the last bar completed none.
    pub projection_from: usize,
}

/// How the Renko box size is assigned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoxSize {
    /// Average true range over this many bars (before the forming one).
    Atr(usize),
    /// A fixed size in price units.
    Fixed(f64),
    /// A percentage of the last close.
    Percent(f64),
}

/// Renko construction inputs (the box size is resolved separately, see
/// [`renko_box`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenkoParams {
    pub box_size: f64,
    /// Feed open, high, low and close of each bar (in path order) instead of
    /// the close only: highs and lows count toward bricks and wicks.
    pub ohlc: bool,
    /// Record the price extremes reached while a brick formed against its
    /// direction: low wicks on up bricks, high wicks on down bricks.
    pub wicks: bool,
}

/// Rounds `v` to a multiple of `tick` (at least one tick).
fn round_to_tick(v: f64, tick: Option<f64>) -> f64 {
    match tick.filter(|t| *t > 0.0 && t.is_finite()) {
        Some(t) => ((v / t).round() * t).max(t),
        None => v,
    }
}

/// The box size for `method`, rounded to `tick`. ATR uses the bars before
/// the forming one, so it doesn't change on every tick. Falls back to 1% of
/// the last close when there is too little data or the input is invalid.
pub fn renko_box(src: &CandleSeries, method: BoxSize, tick: Option<f64>) -> f64 {
    let last = src.last().map(|c| c.close.abs());
    let fallback = || last.map(|c| (c * 0.01).max(1e-9)).unwrap_or(1.0);
    let raw = match method {
        BoxSize::Atr(n) => atr(src, n.max(1), src.len().saturating_sub(1)),
        BoxSize::Fixed(v) => Some(v),
        BoxSize::Percent(p) => last.map(|c| c * p / 100.0),
    };
    let v = raw
        .filter(|v| *v > 0.0 && v.is_finite())
        .unwrap_or_else(fallback);
    round_to_tick(v, tick)
}

/// Box size like TradingView's default ("ATR, length 14").
pub fn renko_box_size(src: &CandleSeries) -> f64 {
    renko_box(src, BoxSize::Atr(14), None)
}

/// Traditional Renko on closes with a fixed `box_size`, no wicks.
pub fn renko(src: &CandleSeries, box_size: f64) -> CandleSeries {
    renko_with(
        src,
        RenkoParams {
            box_size,
            ohlc: false,
            wicks: false,
        },
    )
    .series
}

/// Renko bricks.
///
/// A new brick needs a move of one box in the trend direction, or two boxes
/// against it (a reversal brick starts from the far side of the last brick).
/// Bricks are anchored to a box grid at the first price.
pub fn renko_with(src: &CandleSeries, p: RenkoParams) -> Bricks {
    let mut b = Builder::new();
    let box_size = p.box_size;
    if src.is_empty() || !(box_size > 0.0 && box_size.is_finite()) {
        return Bricks::default();
    }
    let (t, o, h, l, c, v) = (
        src.open_times(),
        src.opens(),
        src.highs(),
        src.lows(),
        src.closes(),
        src.volumes(),
    );
    let first = if p.ohlc { o[0] } else { c[0] };
    let mut st = RenkoState {
        top: first,
        bottom: first,
        dir: 0,
        ext_hi: first,
        ext_lo: first,
    };
    let n = src.len();
    let mut projection_from = 0;
    for i in 0..n {
        if i + 1 == n {
            projection_from = b.out.len();
        }
        b.pending_volume += v[i];
        if p.ohlc {
            // The likely intrabar path: toward the nearer extreme first.
            let (a, z) = if c[i] >= o[i] {
                (l[i], h[i])
            } else {
                (h[i], l[i])
            };
            for price in [o[i], a, z, c[i]] {
                st.feed(&mut b, t[i], price, box_size, p.wicks);
            }
        } else {
            st.feed(&mut b, t[i], c[i], box_size, p.wicks);
        }
    }
    Bricks {
        series: b.out,
        projection_from,
    }
}

struct RenkoState {
    /// Last brick's top and bottom (both the anchor before the first brick).
    top: f64,
    bottom: f64,
    /// 1 up, -1 down, 0 none yet.
    dir: i8,
    /// Price extremes since the last brick (for wicks).
    ext_hi: f64,
    ext_lo: f64,
}

impl RenkoState {
    fn feed(&mut self, b: &mut Builder, t: i64, price: f64, size: f64, wicks: bool) {
        self.ext_hi = self.ext_hi.max(price);
        self.ext_lo = self.ext_lo.min(price);
        loop {
            // One box past the last brick: a continuation, or (two boxes
            // from the far side) a reversal; before the first brick, either.
            let up = price >= self.top + size;
            let down = price <= self.bottom - size;
            if up {
                let (open, close) = (self.top, self.top + size);
                let low = if wicks { self.ext_lo } else { open };
                b.push(t, open, close, close, low);
                self.bottom = open;
                self.top = close;
                self.dir = 1;
            } else if down {
                let (open, close) = (self.bottom, self.bottom - size);
                let high = if wicks { self.ext_hi } else { open };
                b.push(t, open, close, high, close);
                self.top = open;
                self.bottom = close;
                self.dir = -1;
            } else {
                break;
            }
            // Later bricks from the same move start without a wick.
            let last = b.out.closes()[b.out.len() - 1];
            self.ext_hi = last;
            self.ext_lo = last;
        }
        self.ext_hi = self.ext_hi.max(price);
        self.ext_lo = self.ext_lo.min(price);
    }
}

/// Line break (three-line break by default) on closes.
pub fn line_break(src: &CandleSeries, lines: usize) -> CandleSeries {
    line_break_with(src, lines).series
}

/// Line break on closes.
///
/// A close beyond the previous line continues the trend; a reversal needs a
/// close beyond the extreme of the last `lines` lines.
pub fn line_break_with(src: &CandleSeries, lines: usize) -> Bricks {
    let mut b = Builder::new();
    let lines = lines.max(1);
    let n = src.len();
    if n < 2 {
        return Bricks::default();
    }
    let (t, c, v) = (src.open_times(), src.closes(), src.volumes());
    // (open, close) of drawn lines.
    let mut drawn: Vec<(f64, f64)> = Vec::new();
    let mut anchor = c[0];
    let mut projection_from = 0;
    b.pending_volume += v[0];
    for i in 1..n {
        if i + 1 == n {
            projection_from = b.out.len();
        }
        b.pending_volume += v[i];
        let price = c[i];
        let Some(&(lo, lc)) = drawn.last() else {
            if price != anchor {
                b.push(t[i], anchor, price, anchor, anchor);
                drawn.push((anchor, price));
            }
            continue;
        };
        let up = lc > lo;
        let (hi, low) = (lo.max(lc), lo.min(lc));
        let recent = &drawn[drawn.len().saturating_sub(lines)..];
        let max = recent
            .iter()
            .map(|(o, c)| o.max(*c))
            .fold(f64::MIN, f64::max);
        let min = recent
            .iter()
            .map(|(o, c)| o.min(*c))
            .fold(f64::MAX, f64::min);
        // Continue the trend past the last line, or reverse past the extreme
        // of the last `lines` lines; either way the new line starts at the
        // last line's edge in that direction.
        let line = if (up && price > hi) || (!up && price > max) {
            Some((hi, price))
        } else if (!up && price < low) || (up && price < min) {
            Some((low, price))
        } else {
            None
        };
        if let Some((o, cl)) = line {
            b.push(t[i], o, cl, o, o);
            drawn.push((o, cl));
        }
        anchor = price;
    }
    Bricks {
        series: b.out,
        projection_from,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closes(cs: &[f64]) -> CandleSeries {
        let mut s = CandleSeries::new();
        for (i, c) in cs.iter().enumerate() {
            s.upsert(Candle {
                open_time_ms: i as i64 * 1000,
                open: *c,
                high: c + 0.5,
                low: c - 0.5,
                close: *c,
                volume: 1.0,
            });
        }
        s
    }

    fn oc(s: &CandleSeries) -> Vec<(f64, f64)> {
        (0..s.len())
            .map(|i| (s.opens()[i], s.closes()[i]))
            .collect()
    }

    #[test]
    fn renko_trend_and_reversal() {
        // Up 3 boxes, small pullback (no brick), then a 2-box drop reverses.
        let s = renko(&closes(&[100.0, 103.2, 102.0, 100.9, 99.8]), 1.0);
        assert_eq!(
            oc(&s),
            [
                (100.0, 101.0),
                (101.0, 102.0),
                (102.0, 103.0),
                (102.0, 101.0),
                (101.0, 100.0)
            ]
        );
        // Multiple bricks from one bar get unique, increasing times.
        let t = s.open_times();
        assert!(t.windows(2).all(|w| w[1] > w[0]), "{t:?}");
        assert_eq!(t[0], 1000);
        assert_eq!(t[1], 1001);
        // Volume accumulates into the brick that completes.
        assert_eq!(s.volumes()[0], 2.0);
    }

    #[test]
    fn renko_ignores_moves_below_one_box() {
        let s = renko(&closes(&[100.0, 100.4, 99.7, 100.6]), 1.0);
        assert!(s.is_empty());
        assert!(renko(&closes(&[1.0, 2.0]), 0.0).is_empty());
    }

    #[test]
    fn line_break_three_lines() {
        // Rises make lines; a dip that doesn't break below the last 3 lines'
        // low is ignored; a deeper drop reverses.
        let s = line_break(&closes(&[10.0, 11.0, 12.0, 13.0, 12.5, 9.5]), 3);
        assert_eq!(
            oc(&s),
            [(10.0, 11.0), (11.0, 12.0), (12.0, 13.0), (12.0, 9.5)]
        );
    }

    #[test]
    fn line_break_continues_downtrend() {
        let s = line_break(&closes(&[10.0, 9.0, 8.0, 8.5, 7.0]), 3);
        assert_eq!(oc(&s), [(10.0, 9.0), (9.0, 8.0), (8.0, 7.0)]);
    }

    #[test]
    fn atr_and_box_size() {
        let s = closes(&[10.0, 11.0, 12.0]);
        // TR = max(1.0 range, |h - prev c| = 1.5, |l - prev c| = 0.5) = 1.5
        assert_eq!(atr(&s, 14, 3), Some(1.5));
        assert_eq!(atr(&s, 14, 1), None);
        assert!(renko_box_size(&s) > 0.0);
    }

    fn bars(rows: &[(f64, f64, f64, f64)]) -> CandleSeries {
        let mut s = CandleSeries::new();
        for (i, &(open, high, low, close)) in rows.iter().enumerate() {
            s.upsert(Candle {
                open_time_ms: i as i64 * 1000,
                open,
                high,
                low,
                close,
                volume: 1.0,
            });
        }
        s
    }

    fn params(box_size: f64, ohlc: bool, wicks: bool) -> RenkoParams {
        RenkoParams {
            box_size,
            ohlc,
            wicks,
        }
    }

    #[test]
    fn renko_wicks_show_the_dip_on_up_bricks() {
        // Up a box, dip 1.5 boxes (no reversal), then up another box.
        let s = closes(&[100.0, 101.0, 99.5, 102.0]);
        let b = renko_with(&s, params(1.0, false, true)).series;
        assert_eq!(oc(&b), [(100.0, 101.0), (101.0, 102.0)]);
        assert_eq!(b.lows()[1], 99.5, "low wick on the up brick");
        assert_eq!(b.highs()[1], 102.0, "no high wick on an up brick");
        let plain = renko_with(&s, params(1.0, false, false)).series;
        assert_eq!(plain.lows()[1], 101.0);
    }

    #[test]
    fn renko_wicks_on_down_bricks_are_high_wicks() {
        let s = closes(&[100.0, 99.0, 100.5, 98.0]);
        let b = renko_with(&s, params(1.0, false, true)).series;
        assert_eq!(oc(&b), [(100.0, 99.0), (99.0, 98.0)]);
        assert_eq!(b.highs()[1], 100.5);
        assert_eq!(b.lows()[1], 98.0);
    }

    #[test]
    fn renko_ohlc_source_uses_highs_and_lows() {
        // Closes never move a box, but the high of bar 1 does.
        let s = bars(&[(100.0, 100.2, 99.8, 100.0), (100.0, 101.3, 99.9, 100.1)]);
        assert!(renko_with(&s, params(1.0, false, false)).series.is_empty());
        let b = renko_with(&s, params(1.0, true, false));
        assert_eq!(oc(&b.series), [(100.0, 101.0)]);
        // Formed by the last bar: a projection brick.
        assert_eq!(b.projection_from, 0);
    }

    #[test]
    fn projection_is_the_last_bars_bricks() {
        let s = closes(&[100.0, 101.0, 102.0, 102.5, 104.0]);
        let b = renko_with(&s, params(1.0, false, false));
        assert_eq!(b.series.len(), 4);
        assert_eq!(b.projection_from, 2, "bar 4 completed bricks 2 and 3");
        let s = closes(&[100.0, 101.0, 101.5]);
        let b = renko_with(&s, params(1.0, false, false));
        assert_eq!(b.projection_from, b.series.len(), "none forming");
        let lb = line_break_with(&closes(&[10.0, 11.0, 12.0, 13.0]), 3);
        assert_eq!((lb.series.len(), lb.projection_from), (3, 2));
    }

    #[test]
    fn box_size_methods_round_to_tick() {
        let s = closes(&[10.0, 11.0, 12.0, 200.0]);
        // ATR over the bars before the forming one: TR = 1.5 each.
        assert_eq!(renko_box(&s, BoxSize::Atr(2), None), 1.5);
        assert_eq!(renko_box(&s, BoxSize::Atr(2), Some(1.0)), 2.0);
        assert_eq!(renko_box(&s, BoxSize::Fixed(3.26), Some(0.5)), 3.5);
        assert_eq!(renko_box(&s, BoxSize::Fixed(0.01), Some(0.5)), 0.5);
        // 1 % of the last close.
        assert_eq!(renko_box(&s, BoxSize::Percent(1.0), Some(0.1)), 2.0);
        // Invalid input falls back to 1 % of the last close.
        assert_eq!(renko_box(&s, BoxSize::Fixed(-1.0), None), 2.0);
    }

    #[test]
    fn line_break_number_of_lines() {
        let s = closes(&[10.0, 11.0, 12.0, 13.0, 11.5]);
        // 3 lines: 11.5 is above the low of the last three (11), no line.
        assert_eq!(line_break(&s, 3).len(), 3);
        // 1 line: below the last line's low (12): a reversal.
        assert_eq!(oc(&line_break(&s, 1))[3], (12.0, 11.5));
    }
}
