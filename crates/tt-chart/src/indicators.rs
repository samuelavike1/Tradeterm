//! Indicators attached to a chart: a configured [`Study`], its style and
//! visibility, and cached outputs that are updated incrementally.

use crate::indicator_style::{IndicatorStyle, ResolvedPlot};
use crate::scale::{format_price, format_volume};
use crate::style::Rgba;
use ecolor::Color32;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use tt_core::{CandleSeries, InputValue, Placement, PlotDef, Study, ValueFormat};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

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
    /// Per-plot / level / fill style. Not part of the compute cache: editing
    /// it only repaints.
    pub style: IndicatorStyle,
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
            study,
            visible: true,
            style: IndicatorStyle::default(),
            pane_weight: 1.0,
            pane_collapsed: false,
            cache: Cache::default(),
        }
    }

    pub fn placement(&self) -> Placement {
        self.study.placement()
    }

    /// Definition of output `k`.
    #[inline]
    pub fn plot_def(&self, k: usize) -> Option<&'static PlotDef> {
        self.study.meta().plots.get(k)
    }

    /// Effective style of output `k` (the metadata default for an unknown
    /// index is never needed by callers that iterate `meta().plots`).
    #[inline]
    pub fn plot_style(&self, k: usize) -> Option<ResolvedPlot> {
        self.plot_def(k).map(|d| self.style.plot(d))
    }

    /// Whether output `k` is drawn: enabled by the inputs (Volume MA needs
    /// "show MA") and visible in the style.
    pub fn output_enabled(&self, k: usize) -> bool {
        self.plot_def(k).is_some_and(|d| {
            d.enabled_if.is_none_or(|f| f(&self.study)) && self.style.plot(d).visible
        })
    }

    /// Main color of output `k`. Compatibility accessor for the settings
    /// dialog, which edits one color per output.
    pub fn color(&self, k: usize) -> Color32 {
        self.plot_style(k)
            .map_or(Color32::from_rgb(0x29, 0x62, 0xff), |p| {
                p.color().to_color32()
            })
    }

    /// Sets the main color of output `k` (see [`Self::color`]).
    pub fn set_color(&mut self, k: usize, c: Color32) {
        if let Some(d) = self.plot_def(k) {
            self.style.set_plot_color(d.id, 0, Rgba::from_color32(c));
        }
    }

    /// Plot offset in bars (the study's `offset` input; 0 if it has none):
    /// value `i` is drawn at bar `i + offset`.
    #[inline]
    pub fn offset(&self) -> i64 {
        match self.study.get("offset") {
            Some(InputValue::Int(o)) => o,
            _ => 0,
        }
    }

    /// Value of output `k` shown at bar `bar` (honours the offset).
    pub fn value_at(&self, k: usize, bar: usize) -> Option<f64> {
        let j = usize::try_from(bar as i64 - self.offset()).ok()?;
        self.outputs().get(k)?.get(j).copied()
    }

    /// Formats a value with this indicator's precision.
    pub fn format(&self, v: f64, price_precision: usize) -> String {
        format_value_with(&self.study, self.style.precision(), v, price_precision)
    }

    /// (min, max) of the visible levels, for the pane's auto-fit.
    pub fn hline_extent(&self) -> Option<(f64, f64)> {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for def in self.study.meta().hlines {
            let h = self.style.hline(def);
            if h.visible {
                lo = lo.min(h.value);
                hi = hi.max(h.value);
            }
        }
        (lo <= hi).then_some((lo, hi))
    }

    /// Outputs computed by the last [`Self::sync`].
    pub fn outputs(&self) -> &[Vec<f64>] {
        &self.cache.outputs
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

    /// (min, max) of the drawn outputs over the bars `range` (columns,
    /// histograms and areas include 0).
    pub fn extent(&self, range: Range<usize>) -> Option<(f64, f64)> {
        self.extent_sampled(range, 1)
    }

    /// [`Self::extent`] over every `stride`-th bar (aligned to multiples of
    /// `stride`, plus the last one), i.e. the points actually drawn when
    /// zoomed far out. Keeps auto-fit O(pixels) instead of O(bars).
    pub fn extent_sampled(&self, range: Range<usize>, stride: usize) -> Option<(f64, f64)> {
        let stride = stride.max(1);
        let off = self.offset();
        // Bars → value indices.
        let shift = |b: usize| (b as i64 - off).max(0) as usize;
        let range = shift(range.start)..shift(range.end);
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for (k, (out, def)) in self
            .outputs()
            .iter()
            .zip(self.study.meta().plots)
            .enumerate()
        {
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
            if self.style.plot(def).from_zero() && lo.is_finite() {
                lo = lo.min(0.0);
                hi = hi.max(0.0);
            }
        }
        (lo <= hi).then_some((lo, hi))
    }
}

/// Formats a study value for legends and axis labels (default precision).
pub fn format_value(study: &Study, v: f64, price_precision: usize) -> String {
    format_value_with(study, None, v, price_precision)
}

/// Formats a study value; `precision` (decimals) overrides the study's
/// default format. Volume formats stay abbreviated (`1.234K`).
pub fn format_value_with(
    study: &Study,
    precision: Option<u8>,
    v: f64,
    price_precision: usize,
) -> String {
    if !v.is_finite() {
        return "∅".into();
    }
    match (study.meta().format, precision) {
        (ValueFormat::Volume, p) => {
            let abs = match p {
                None => format_volume(v.abs()),
                Some(d) => abbreviated(v.abs(), d as usize),
            };
            if v < 0.0 { format!("−{abs}") } else { abs }
        }
        (_, Some(d)) => format_price(v, d as usize),
        (ValueFormat::Fixed(d), None) => format!("{v:.*}", d as usize),
        (ValueFormat::Price, None) => format_price(v, price_precision),
    }
}

/// `format_volume` with `d` decimals.
fn abbreviated(v: f64, d: usize) -> String {
    let a = v.abs();
    if a >= 1e9 {
        format!("{:.d$}B", v / 1e9)
    } else if a >= 1e6 {
        format!("{:.d$}M", v / 1e6)
    } else if a >= 1e3 {
        format!("{:.d$}K", v / 1e3)
    } else {
        format!("{v:.d$}")
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

    /// Style edits only repaint: they never invalidate the outputs.
    #[test]
    fn style_edits_do_not_recompute() {
        let s = series(50);
        let mut ind = Indicator::new(StudyKind::Macd.default_study());
        ind.sync(&s);
        ind.cache.outputs[1][49] = -1.0;
        ind.set_color(1, Color32::RED);
        ind.style.plot_mut("hist").plot_type = Some(tt_core::PlotType::Line);
        ind.style.precision = Some(4);
        ind.sync(&s);
        assert_eq!(ind.outputs()[1][49], -1.0);
        assert_eq!(ind.color(1), Color32::RED);
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
        // Hidden in the style: not drawn, not fitted.
        let mut rsi = Indicator::new(StudyKind::Rsi.default_study());
        rsi.sync(&s);
        assert!(rsi.extent(0..100).is_some());
        rsi.style.plot_mut("rsi").visible = Some(false);
        assert!(!rsi.output_enabled(0));
        assert_eq!(rsi.extent(0..100), None);
        assert_eq!(rsi.hline_extent(), Some((30.0, 70.0)));
        rsi.style.hline_mut("upper").value = Some(85.0);
        rsi.style.hline_mut("lower").visible = Some(false);
        assert_eq!(rsi.hline_extent(), Some((50.0, 85.0)));
    }

    #[test]
    fn precision_overrides_the_format() {
        let mut rsi = Indicator::new(StudyKind::Rsi.default_study());
        assert_eq!(rsi.format(12.3456, 5), "12.35");
        rsi.style.precision = Some(0);
        assert_eq!(rsi.format(12.3456, 5), "12");
        let mut obv = Indicator::new(StudyKind::Obv.default_study());
        obv.style.precision = Some(3);
        assert_eq!(obv.format(-1234.5, 2), "−1.234K");
        let mut sma = Indicator::new(StudyKind::Sma.default_study());
        assert_eq!(sma.format(1234.5, 1), "1,234.5");
        sma.style.precision = Some(3);
        assert_eq!(sma.format(1234.5, 1), "1,234.500");
    }

    /// The study metadata describes what this module and the renderer do
    /// (enabled outputs, value format).
    #[test]
    fn study_meta_drives_defaults() {
        for kind in StudyKind::ALL {
            let study = kind.default_study();
            let meta = study.meta();
            let ind = Indicator::new(study.clone());
            for (k, plot) in meta.plots.iter().enumerate() {
                let enabled = plot.enabled_if.is_none_or(|f| f(&study)) && plot.default.visible;
                assert_eq!(enabled, ind.output_enabled(k), "{kind:?} {}", plot.id);
                assert_eq!(
                    ind.plot_style(k).unwrap().color().to_core(),
                    plot.default.color
                );
            }
            let want = match meta.format {
                ValueFormat::Volume => format_volume(1234.0),
                ValueFormat::Fixed(d) => format!("{:.*}", d as usize, 1234.0),
                ValueFormat::Price => format_price(1234.0, 3),
            };
            assert_eq!(format_value(&study, 1234.0, 3), want, "{kind:?}");
        }
    }
}
