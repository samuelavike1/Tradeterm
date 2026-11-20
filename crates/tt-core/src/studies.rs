//! Technical indicators ("studies"), computed incrementally.
//!
//! Each [`Study`] produces one or more output series aligned with the source
//! bars (NaN where undefined). [`Study::compute`] fills outputs from index
//! `from` onward, reading earlier outputs for recursive formulas (EMA, RSI…),
//! so a live tick on the forming bar recomputes only that bar.

// Index loops mirror the formulas (several series indexed together).
#![allow(clippy::needless_range_loop)]

use crate::CandleSeries;
use serde::{Deserialize, Serialize};

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
}

impl Source {
    pub const ALL: [Source; 7] = [
        Source::Open,
        Source::High,
        Source::Low,
        Source::Close,
        Source::Hl2,
        Source::Hlc3,
        Source::Ohlc4,
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

    /// Full name as in TradingView's indicator list.
    pub fn name(self) -> &'static str {
        match self {
            StudyKind::Volume => "Volume",
            StudyKind::Sma => "Moving Average Simple",
            StudyKind::Ema => "Moving Average Exponential",
            StudyKind::BollingerBands => "Bollinger Bands",
            StudyKind::Vwap => "Volume Weighted Average Price",
            StudyKind::Rsi => "Relative Strength Index",
            StudyKind::Macd => "MACD",
            StudyKind::Stochastic => "Stochastic",
            StudyKind::Atr => "Average True Range",
            StudyKind::Obv => "On Balance Volume",
        }
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

const DAY_MS: i64 = 86_400_000;

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

    pub fn placement(&self) -> Placement {
        match self {
            Study::Volume { .. } => Placement::VolumeOverlay,
            Study::Sma { .. } | Study::Ema { .. } | Study::BollingerBands { .. } | Study::Vwap => {
                Placement::Overlay
            }
            _ => Placement::Pane,
        }
    }

    /// Legend title with inputs, e.g. `MA 9 close`, `BB 20 2 close`.
    pub fn legend_title(&self) -> String {
        match self {
            Study::Volume { .. } => "Vol".into(),
            Study::Sma { length, source } => format!("MA {length} {}", source.label()),
            Study::Ema { length, source } => format!("EMA {length} {}", source.label()),
            Study::BollingerBands {
                length,
                mult,
                source,
            } => {
                format!("BB {length} {mult} {}", source.label())
            }
            Study::Vwap => "VWAP Session".into(),
            Study::Rsi { length, source } => format!("RSI {length} {}", source.label()),
            Study::Macd {
                fast,
                slow,
                signal,
                source,
            } => format!("MACD {fast} {slow} {signal} {}", source.label()),
            Study::Stochastic { k, k_smooth, d } => format!("Stoch {k} {k_smooth} {d}"),
            Study::Atr { length } => format!("ATR {length}"),
            Study::Obv => "OBV".into(),
        }
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
        let k = self.outputs().len();
        out.resize_with(k, Vec::new);
        for o in out.iter_mut() {
            o.resize(n, f64::NAN);
        }
        let from = from.min(n);
        match *self {
            Study::Volume { ma_length, .. } => {
                let v = src.volumes();
                out[0][from..n].copy_from_slice(&v[from..n]);
                let (vol, ma) = split2(out);
                sma(|i| vol[i], ma_length as usize, ma, from, n);
            }
            Study::Sma { length, source } => sma(
                |i| source.value(src, i),
                length as usize,
                &mut out[0],
                from,
                n,
            ),
            Study::Ema { length, source } => ema(
                |i| source.value(src, i),
                length as usize,
                &mut out[0],
                from,
                n,
            ),
            Study::BollingerBands {
                length,
                mult,
                source,
            } => {
                let len = length.max(1) as usize;
                for i in from..n {
                    if i + 1 < len {
                        out[0][i] = f64::NAN;
                        out[1][i] = f64::NAN;
                        out[2][i] = f64::NAN;
                        continue;
                    }
                    let (mut s, mut s2) = (0.0, 0.0);
                    for j in i + 1 - len..=i {
                        let x = source.value(src, j);
                        s += x;
                        s2 += x * x;
                    }
                    let mean = s / len as f64;
                    let sd = (s2 / len as f64 - mean * mean).max(0.0).sqrt();
                    out[0][i] = mean;
                    out[1][i] = mean + mult * sd;
                    out[2][i] = mean - mult * sd;
                }
            }
            Study::Vwap => {
                // Session VWAP: cumulative typical price × volume since 00:00 UTC.
                // Walk back to the session start of `from` to resume sums.
                let t = src.open_times();
                let start = if from == 0 {
                    0
                } else {
                    let day = t[from].div_euclid(DAY_MS);
                    (0..=from)
                        .rev()
                        .take_while(|&j| t[j].div_euclid(DAY_MS) == day)
                        .last()
                        .unwrap_or(from)
                };
                let (mut pv, mut vol) = (0.0, 0.0);
                let mut day = None;
                for i in start..n {
                    let d = t[i].div_euclid(DAY_MS);
                    if day != Some(d) {
                        day = Some(d);
                        pv = 0.0;
                        vol = 0.0;
                    }
                    let v = src.volumes()[i];
                    pv += Source::Hlc3.value(src, i) * v;
                    vol += v;
                    if i >= from {
                        out[0][i] = if vol > 0.0 {
                            pv / vol
                        } else {
                            Source::Hlc3.value(src, i)
                        };
                    }
                }
            }
            Study::Rsi { length, source } => {
                let len = length.max(1) as usize;
                // Wilder's RSI: its state lives in the RMAs, so always run
                // from the start (cheap: one pass).
                rsi(|i| source.value(src, i), len, &mut out[0], n);
            }
            Study::Macd {
                fast,
                slow,
                signal,
                source,
            } => {
                let mut f = vec![f64::NAN; n];
                let mut s = vec![f64::NAN; n];
                ema(|i| source.value(src, i), fast as usize, &mut f, 0, n);
                ema(|i| source.value(src, i), slow as usize, &mut s, 0, n);
                let (hist, rest) = out.split_at_mut(1);
                let (macd, sig) = rest.split_at_mut(1);
                for i in from..n {
                    macd[0][i] = f[i] - s[i];
                }
                let m = &macd[0];
                let first = m.iter().position(|x| x.is_finite()).unwrap_or(n);
                ema_skip_nan(m, signal as usize, &mut sig[0], first, from, n);
                for i in from..n {
                    hist[0][i] = m[i] - sig[0][i];
                }
            }
            Study::Stochastic { k, k_smooth, d } => {
                let (h, l, c) = (src.highs(), src.lows(), src.closes());
                let kl = k.max(1) as usize;
                let mut raw = vec![f64::NAN; n];
                for i in 0..n {
                    if i + 1 < kl {
                        continue;
                    }
                    let hh = h[i + 1 - kl..=i].iter().copied().fold(f64::MIN, f64::max);
                    let ll = l[i + 1 - kl..=i].iter().copied().fold(f64::MAX, f64::min);
                    raw[i] = if hh > ll {
                        (c[i] - ll) / (hh - ll) * 100.0
                    } else {
                        50.0
                    };
                }
                let (kout, dout) = split2(out);
                sma(|i| raw[i], k_smooth.max(1) as usize, kout, 0, n);
                let kk = kout.clone();
                sma(|i| kk[i], d.max(1) as usize, dout, 0, n);
            }
            Study::Atr { length } => {
                let (h, l, c) = (src.highs(), src.lows(), src.closes());
                let tr = |i: usize| {
                    if i == 0 {
                        h[0] - l[0]
                    } else {
                        (h[i] - l[i])
                            .max((h[i] - c[i - 1]).abs())
                            .max((l[i] - c[i - 1]).abs())
                    }
                };
                rma(tr, length.max(1) as usize, &mut out[0], from, n);
            }
            Study::Obv => {
                let (c, v) = (src.closes(), src.volumes());
                for i in from..n {
                    let prev = if i == 0 { 0.0 } else { out[0][i - 1] };
                    out[0][i] = if i == 0 {
                        0.0
                    } else if c[i] > c[i - 1] {
                        prev + v[i]
                    } else if c[i] < c[i - 1] {
                        prev - v[i]
                    } else {
                        prev
                    };
                }
            }
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

/// Simple moving average (NaN inputs make the window NaN).
fn sma(x: impl Fn(usize) -> f64, len: usize, out: &mut [f64], from: usize, n: usize) {
    let len = len.max(1);
    let mut sum: f64 = 0.0;
    let mut have = false;
    for i in from..n {
        if i + 1 < len {
            out[i] = f64::NAN;
            continue;
        }
        // A NaN in the window (e.g. another study's warm-up) would poison a
        // running sum forever, so sum the window afresh until it is clean.
        if !have || !sum.is_finite() {
            sum = (i + 1 - len..=i).map(&x).sum();
            have = true;
        } else {
            sum += x(i) - x(i - len);
        }
        out[i] = sum / len as f64;
    }
}

/// Exponential moving average seeded with the SMA of the first `len` values.
fn ema(x: impl Fn(usize) -> f64, len: usize, out: &mut [f64], from: usize, n: usize) {
    let len = len.max(1);
    let alpha = 2.0 / (len as f64 + 1.0);
    for i in from..n {
        out[i] = if i + 1 < len {
            f64::NAN
        } else if i + 1 == len {
            (0..len).map(&x).sum::<f64>() / len as f64
        } else {
            alpha * x(i) + (1.0 - alpha) * out[i - 1]
        };
    }
}

/// EMA over a series that starts with NaNs (MACD signal line).
fn ema_skip_nan(x: &[f64], len: usize, out: &mut [f64], first: usize, from: usize, n: usize) {
    let len = len.max(1);
    let alpha = 2.0 / (len as f64 + 1.0);
    for i in from..n {
        out[i] = if i < first || i + 1 < first + len {
            f64::NAN
        } else if i + 1 == first + len {
            x[first..=i].iter().sum::<f64>() / len as f64
        } else {
            alpha * x[i] + (1.0 - alpha) * out[i - 1]
        };
    }
}

/// Wilder's moving average (RMA), seeded with an SMA.
fn rma(x: impl Fn(usize) -> f64, len: usize, out: &mut [f64], from: usize, n: usize) {
    for i in from..n {
        out[i] = if i + 1 < len {
            f64::NAN
        } else if i + 1 == len {
            (0..len).map(&x).sum::<f64>() / len as f64
        } else {
            (out[i - 1] * (len as f64 - 1.0) + x(i)) / len as f64
        };
    }
}

fn rsi(x: impl Fn(usize) -> f64, len: usize, out: &mut [f64], n: usize) {
    let (mut up, mut down) = (0.0, 0.0);
    for i in 0..n {
        if i == 0 {
            out[0] = f64::NAN;
            continue;
        }
        let ch = x(i) - x(i - 1);
        let (g, l) = (ch.max(0.0), (-ch).max(0.0));
        if i <= len {
            up += g;
            down += l;
            if i < len {
                out[i] = f64::NAN;
                continue;
            }
            up /= len as f64;
            down /= len as f64;
        } else {
            up = (up * (len as f64 - 1.0) + g) / len as f64;
            down = (down * (len as f64 - 1.0) + l) / len as f64;
        }
        out[i] = if down == 0.0 {
            100.0
        } else {
            100.0 - 100.0 / (1.0 + up / down)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Candle;

    fn series(closes: &[f64]) -> CandleSeries {
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

    fn run(study: &Study, s: &CandleSeries) -> Vec<Vec<f64>> {
        let mut out = Vec::new();
        study.compute(s, &mut out, 0);
        out
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn sma_and_ema_values() {
        let s = series(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let o = run(
            &Study::Sma {
                length: 3,
                source: Source::Close,
            },
            &s,
        );
        assert!(o[0][1].is_nan());
        assert_eq!(&o[0][2..], &[2.0, 3.0, 4.0]);
        let e = run(
            &Study::Ema {
                length: 3,
                source: Source::Close,
            },
            &s,
        );
        // seed = SMA(1,2,3) = 2; alpha = 0.5 → 3.0, 4.0
        assert_eq!(&e[0][2..], &[2.0, 3.0, 4.0]);
    }

    #[test]
    fn bollinger_bands() {
        let s = series(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        let o = run(
            &Study::BollingerBands {
                length: 8,
                mult: 2.0,
                source: Source::Close,
            },
            &s,
        );
        // mean 5, population sd 2
        assert!(close(o[0][7], 5.0) && close(o[1][7], 9.0) && close(o[2][7], 1.0));
    }

    #[test]
    fn rsi_extremes_and_range() {
        let up = series(&(0..30).map(|i| i as f64).collect::<Vec<_>>());
        assert!(close(
            run(
                &Study::Rsi {
                    length: 14,
                    source: Source::Close
                },
                &up
            )[0][29],
            100.0
        ));
        let zig = series(
            &(0..60)
                .map(|i| 100.0 + if i % 2 == 0 { 1.0 } else { -1.0 })
                .collect::<Vec<_>>(),
        );
        let r = run(
            &Study::Rsi {
                length: 14,
                source: Source::Close,
            },
            &zig,
        );
        assert!(r[0][13].is_nan() && r[0][14].is_finite());
        assert!(
            r[0].iter()
                .filter(|x| x.is_finite())
                .all(|x| (0.0..=100.0).contains(x))
        );
        assert!((r[0][59] - 50.0).abs() < 5.0, "{}", r[0][59]);
    }

    #[test]
    fn macd_histogram_is_macd_minus_signal() {
        let s = series(
            &(0..80)
                .map(|i| 100.0 + (i as f64 * 0.3).sin() * 5.0)
                .collect::<Vec<_>>(),
        );
        let o = run(&StudyKind::Macd.default_study(), &s);
        let last = 79;
        assert!(o[1][last].is_finite() && o[2][last].is_finite());
        assert!(close(o[0][last], o[1][last] - o[2][last]));
        assert!(o[2][24].is_nan(), "signal needs slow + signal - 1 bars");
    }

    #[test]
    fn stochastic_bounds() {
        let s = series(
            &(0..40)
                .map(|i| 50.0 + (i as f64 * 0.7).sin() * 10.0)
                .collect::<Vec<_>>(),
        );
        let o = run(&StudyKind::Stochastic.default_study(), &s);
        assert!(
            o[0].iter()
                .chain(&o[1])
                .filter(|x| x.is_finite())
                .all(|x| (0.0..=100.0).contains(x))
        );
    }

    #[test]
    fn atr_and_obv() {
        let s = series(&[10.0, 11.0, 10.5, 12.0]);
        let a = run(&Study::Atr { length: 2 }, &s);
        // TR: 2 (h-l), 2 (h-l=2, |h-pc|=2), 2, |12+1-10.5|=2.5 → RMA(2): seed 2, then 2, 2.25
        assert!(close(a[0][1], 2.0) && close(a[0][3], 2.25));
        let o = run(&Study::Obv, &s);
        // up (+11), down (-12), up (+13)
        assert_eq!(o[0], vec![0.0, 11.0, -1.0, 12.0]);
    }

    #[test]
    fn vwap_resets_each_utc_day() {
        let mut s = CandleSeries::new();
        for (i, (t, p)) in [(0, 10.0), (60_000, 20.0), (DAY_MS, 30.0)]
            .iter()
            .enumerate()
        {
            s.upsert(Candle {
                open_time_ms: *t,
                open: *p,
                high: *p,
                low: *p,
                close: *p,
                volume: (i + 1) as f64,
            });
        }
        let o = run(&Study::Vwap, &s);
        assert!(close(o[0][1], (10.0 + 40.0) / 3.0));
        assert!(close(o[0][2], 30.0), "new day starts fresh");
    }

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
}
