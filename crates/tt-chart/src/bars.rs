//! The per-frame list of bars to draw.
//!
//! Drawing code never walks the series directly; it draws [`DrawBar`]s. When
//! zoomed in there is one per visible bar; when zoomed far out (B6) one per
//! pixel column, aggregated from a precomputed min/max pyramid. Either way the
//! renderers and auto-scaling see the same shape of data.

use crate::viewport::Viewport;
use std::ops::Range;
use tt_core::CandleSeries;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawBar {
    /// Screen x of the bar (or pixel column) center.
    pub x: f32,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// Close of the preceding bar (for hollow-candle coloring).
    pub prev_close: f64,
    /// Lowest / highest close inside an aggregated column (= close for single bars).
    pub close_min: f64,
    pub close_max: f64,
    pub volume: f64,
}

/// One `DrawBar` per bar in `range`.
pub fn collect_bars(series: &CandleSeries, range: Range<usize>, vp: &Viewport) -> Vec<DrawBar> {
    let (o, h, l, c, v) = (
        series.opens(),
        series.highs(),
        series.lows(),
        series.closes(),
        series.volumes(),
    );
    range
        .map(|i| DrawBar {
            x: vp.x_of(i as f32),
            open: o[i],
            high: h[i],
            low: l[i],
            close: c[i],
            prev_close: if i > 0 { c[i - 1] } else { o[i] },
            close_min: c[i],
            close_max: c[i],
            volume: v[i],
        })
        .collect()
}

/// (min, max) price over the bars: high/low, or close only for line charts.
pub fn price_extent(bars: &[DrawBar], close_only: bool) -> Option<(f64, f64)> {
    if bars.is_empty() {
        return None;
    }
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for b in bars {
        let (l, h) = if close_only {
            (b.close_min, b.close_max)
        } else {
            (b.low, b.high)
        };
        lo = lo.min(l);
        hi = hi.max(h);
    }
    Some((lo, hi))
}

pub fn max_volume(bars: &[DrawBar]) -> f64 {
    bars.iter().map(|b| b.volume).fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::{PlotRect, PriceScale};
    use tt_core::Candle;

    #[test]
    fn collects_with_prev_close_and_extents() {
        let mut s = CandleSeries::new();
        for (t, cl) in [(0, 10.0), (1, 12.0), (2, 9.0)] {
            s.upsert(Candle {
                open_time_ms: t,
                open: 10.0,
                high: cl + 1.0,
                low: cl - 1.0,
                close: cl,
                volume: t as f64 + 1.0,
            });
        }
        let vp = Viewport {
            bar_spacing: 10.0,
            right_offset_bars: 0.0,
            price_min: 0.0,
            price_max: 1.0,
            plot: PlotRect {
                left: 0.0,
                top: 0.0,
                right: 100.0,
                bottom: 100.0,
            },
            bar_count: 3,
            scale: PriceScale::linear(),
        };
        let bars = collect_bars(&s, 1..3, &vp);
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].prev_close, 10.0);
        assert_eq!(bars[1].prev_close, 12.0);
        assert_eq!(price_extent(&bars, false), Some((8.0, 13.0)));
        assert_eq!(price_extent(&bars, true), Some((9.0, 12.0)));
        assert_eq!(max_volume(&bars), 3.0);
        assert_eq!(price_extent(&[], true), None);
    }
}
