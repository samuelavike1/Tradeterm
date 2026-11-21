//! The per-frame list of bars to draw.
//!
//! Drawing code never walks the series directly; it draws [`DrawBar`]s. When
//! zoomed in there is one per visible bar; when zoomed far out (B6) one per
//! pixel column, aggregated from a precomputed min/max pyramid. Either way the
//! renderers and auto-scaling see the same shape of data.

use crate::ChartType;
use crate::style::{PriceSource, SymbolStyle};
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
    /// The chart type's price source (close, hl2, …) of the bar (of the
    /// last bar inside an aggregated column); see [`price_source`].
    pub value: f64,
    /// `value` of the preceding bar (or column).
    pub prev_value: f64,
    /// Lowest / highest `value` inside an aggregated column (= `value` for
    /// single bars).
    pub value_min: f64,
    pub value_max: f64,
    pub volume: f64,
}

/// The value `source` picks from a bar.
#[inline]
pub fn source_value(source: PriceSource, open: f64, high: f64, low: f64, close: f64) -> f64 {
    match source {
        PriceSource::Close => close,
        PriceSource::Open => open,
        PriceSource::High => high,
        PriceSource::Low => low,
        PriceSource::Hl2 => (high + low) / 2.0,
        PriceSource::Hlc3 => (high + low + close) / 3.0,
        PriceSource::Ohlc4 => (open + high + low + close) / 4.0,
        PriceSource::Hlcc4 => (high + low + close + close) / 4.0,
    }
}

/// The price source a chart type plots (Close for the types without one).
pub fn price_source(style: &SymbolStyle, t: ChartType) -> PriceSource {
    match t {
        ChartType::Line | ChartType::LineWithMarkers | ChartType::StepLine => style.line(t).source,
        ChartType::Area => style.area.source,
        ChartType::Baseline => style.baseline.source,
        ChartType::Columns => style.columns.source,
        _ => PriceSource::Close,
    }
}

/// One `DrawBar` per bar in `range`, with `value` from `source`.
pub fn collect_bars(
    series: &CandleSeries,
    range: Range<usize>,
    vp: &Viewport,
    source: PriceSource,
) -> Vec<DrawBar> {
    let (o, h, l, c, v) = (
        series.opens(),
        series.highs(),
        series.lows(),
        series.closes(),
        series.volumes(),
    );
    let val = |i: usize| source_value(source, o[i], h[i], l[i], c[i]);
    range
        .map(|i| {
            let value = val(i);
            DrawBar {
                x: vp.x_of(i as f32),
                open: o[i],
                high: h[i],
                low: l[i],
                close: c[i],
                prev_close: if i > 0 { c[i - 1] } else { o[i] },
                value,
                prev_value: if i > 0 { val(i - 1) } else { o[i] },
                value_min: value,
                value_max: value,
                volume: v[i],
            }
        })
        .collect()
}

/// (min, max) price over the bars: high/low, or the source values only for
/// line-like charts ([`ChartType::uses_source_only`]).
pub fn price_extent(bars: &[DrawBar], source_only: bool) -> Option<(f64, f64)> {
    if bars.is_empty() {
        return None;
    }
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for b in bars {
        let (l, h) = if source_only {
            (b.value_min, b.value_max)
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
        let bars = collect_bars(&s, 1..3, &vp, PriceSource::Close);
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].prev_close, 10.0);
        assert_eq!(bars[1].prev_close, 12.0);
        assert_eq!(price_extent(&bars, false), Some((8.0, 13.0)));
        assert_eq!(price_extent(&bars, true), Some((9.0, 12.0)));
        assert_eq!(max_volume(&bars), 3.0);
        assert_eq!(price_extent(&[], true), None);
        // hl2 = close here (high/low are close ± 1); open = 10 for every bar.
        let hl2 = collect_bars(&s, 0..3, &vp, PriceSource::Hl2);
        assert_eq!(
            hl2.iter().map(|b| b.value).collect::<Vec<_>>(),
            [10.0, 12.0, 9.0]
        );
        let open = collect_bars(&s, 1..3, &vp, PriceSource::Open);
        assert_eq!(price_extent(&open, true), Some((10.0, 10.0)));
        let high = collect_bars(&s, 1..3, &vp, PriceSource::High);
        assert_eq!((high[1].value, high[1].prev_value), (10.0, 13.0));
    }

    #[test]
    fn sources_and_per_type_lookup() {
        let (o, h, l, c) = (1.0, 4.0, 0.0, 3.0);
        let v = |s| source_value(s, o, h, l, c);
        assert_eq!(v(PriceSource::Open), 1.0);
        assert_eq!(v(PriceSource::High), 4.0);
        assert_eq!(v(PriceSource::Low), 0.0);
        assert_eq!(v(PriceSource::Close), 3.0);
        assert_eq!(v(PriceSource::Hl2), 2.0);
        assert_eq!(v(PriceSource::Hlc3), 7.0 / 3.0);
        assert_eq!(v(PriceSource::Ohlc4), 2.0);
        assert_eq!(v(PriceSource::Hlcc4), 2.5);
        let mut st = SymbolStyle::default();
        st.step_line.source = PriceSource::Hl2;
        st.columns.source = PriceSource::Open;
        assert_eq!(price_source(&st, ChartType::StepLine), PriceSource::Hl2);
        assert_eq!(price_source(&st, ChartType::Line), PriceSource::Close);
        assert_eq!(price_source(&st, ChartType::Columns), PriceSource::Open);
        assert_eq!(price_source(&st, ChartType::Candles), PriceSource::Close);
    }
}
