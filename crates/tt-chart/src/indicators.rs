//! Indicators attached to a chart: a configured [`Study`], its colors and
//! visibility, and cached outputs that are updated incrementally.

use crate::scale::{format_price, format_volume};
use ecolor::Color32;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use tt_core::{CandleSeries, Placement, Plot, Study};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

// TradingView's default indicator colors.
const BLUE: Color32 = Color32::from_rgb(0x29, 0x62, 0xff);
const ORANGE: Color32 = Color32::from_rgb(0xff, 0x6d, 0x00);
const RED: Color32 = Color32::from_rgb(0xf2, 0x36, 0x45);
const TEAL: Color32 = Color32::from_rgb(0x08, 0x99, 0x81);
const PURPLE: Color32 = Color32::from_rgb(0x7e, 0x57, 0xc2);
const LIGHT_BLUE: Color32 = Color32::from_rgb(0x21, 0x96, 0xf3);
const DARK_RED: Color32 = Color32::from_rgb(0xb7, 0x1c, 0x1c);
const GREEN: Color32 = Color32::from_rgb(0x26, 0xa6, 0x9a);
const SALMON: Color32 = Color32::from_rgb(0xff, 0x52, 0x52);

/// Default colors, one per output (histogram / volume outputs use their
/// color for the positive side and the theme for the rest).
pub fn default_colors(study: &Study) -> Vec<Color32> {
    match study {
        Study::Volume { .. } => vec![TEAL, LIGHT_BLUE],
        Study::Sma { .. } | Study::Ema { .. } | Study::Vwap | Study::Obv => vec![BLUE],
        Study::BollingerBands { .. } => vec![LIGHT_BLUE, RED, TEAL],
        Study::Rsi { .. } => vec![PURPLE],
        Study::Macd { .. } => vec![GREEN, BLUE, ORANGE],
        Study::Stochastic { .. } => vec![BLUE, ORANGE],
        Study::Atr { .. } => vec![DARK_RED],
    }
}

/// Negative-side color of a histogram whose positive color is [`GREEN`].
pub(crate) fn histogram_down() -> Color32 {
    SALMON
}

#[derive(Debug, Clone, Default)]
struct Cache {
    study: Option<Study>,
    version: u64,
    len: usize,
    /// Last bar when computed: unchanged series ⇒ nothing to do.
    last: Option<tt_core::Candle>,
    outputs: Vec<Vec<f64>>,
}

#[derive(Debug, Clone)]
pub struct Indicator {
    /// Unique at runtime (not persisted); identifies the indicator in events.
    pub id: u64,
    pub study: Study,
    pub visible: bool,
    /// One color per output; see [`default_colors`].
    pub colors: Vec<Color32>,
    /// Relative height of its pane (pane studies; the price pane is 3).
    pub pane_weight: f32,
    /// Its pane is collapsed to a strip with just the legend.
    pub pane_collapsed: bool,
    cache: Cache,
}

impl Indicator {
    pub fn new(study: Study) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            colors: default_colors(&study),
            study,
            visible: true,
            pane_weight: 1.0,
            pane_collapsed: false,
            cache: Cache::default(),
        }
    }

    pub fn placement(&self) -> Placement {
        self.study.placement()
    }

    /// Color of output `k` (falls back to the default if the list is short).
    pub fn color(&self, k: usize) -> Color32 {
        self.colors
            .get(k)
            .copied()
            .or_else(|| default_colors(&self.study).get(k).copied())
            .unwrap_or(BLUE)
    }

    /// Outputs computed by the last [`Self::sync`].
    pub fn outputs(&self) -> &[Vec<f64>] {
        &self.cache.outputs
    }

    /// Whether output `k` is drawn (Volume MA is optional).
    pub fn output_enabled(&self, k: usize) -> bool {
        match self.study {
            Study::Volume { show_ma, .. } => k == 0 || show_ma,
            _ => true,
        }
    }

    /// Brings outputs up to date with `src`: a full recompute after inputs
    /// or history changed, otherwise only the last bar(s).
    pub fn sync(&mut self, src: &CandleSeries) {
        let c = &mut self.cache;
        let same_study = c.study.as_ref() == Some(&self.study);
        if same_study && c.version == src.version() && c.len == src.len() && c.last == src.last() {
            return;
        }
        let full = !same_study
            || c.version != src.version()
            || src.len() < c.len
            || self.study.recompute_fully();
        // Otherwise only the previously last bar (it may have been updated)
        // and newly appended bars need computing.
        let from = if full { 0 } else { c.len.saturating_sub(1) };
        self.study.compute(src, &mut c.outputs, from);
        c.study = Some(self.study.clone());
        c.version = src.version();
        c.len = src.len();
        c.last = src.last();
    }

    /// (min, max) of the drawn outputs over `range` (histograms include 0).
    pub fn extent(&self, range: Range<usize>) -> Option<(f64, f64)> {
        self.extent_sampled(range, 1)
    }

    /// [`Self::extent`] over every `stride`-th bar (aligned to multiples of
    /// `stride`, plus the last one), i.e. the points actually drawn when
    /// zoomed far out. Keeps auto-fit O(pixels) instead of O(bars).
    pub fn extent_sampled(&self, range: Range<usize>, stride: usize) -> Option<(f64, f64)> {
        let stride = stride.max(1);
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for (k, (out, (_, plot))) in self.outputs().iter().zip(self.study.outputs()).enumerate() {
            if !self.output_enabled(k) {
                continue;
            }
            let end = range.end.min(out.len());
            let start = range.start.min(end) / stride * stride;
            let last = end.checked_sub(1);
            for i in (start..end).step_by(stride).chain(last) {
                let v = out[i];
                if v.is_finite() {
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            if *plot != Plot::Line && lo.is_finite() {
                lo = lo.min(0.0);
                hi = hi.max(0.0);
            }
        }
        (lo <= hi).then_some((lo, hi))
    }
}

/// Formats a study value for legends and axis labels.
pub fn format_value(study: &Study, v: f64, price_precision: usize) -> String {
    if !v.is_finite() {
        return "∅".into();
    }
    match study {
        Study::Volume { .. } | Study::Obv => {
            if v < 0.0 {
                format!("−{}", format_volume(-v))
            } else {
                format_volume(v)
            }
        }
        Study::Rsi { .. } | Study::Stochastic { .. } => format!("{v:.2}"),
        _ => format_price(v, price_precision),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tt_core::{Candle, Source, StudyKind};

    fn series(n: usize) -> CandleSeries {
        let mut s = CandleSeries::new();
        for i in 0..n {
            let c = 100.0 + (i as f64 * 0.3).sin() * 5.0;
            s.upsert(Candle {
                open_time_ms: i as i64 * 60_000,
                open: c,
                high: c + 1.0,
                low: c - 1.0,
                close: c,
                volume: 5.0,
            });
        }
        s
    }

    #[test]
    fn sync_recomputes_on_input_change_and_updates_tail() {
        let mut s = series(50);
        let mut ind = Indicator::new(Study::Sma {
            length: 5,
            source: Source::Close,
        });
        ind.sync(&s);
        assert_eq!(ind.outputs()[0].len(), 50);
        let mut last = s.last().unwrap();
        last.close = 1000.0;
        s.upsert(last);
        ind.sync(&s);
        let closes = &s.closes()[45..50];
        assert!((ind.outputs()[0][49] - closes.iter().sum::<f64>() / 5.0).abs() < 1e-9);
        ind.study = Study::Sma {
            length: 10,
            source: Source::Close,
        };
        ind.sync(&s);
        assert!(ind.outputs()[0][8].is_nan() && ind.outputs()[0][9].is_finite());
    }

    #[test]
    fn unchanged_series_is_not_recomputed() {
        let s = series(50);
        let mut ind = Indicator::new(StudyKind::Rsi.default_study());
        ind.sync(&s);
        // Poison the cache: a real recompute would overwrite it.
        ind.cache.outputs[0][49] = -1.0;
        ind.sync(&s);
        assert_eq!(ind.outputs()[0][49], -1.0, "no change, no recompute");
        let mut s2 = s.clone();
        let mut last = s2.last().unwrap();
        last.close += 1.0;
        s2.upsert(last);
        ind.sync(&s2);
        assert_ne!(ind.outputs()[0][49], -1.0, "a tick recomputes");
    }

    #[test]
    fn extent_includes_zero_for_histograms_and_skips_disabled_outputs() {
        let s = series(100);
        let mut macd = Indicator::new(StudyKind::Macd.default_study());
        macd.sync(&s);
        let (lo, hi) = macd.extent(60..100).unwrap();
        assert!(lo <= 0.0 && hi >= 0.0);
        let mut vol = Indicator::new(StudyKind::Volume.default_study());
        vol.sync(&s);
        assert_eq!(vol.extent(0..100), Some((0.0, 5.0)));
        assert!(!vol.output_enabled(1));
        assert_eq!(format_value(&vol.study, 1500.0, 2), "1.50K");
    }
}
