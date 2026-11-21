//! Technical indicators ("studies"), computed incrementally.
//!
//! Each [`Study`] produces one or more output series aligned with the source
//! bars (NaN where undefined). [`Study::compute`] fills outputs from index
//! `from` onward, reading earlier outputs for recursive formulas (EMA, RSI…),
//! so a live tick on the forming bar recomputes only that bar.
//!
//! Layout: this file holds the [`Study`] enum and dispatch; [`ma`] the shared
//! moving averages; [`meta`] the declarative metadata types; and one file
//! per study family owns its compute function, its `META` and its tests.

// Index loops mirror the formulas (several series indexed together).
#![allow(clippy::needless_range_loop)]

pub mod ma;
pub mod meta;

mod atr;
mod bb;
mod ma_simple;
mod macd;
mod obv;
mod rsi;
mod stoch;
mod volume;
mod vwap;

use crate::CandleSeries;
use meta::{InputKind, InputValue, StudyMeta};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Price input for a study.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Source {
    Open,
    High,
    Low,
    #[default]
    Close,
    Hl2,
    Hlc3,
    Ohlc4,
    /// (h + l + 2c) / 4.
    Hlcc4,
}

impl Source {
    pub const ALL: [Source; 8] = [
        Source::Open,
        Source::High,
        Source::Low,
        Source::Close,
        Source::Hl2,
        Source::Hlc3,
        Source::Ohlc4,
        Source::Hlcc4,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Source::Open => "open",
            Source::High => "high",
            Source::Low => "low",
            Source::Close => "close",
            Source::Hl2 => "hl2",
            Source::Hlc3 => "hlc3",
            Source::Ohlc4 => "ohlc4",
            Source::Hlcc4 => "hlcc4",
        }
    }

    pub fn value(self, s: &CandleSeries, i: usize) -> f64 {
        let (o, h, l, c) = (s.opens()[i], s.highs()[i], s.lows()[i], s.closes()[i]);
        match self {
            Source::Open => o,
            Source::High => h,
            Source::Low => l,
            Source::Close => c,
            Source::Hl2 => (h + l) / 2.0,
            Source::Hlc3 => (h + l + c) / 3.0,
            Source::Ohlc4 => (o + h + l + c) / 4.0,
            Source::Hlcc4 => (h + l + 2.0 * c) / 4.0,
        }
    }
}

/// Where a study is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// On the price scale of the main pane (moving averages, bands…).
    Overlay,
    /// Bottom of the main pane with its own scale (Volume, like TradingView).
    VolumeOverlay,
    /// Its own pane below the chart.
    Pane,
}

/// How an output is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plot {
    Line,
    /// Columns colored by sign (MACD histogram).
    Histogram,
    /// Volume columns colored by bar direction.
    VolumeColumns,
}

/// Catalog entry kind (one per built-in study).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StudyKind {
    Volume,
    Sma,
    Ema,
    BollingerBands,
    Vwap,
    Rsi,
    Macd,
    Stochastic,
    Atr,
    Obv,
}

impl StudyKind {
    pub const ALL: [StudyKind; 10] = [
        StudyKind::Atr,
        StudyKind::BollingerBands,
        StudyKind::Macd,
        StudyKind::Sma,
        StudyKind::Ema,
        StudyKind::Obv,
        StudyKind::Rsi,
        StudyKind::Stochastic,
        StudyKind::Volume,
        StudyKind::Vwap,
    ];

    /// Static metadata (inputs, plots, levels, fills).
    pub fn meta(self) -> &'static StudyMeta {
        match self {
            StudyKind::Volume => &volume::META,
            StudyKind::Sma => &ma_simple::SMA_META,
            StudyKind::Ema => &ma_simple::EMA_META,
            StudyKind::BollingerBands => &bb::META,
            StudyKind::Vwap => &vwap::META,
            StudyKind::Rsi => &rsi::META,
            StudyKind::Macd => &macd::META,
            StudyKind::Stochastic => &stoch::META,
            StudyKind::Atr => &atr::META,
            StudyKind::Obv => &obv::META,
        }
    }

    /// Full name as in TradingView's indicator list.
    pub fn name(self) -> &'static str {
        self.meta().name
    }

    pub fn default_study(self) -> Study {
        match self {
            StudyKind::Volume => Study::Volume {
                ma_length: 20,
                color_by_prev_close: false,
                show_ma: false,
            },
            StudyKind::Sma => Study::Sma {
                length: 9,
                source: Source::Close,
            },
            StudyKind::Ema => Study::Ema {
                length: 9,
                source: Source::Close,
            },
            StudyKind::BollingerBands => Study::BollingerBands {
                length: 20,
                mult: 2.0,
                source: Source::Close,
            },
            StudyKind::Vwap => Study::Vwap,
            StudyKind::Rsi => Study::Rsi {
                length: 14,
                source: Source::Close,
            },
            StudyKind::Macd => Study::Macd {
                fast: 12,
                slow: 26,
                signal: 9,
                source: Source::Close,
            },
            StudyKind::Stochastic => Study::Stochastic {
                k: 14,
                k_smooth: 1,
                d: 3,
            },
            StudyKind::Atr => Study::Atr { length: 14 },
            StudyKind::Obv => Study::Obv,
        }
    }
}

/// A configured study (its inputs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Study {
    Volume {
        ma_length: u32,
        /// Color bars by close vs previous close instead of close vs open.
        color_by_prev_close: bool,
        show_ma: bool,
    },
    Sma {
        length: u32,
        source: Source,
    },
    Ema {
        length: u32,
        source: Source,
    },
    BollingerBands {
        length: u32,
        mult: f64,
        source: Source,
    },
    /// Session VWAP, reset at 00:00 UTC.
    Vwap,
    Rsi {
        length: u32,
        source: Source,
    },
    Macd {
        fast: u32,
        slow: u32,
        signal: u32,
        source: Source,
    },
    Stochastic {
        k: u32,
        k_smooth: u32,
        d: u32,
    },
    Atr {
        length: u32,
    },
    Obv,
}

impl Study {
    pub fn kind(&self) -> StudyKind {
        match self {
            Study::Volume { .. } => StudyKind::Volume,
            Study::Sma { .. } => StudyKind::Sma,
            Study::Ema { .. } => StudyKind::Ema,
            Study::BollingerBands { .. } => StudyKind::BollingerBands,
            Study::Vwap => StudyKind::Vwap,
            Study::Rsi { .. } => StudyKind::Rsi,
            Study::Macd { .. } => StudyKind::Macd,
            Study::Stochastic { .. } => StudyKind::Stochastic,
            Study::Atr { .. } => StudyKind::Atr,
            Study::Obv => StudyKind::Obv,
        }
    }

    /// Static metadata of this study's kind.
    pub fn meta(&self) -> &'static StudyMeta {
        self.kind().meta()
    }

    pub fn placement(&self) -> Placement {
        self.meta().placement
    }

    /// Current value of input `id` (see `StudyMeta::inputs`).
    pub fn get(&self, id: &str) -> Option<InputValue> {
        match self.kind() {
            StudyKind::Volume => volume::get(self, id),
            StudyKind::Sma | StudyKind::Ema => ma_simple::get(self, id),
            StudyKind::BollingerBands => bb::get(self, id),
            StudyKind::Vwap => vwap::get(self, id),
            StudyKind::Rsi => rsi::get(self, id),
            StudyKind::Macd => macd::get(self, id),
            StudyKind::Stochastic => stoch::get(self, id),
            StudyKind::Atr => atr::get(self, id),
            StudyKind::Obv => None,
        }
    }

    /// Sets input `id`, clamped to its range. `false` if there is no such
    /// input or the value has the wrong type.
    pub fn set(&mut self, id: &str, v: InputValue) -> bool {
        let Some(v) = self.meta().input(id).and_then(|d| d.kind.clamp(v)) else {
            return false;
        };
        match self.kind() {
            StudyKind::Volume => volume::set(self, id, v),
            StudyKind::Sma | StudyKind::Ema => ma_simple::set(self, id, v),
            StudyKind::BollingerBands => bb::set(self, id, v),
            StudyKind::Vwap => vwap::set(self, id, v),
            StudyKind::Rsi => rsi::set(self, id, v),
            StudyKind::Macd => macd::set(self, id, v),
            StudyKind::Stochastic => stoch::set(self, id, v),
            StudyKind::Atr => atr::set(self, id, v),
            StudyKind::Obv => false,
        }
    }

    /// Legend title: the short title, then the status-line inputs in order,
    /// e.g. `MA 9 close`, `BB 20 2 close`.
    pub fn legend_title(&self) -> String {
        let m = self.meta();
        let mut s = String::from(m.short);
        for def in m.inputs.iter().filter(|d| d.in_status_line) {
            let Some(v) = self.get(def.id) else { continue };
            // Writing to a String cannot fail.
            let _ = match (v, def.kind) {
                (InputValue::Int(x), _) => write!(s, " {x}"),
                (InputValue::Float(x), _) => write!(s, " {x}"),
                (InputValue::Choice(i), InputKind::Choice(opts)) => {
                    write!(s, " {}", opts.get(i as usize).copied().unwrap_or("?"))
                }
                (InputValue::Source(src), _) => write!(s, " {}", src.label()),
                (InputValue::Timeframe(Some(tf)), _) => write!(s, " {}", tf.label()),
                _ => Ok(()),
            };
        }
        s
    }

    /// Output names and plot styles, in output order.
    pub fn outputs(&self) -> &'static [(&'static str, Plot)] {
        match self {
            Study::Volume { .. } => &[("Volume", Plot::VolumeColumns), ("Volume MA", Plot::Line)],
            Study::Sma { .. } | Study::Ema { .. } | Study::Vwap => &[("Value", Plot::Line)],
            Study::BollingerBands { .. } => &[
                ("Basis", Plot::Line),
                ("Upper", Plot::Line),
                ("Lower", Plot::Line),
            ],
            Study::Rsi { .. } => &[("RSI", Plot::Line)],
            Study::Macd { .. } => &[
                ("Histogram", Plot::Histogram),
                ("MACD", Plot::Line),
                ("Signal", Plot::Line),
            ],
            Study::Stochastic { .. } => &[("%K", Plot::Line), ("%D", Plot::Line)],
            Study::Atr { .. } => &[("ATR", Plot::Line)],
            Study::Obv => &[("OBV", Plot::Line)],
        }
    }

    /// Fixed scale and guide lines for bounded oscillators: (min, max, levels).
    pub fn bounds(&self) -> Option<(f64, f64, &'static [f64])> {
        match self {
            Study::Rsi { .. } => Some((0.0, 100.0, &[30.0, 50.0, 70.0])),
            Study::Stochastic { .. } => Some((0.0, 100.0, &[20.0, 50.0, 80.0])),
            _ => None,
        }
    }

    /// Bars needed before the first defined value (for backfill decisions).
    pub fn warmup(&self) -> usize {
        match self {
            Study::Volume { ma_length, .. } => *ma_length as usize,
            Study::Sma { length, .. } | Study::Ema { length, .. } => *length as usize,
            Study::BollingerBands { length, .. } | Study::Rsi { length, .. } => *length as usize,
            Study::Macd { slow, signal, .. } => (*slow + *signal) as usize,
            Study::Stochastic { k, k_smooth, d } => (k + k_smooth + d) as usize,
            Study::Atr { length } => *length as usize,
            Study::Vwap | Study::Obv => 1,
        }
    }

    /// Fills `out` (one Vec per output, resized to `src.len()`) from index
    /// `from` onward. Earlier values must already be correct.
    pub fn compute(&self, src: &CandleSeries, out: &mut Vec<Vec<f64>>, from: usize) {
        let n = src.len();
        let k = self.meta().plots.len();
        out.resize_with(k, Vec::new);
        for o in out.iter_mut() {
            o.resize(n, f64::NAN);
        }
        let from = from.min(n);
        match *self {
            Study::Volume { ma_length, .. } => volume::compute(src, out, from, ma_length),
            Study::Sma { length, source } => ma_simple::sma(src, out, from, length, source),
            Study::Ema { length, source } => ma_simple::ema(src, out, from, length, source),
            Study::BollingerBands {
                length,
                mult,
                source,
            } => bb::compute(src, out, from, length, mult, source),
            Study::Vwap => vwap::compute(src, out, from),
            Study::Rsi { length, source } => rsi::compute(src, out, length, source),
            Study::Macd {
                fast,
                slow,
                signal,
                source,
            } => macd::compute(src, out, from, fast, slow, signal, source),
            Study::Stochastic { k, k_smooth, d } => stoch::compute(src, out, k, k_smooth, d),
            Study::Atr { length } => atr::compute(src, out, from, length),
            Study::Obv => obv::compute(src, out, from),
        }
    }

    /// Outputs that need a full recompute even for a tail update.
    pub fn recompute_fully(&self) -> bool {
        matches!(
            self,
            Study::Rsi { .. } | Study::Macd { .. } | Study::Stochastic { .. }
        )
    }
}

fn split2(out: &mut [Vec<f64>]) -> (&mut Vec<f64>, &mut Vec<f64>) {
    let (a, b) = out.split_at_mut(1);
    (&mut a[0], &mut b[0])
}

/// An already-clamped `Int` input as a `u32` field.
fn as_u32(x: i64) -> u32 {
    x.clamp(0, u32::MAX as i64) as u32
}

#[cfg(test)]
pub(crate) mod test_util {
    use crate::{Candle, CandleSeries};

    pub fn series(closes: &[f64]) -> CandleSeries {
        let mut s = CandleSeries::new();
        for (i, c) in closes.iter().enumerate() {
            s.upsert(Candle {
                open_time_ms: i as i64 * 60_000,
                open: c - 0.5,
                high: c + 1.0,
                low: c - 1.0,
                close: *c,
                volume: 10.0 + i as f64,
            });
        }
        s
    }

    pub fn run(study: &super::Study, s: &CandleSeries) -> Vec<Vec<f64>> {
        let mut out = Vec::new();
        study.compute(s, &mut out, 0);
        out
    }

    pub fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }
}

#[cfg(test)]
mod tests {
    use super::meta::{FillEdge, PlotKind};
    use super::test_util::*;
    use super::*;

    #[test]
    fn incremental_tail_matches_full() {
        for kind in StudyKind::ALL {
            let study = kind.default_study();
            let mut s = series(
                &(0..120)
                    .map(|i| 100.0 + (i as f64 * 0.37).sin() * 7.0)
                    .collect::<Vec<_>>(),
            );
            let mut out = Vec::new();
            study.compute(&s, &mut out, 0);
            // Live tick on the last bar, then a new bar.
            for step in 0..4 {
                let mut c = s.last().unwrap();
                if step % 2 == 1 {
                    c.open_time_ms += 60_000;
                }
                c.close += 1.3;
                c.high = c.high.max(c.close);
                c.volume += 2.0;
                s.upsert(c);
                let from = if study.recompute_fully() {
                    0
                } else {
                    s.len() - 2
                };
                study.compute(&s, &mut out, from);
                let full = run(&study, &s);
                for (k, o) in full.iter().enumerate() {
                    assert!(
                        o[s.len() - 1].is_finite(),
                        "{kind:?} output {k} undefined at the last bar"
                    );
                }
                for (k, (a, b)) in out.iter().zip(&full).enumerate() {
                    for i in 0..s.len() {
                        assert!(
                            (a[i].is_nan() && b[i].is_nan()) || close(a[i], b[i]),
                            "{kind:?} output {k} bar {i}: {} vs {}",
                            a[i],
                            b[i]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn serde_round_trip() {
        let st = Study::BollingerBands {
            length: 20,
            mult: 2.5,
            source: Source::Hlc3,
        };
        let json = serde_json::to_string(&st).unwrap();
        assert!(json.contains("\"type\":\"BollingerBands\""), "{json}");
        assert_eq!(serde_json::from_str::<Study>(&json).unwrap(), st);
    }

    #[test]
    fn legacy_json_still_loads() {
        // The current on-disk format, one sample per variant.
        let samples = [
            (
                r#"{"type":"Volume","ma_length":20,"color_by_prev_close":true,"show_ma":false}"#,
                Study::Volume {
                    ma_length: 20,
                    color_by_prev_close: true,
                    show_ma: false,
                },
            ),
            (
                r#"{"type":"Sma","length":9,"source":"Close"}"#,
                StudyKind::Sma.default_study(),
            ),
            (
                r#"{"type":"Ema","length":21,"source":"Hl2"}"#,
                Study::Ema {
                    length: 21,
                    source: Source::Hl2,
                },
            ),
            (
                r#"{"type":"BollingerBands","length":20,"mult":2.0,"source":"Close"}"#,
                StudyKind::BollingerBands.default_study(),
            ),
            (r#"{"type":"Vwap"}"#, Study::Vwap),
            (
                r#"{"type":"Rsi","length":14,"source":"Close"}"#,
                StudyKind::Rsi.default_study(),
            ),
            (
                r#"{"type":"Macd","fast":12,"slow":26,"signal":9,"source":"Close"}"#,
                StudyKind::Macd.default_study(),
            ),
            (
                r#"{"type":"Stochastic","k":14,"k_smooth":1,"d":3}"#,
                StudyKind::Stochastic.default_study(),
            ),
            (r#"{"type":"Atr","length":14}"#, Study::Atr { length: 14 }),
            (r#"{"type":"Obv"}"#, Study::Obv),
        ];
        for (json, want) in samples {
            assert_eq!(serde_json::from_str::<Study>(json).unwrap(), want, "{json}");
        }
    }

    #[test]
    fn hlcc4_source() {
        let s = series(&[10.0]);
        // h 11, l 9, c 10 → (11 + 9 + 20) / 4
        assert!(close(Source::Hlcc4.value(&s, 0), 10.0));
        assert_eq!(Source::Hlcc4.label(), "hlcc4");
    }

    #[test]
    fn legend_titles_unchanged() {
        let t = |s: Study| s.legend_title();
        assert_eq!(t(StudyKind::Volume.default_study()), "Vol");
        assert_eq!(t(StudyKind::Sma.default_study()), "MA 9 close");
        assert_eq!(t(StudyKind::Ema.default_study()), "EMA 9 close");
        assert_eq!(
            t(StudyKind::BollingerBands.default_study()),
            "BB 20 2 close"
        );
        assert_eq!(
            t(Study::BollingerBands {
                length: 10,
                mult: 2.5,
                source: Source::Hlc3
            }),
            "BB 10 2.5 hlc3"
        );
        assert_eq!(t(Study::Vwap), "VWAP Session");
        assert_eq!(t(StudyKind::Rsi.default_study()), "RSI 14 close");
        assert_eq!(t(StudyKind::Macd.default_study()), "MACD 12 26 9 close");
        assert_eq!(t(StudyKind::Stochastic.default_study()), "Stoch 14 1 3");
        assert_eq!(t(StudyKind::Atr.default_study()), "ATR 14");
        assert_eq!(t(Study::Obv), "OBV");
    }

    #[test]
    fn meta_matches_studies() {
        for kind in StudyKind::ALL {
            let m = kind.meta();
            let study = kind.default_study();
            assert_eq!(m.kind, kind);
            // Plots are the outputs, in order and of the same kind.
            let outs = study.outputs();
            assert_eq!(m.plots.len(), outs.len(), "{kind:?}");
            for (p, (_, plot)) in m.plots.iter().zip(outs) {
                let want = match plot {
                    Plot::Line => PlotKind::Value,
                    Plot::Histogram => PlotKind::SignedColumns,
                    Plot::VolumeColumns => PlotKind::VolumeColumns,
                };
                assert_eq!(p.kind, want, "{kind:?} {}", p.id);
                if let Some(first) = p.palette.first() {
                    assert_eq!(p.default.color, first.1, "{kind:?} {}", p.id);
                }
            }
            // Factory defaults agree with `default_study`, and the inputs
            // round-trip through get/set.
            for def in m.inputs {
                assert_eq!(study.get(def.id), Some(def.default), "{kind:?} {}", def.id);
                assert_eq!(def.kind.clamp(def.default), Some(def.default));
                let mut s = study.clone();
                assert!(s.set(def.id, def.default), "{kind:?} {}", def.id);
                assert_eq!(s, study);
            }
            // Ids are unique, and fills reference existing plots/levels.
            let mut ids: Vec<&str> = m.inputs.iter().map(|d| d.id).collect();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), m.inputs.len(), "{kind:?} input ids");
            let mut ids: Vec<&str> = m.plots.iter().map(|d| d.id).collect();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), m.plots.len(), "{kind:?} plot ids");
            for f in m.fills {
                for e in [f.between.0, f.between.1] {
                    match e {
                        FillEdge::Plot(id) => assert!(m.plot(id).is_some(), "{kind:?} {id}"),
                        FillEdge::HLine(id) => assert!(m.hline(id).is_some(), "{kind:?} {id}"),
                    }
                }
            }
            // Guide levels and bounds agree with `bounds()`.
            let levels: Vec<f64> = study
                .bounds()
                .map(|(_, _, l)| l.to_vec())
                .unwrap_or_default();
            let mut hl: Vec<f64> = m.hlines.iter().map(|h| h.value).collect();
            hl.sort_by(f64::total_cmp);
            assert_eq!(hl, levels, "{kind:?}");
            assert_eq!(m.bounds, study.bounds().map(|(a, b, _)| (a, b)), "{kind:?}");
        }
    }

    #[test]
    fn set_clamps_and_rejects() {
        let mut s = StudyKind::Rsi.default_study();
        assert!(s.set("length", InputValue::Int(0)));
        assert_eq!(s.get("length"), Some(InputValue::Int(1)));
        assert!(s.set("length", InputValue::Int(99_999)));
        assert_eq!(s.get("length"), Some(InputValue::Int(2000)));
        assert!(s.set("src", InputValue::Source(Source::Hlcc4)));
        assert_eq!(s.legend_title(), "RSI 2000 hlcc4");
        assert!(!s.set("length", InputValue::Float(3.0)), "wrong type");
        assert!(!s.set("nope", InputValue::Int(3)), "unknown id");

        let mut bb = StudyKind::BollingerBands.default_study();
        assert!(bb.set("mult", InputValue::Float(50.0)));
        assert_eq!(bb.get("mult"), Some(InputValue::Float(10.0)));
        assert!(!bb.set("mult", InputValue::Float(f64::NAN)));

        let mut v = Study::Vwap;
        assert!(v.set("anchor", InputValue::Choice(0)));
        assert!(!v.set("anchor", InputValue::Choice(1)), "out of range");
        assert!(!Study::Obv.set("length", InputValue::Int(3)));

        let mut vol = StudyKind::Volume.default_study();
        assert!(vol.set("show_ma", InputValue::Bool(true)));
        assert!(matches!(vol, Study::Volume { show_ma: true, .. }));
        let ma = vol.meta().plot("vol_ma").unwrap();
        assert!(ma.enabled_if.unwrap()(&vol));
    }
}
