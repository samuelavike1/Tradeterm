//! Series derived from the raw bars for some chart types: Heikin Ashi
//! (incremental) and the brick types Renko / Line break (rebuilt when the
//! source or their inputs change; they reflow anyway when a brick forms).

use crate::ChartType;
use crate::style::{BoxMethod, BrickSource, RenkoInputs, SymbolStyle};
use tt_core::{
    BoxSize, Bricks, Candle, CandleSeries, HeikinAshi, RenkoParams, line_break_with, renko_box,
    renko_with,
};

/// The brick inputs of a chart type (part of the cache key).
#[derive(Debug, Clone, Copy, PartialEq)]
enum BrickInputs {
    Renko(RenkoInputs),
    LineBreak { lines: u32 },
}

/// Everything the brick series depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BrickKey {
    inputs: BrickInputs,
    tick: Option<f64>,
    src_version: u64,
    src_len: usize,
    src_last: Option<Candle>,
}

#[derive(Debug, Clone, Default)]
pub struct Derived {
    ha: HeikinAshi,
    bricks: Bricks,
    key: Option<BrickKey>,
    /// Percentage (LTP) box size, held until the source version or the
    /// inputs change: (source version, inputs, tick, box size).
    held_box: Option<(u64, RenkoInputs, Option<f64>, f64)>,
}

impl Derived {
    /// Brings the derived series of `kind` up to date with `src`. Bricks
    /// rebuild only when the source or their inputs (from `style`, and the
    /// symbol's `tick` size) change.
    pub fn update(
        &mut self,
        kind: ChartType,
        style: &SymbolStyle,
        tick: Option<f64>,
        src: &CandleSeries,
    ) {
        let inputs = match kind {
            ChartType::HeikinAshi => {
                self.ha.sync(src);
                return;
            }
            ChartType::Renko => BrickInputs::Renko(style.renko.inputs),
            ChartType::LineBreak => BrickInputs::LineBreak {
                lines: style.line_break.lines,
            },
            _ => return,
        };
        let key = BrickKey {
            inputs,
            tick,
            src_version: src.version(),
            src_len: src.len(),
            src_last: src.last(),
        };
        if self.key != Some(key) {
            self.bricks = match inputs {
                BrickInputs::Renko(r) => {
                    let box_size = self.box_size(&r, tick, src);
                    renko_with(
                        src,
                        RenkoParams {
                            box_size,
                            ohlc: r.source == BrickSource::Ohlc,
                            wicks: r.wicks,
                        },
                    )
                }
                BrickInputs::LineBreak { lines } => {
                    line_break_with(src, lines.clamp(1, 100) as usize)
                }
            };
            self.key = Some(key);
        }
    }

    /// The series to display for `kind` (as of the last [`Self::update`]),
    /// or `None` to show the raw bars.
    pub fn series(&self, kind: ChartType) -> Option<&CandleSeries> {
        match kind {
            ChartType::HeikinAshi => Some(self.ha.series()),
            ChartType::Renko | ChartType::LineBreak => Some(&self.bricks.series),
            _ => None,
        }
    }

    /// Index of the first projection (forming) brick in the displayed
    /// series, for the brick types.
    pub fn projection_from(&self, kind: ChartType) -> Option<usize> {
        matches!(kind, ChartType::Renko | ChartType::LineBreak)
            .then_some(self.bricks.projection_from)
    }

    fn box_size(&mut self, r: &RenkoInputs, tick: Option<f64>, src: &CandleSeries) -> f64 {
        match r.method {
            BoxMethod::Atr => renko_box(src, BoxSize::Atr(r.atr_length.max(1) as usize), tick),
            BoxMethod::Traditional => renko_box(src, BoxSize::Fixed(r.box_size), tick),
            BoxMethod::PercentLtp => {
                if let Some((v, i, t, b)) = self.held_box
                    && v == src.version()
                    && i == *r
                    && t == tick
                    && !src.is_empty()
                {
                    return b;
                }
                let b = renko_box(src, BoxSize::Percent(r.percent), tick);
                if !src.is_empty() {
                    self.held_box = Some((src.version(), *r, tick, b));
                }
                b
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Derived {
        fn sync(
            &mut self,
            kind: ChartType,
            style: &SymbolStyle,
            tick: Option<f64>,
            src: &CandleSeries,
        ) -> Option<&CandleSeries> {
            self.update(kind, style, tick, src);
            self.series(kind)
        }
    }

    fn series(closes: &[f64]) -> CandleSeries {
        let mut s = CandleSeries::new();
        for (i, c) in closes.iter().enumerate() {
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

    #[test]
    fn renko_inputs_rebuild_once() {
        let s = series(&[100.0, 102.0, 104.0, 106.0]);
        let mut style = SymbolStyle::default();
        style.renko.inputs.method = BoxMethod::Traditional;
        style.renko.inputs.box_size = 2.0;
        let mut d = Derived::default();
        assert_eq!(d.sync(ChartType::Renko, &style, None, &s).unwrap().len(), 3);
        let key = d.key;
        d.sync(ChartType::Renko, &style, None, &s);
        assert_eq!(d.key, key, "unchanged inputs: cached");
        style.renko.inputs.box_size = 1.0;
        assert_eq!(d.sync(ChartType::Renko, &style, None, &s).unwrap().len(), 6);
        assert_eq!(d.projection_from(ChartType::Renko), Some(4));
        // Line break uses its number of lines.
        style.line_break.lines = 1;
        assert_eq!(
            d.sync(ChartType::LineBreak, &style, None, &s)
                .unwrap()
                .len(),
            3
        );
        assert_eq!(d.projection_from(ChartType::Candles), None);
    }

    #[test]
    fn percent_box_is_held_while_the_version_is_unchanged() {
        let mut s = series(&[100.0, 101.0, 102.0]);
        let mut style = SymbolStyle::default();
        style.renko.inputs.method = BoxMethod::PercentLtp;
        style.renko.inputs.percent = 1.0;
        let mut d = Derived::default();
        d.sync(ChartType::Renko, &style, Some(0.01), &s);
        let held = d.held_box.unwrap().3;
        assert_eq!(held, 1.02);
        // A live tick doubles the price: the box size stays.
        let mut last = s.last().unwrap();
        last.close = 204.0;
        s.upsert(last);
        d.sync(ChartType::Renko, &style, Some(0.01), &s);
        assert_eq!(d.held_box.unwrap().3, held);
    }
}
