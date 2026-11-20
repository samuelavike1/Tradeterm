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

    fn push(&mut self, time: i64, open: f64, close: f64) {
        let t = time.max(self.last_time.saturating_add(1));
        self.last_time = t;
        self.out.upsert(Candle {
            open_time_ms: t,
            open,
            high: open.max(close),
            low: open.min(close),
            close,
            volume: std::mem::take(&mut self.pending_volume),
        });
    }
}

/// Traditional Renko on closes with a fixed `box_size`.
///
/// A new brick needs a move of one box in the trend direction, or two boxes
/// against it (a reversal brick starts from the far side of the last brick).
pub fn renko(src: &CandleSeries, box_size: f64) -> CandleSeries {
    let mut b = Builder::new();
    if src.is_empty() || !(box_size > 0.0 && box_size.is_finite()) {
        return b.out;
    }
    let (t, c, v) = (src.open_times(), src.closes(), src.volumes());
    // Bricks are anchored to a box grid at the first close.
    let mut top = c[0];
    let mut bottom = c[0];
    let mut dir = 0i8; // 1 up, -1 down, 0 none yet
    for i in 0..src.len() {
        b.pending_volume += v[i];
        let price = c[i];
        loop {
            if price >= top + box_size && dir >= 0 {
                b.push(t[i], top, top + box_size);
                bottom = top;
                top += box_size;
                dir = 1;
            } else if price <= bottom - box_size && dir <= 0 {
                b.push(t[i], bottom, bottom - box_size);
                top = bottom;
                bottom -= box_size;
                dir = -1;
            } else if dir == 1 && price <= bottom - box_size {
                // Reversal down: the new brick starts at the last brick's bottom.
                b.push(t[i], bottom, bottom - box_size);
                top = bottom;
                bottom -= box_size;
                dir = -1;
            } else if dir == -1 && price >= top + box_size {
                b.push(t[i], top, top + box_size);
                bottom = top;
                top += box_size;
                dir = 1;
            } else {
                break;
            }
        }
    }
    b.out
}

/// Box size like TradingView's default ("ATR, length 14"), from the bars
/// before the forming one so it doesn't change on every tick. Falls back to
/// 1% of the last close when there is too little data.
pub fn renko_box_size(src: &CandleSeries) -> f64 {
    let n = src.len();
    atr(src, 14, n.saturating_sub(1))
        .filter(|a| *a > 0.0)
        .or_else(|| src.last().map(|c| (c.close.abs() * 0.01).max(1e-9)))
        .unwrap_or(1.0)
}

/// Line break (three-line break by default) on closes.
///
/// A close beyond the previous line continues the trend; a reversal needs a
/// close beyond the extreme of the last `lines` lines.
pub fn line_break(src: &CandleSeries, lines: usize) -> CandleSeries {
    let mut b = Builder::new();
    let lines = lines.max(1);
    let n = src.len();
    if n < 2 {
        return b.out;
    }
    let (t, c, v) = (src.open_times(), src.closes(), src.volumes());
    // (open, close) of drawn lines.
    let mut drawn: Vec<(f64, f64)> = Vec::new();
    let mut anchor = c[0];
    b.pending_volume += v[0];
    for i in 1..n {
        b.pending_volume += v[i];
        let price = c[i];
        let Some(&(lo, lc)) = drawn.last() else {
            if price != anchor {
                b.push(t[i], anchor, price);
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
            b.push(t[i], o, cl);
            drawn.push((o, cl));
        }
        anchor = price;
    }
    b.out
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
}
