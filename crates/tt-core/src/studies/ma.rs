//! Moving-average helpers shared by every study.
//!
//! All helpers have the same shape: `x(i)` reads the input at bar `i`, and
//! `out[from..n]` is (re)written while `out[..from]` must already hold
//! correct values (recursive averages read `out[from - 1]`). Indices before
//! the first full window are NaN. None of them allocates per bar; only
//! [`hma`] allocates one scratch buffer per call, sized to the bars it needs.

use serde::{Deserialize, Serialize};

/// Moving-average type of an input such as BB "Basis MA Type".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum MaType {
    #[default]
    Sma,
    Ema,
    /// Wilder's smoothed MA, `ta.rma` (TradingView label "SMMA (RMA)").
    Smma,
    Wma,
    Vwma,
}

impl MaType {
    pub const ALL: [MaType; 5] = [
        MaType::Sma,
        MaType::Ema,
        MaType::Smma,
        MaType::Wma,
        MaType::Vwma,
    ];
    /// Dropdown labels, in [`Self::ALL`] order.
    pub const LABELS: [&'static str; 5] = ["SMA", "EMA", "SMMA (RMA)", "WMA", "VWMA"];

    pub fn label(self) -> &'static str {
        Self::LABELS[self as usize]
    }
}

/// Type of the "Smoothing" input group (SMA, EMA, RSI, OBV…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum SmoothingType {
    #[default]
    None,
    Sma,
    /// SMA plus Bollinger Bands around it.
    SmaBb,
    Ema,
    Smma,
    Wma,
    Vwma,
}

impl SmoothingType {
    pub const ALL: [SmoothingType; 7] = [
        SmoothingType::None,
        SmoothingType::Sma,
        SmoothingType::SmaBb,
        SmoothingType::Ema,
        SmoothingType::Smma,
        SmoothingType::Wma,
        SmoothingType::Vwma,
    ];
    /// Dropdown labels, in [`Self::ALL`] order.
    pub const LABELS: [&'static str; 7] = [
        "None",
        "SMA",
        "SMA + Bollinger Bands",
        "EMA",
        "SMMA (RMA)",
        "WMA",
        "VWMA",
    ];

    pub fn label(self) -> &'static str {
        Self::LABELS[self as usize]
    }

    /// The moving average this smoothing applies (`None` for no smoothing).
    pub fn ma_type(self) -> Option<MaType> {
        match self {
            SmoothingType::None => None,
            SmoothingType::Sma | SmoothingType::SmaBb => Some(MaType::Sma),
            SmoothingType::Ema => Some(MaType::Ema),
            SmoothingType::Smma => Some(MaType::Smma),
            SmoothingType::Wma => Some(MaType::Wma),
            SmoothingType::Vwma => Some(MaType::Vwma),
        }
    }
}

/// The "Smoothing" input group: type, length and BB standard deviations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Smoothing {
    pub kind: SmoothingType,
    pub length: u32,
    pub bb_mult: f64,
}

impl Smoothing {
    /// No smoothing (TradingView's default for SMA/EMA/OBV, and what legacy
    /// files without a smoothing key load as).
    pub const NONE: Smoothing = Smoothing {
        kind: SmoothingType::None,
        length: 14,
        bb_mult: 2.0,
    };
    /// SMA 14 (TradingView's default for RSI).
    pub const SMA_14: Smoothing = Smoothing {
        kind: SmoothingType::Sma,
        length: 14,
        bb_mult: 2.0,
    };
}

impl Default for Smoothing {
    fn default() -> Self {
        Self::NONE
    }
}

/// Any [`MaType`] over `x`. `vol` is the bar volume (used by VWMA only).
pub fn ma(
    kind: MaType,
    x: impl Fn(usize) -> f64,
    vol: &[f64],
    len: usize,
    out: &mut [f64],
    from: usize,
    n: usize,
) {
    match kind {
        MaType::Sma => sma(x, len, out, from, n),
        MaType::Ema => ema(x, len, out, from, n),
        MaType::Smma => rma(x, len, out, from, n),
        MaType::Wma => wma(x, len, out, from, n),
        MaType::Vwma => vwma(x, |i| vol[i], len, out, from, n),
    }
}

/// Simple moving average (NaN inputs make the window NaN).
pub fn sma(x: impl Fn(usize) -> f64, len: usize, out: &mut [f64], from: usize, n: usize) {
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

/// Exponential moving average seeded with the SMA of the first `len` values
/// (Pine `ta.ema`).
pub fn ema(x: impl Fn(usize) -> f64, len: usize, out: &mut [f64], from: usize, n: usize) {
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

/// EMA over a series whose first defined value is at `first` (MACD signal
/// line, smoothing of another study's output).
pub fn ema_skip_nan(x: &[f64], len: usize, out: &mut [f64], first: usize, from: usize, n: usize) {
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

/// Wilder's moving average (RMA / SMMA), seeded with an SMA (Pine `ta.rma`).
pub fn rma(x: impl Fn(usize) -> f64, len: usize, out: &mut [f64], from: usize, n: usize) {
    let len = len.max(1);
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

/// Linearly weighted moving average (Pine `ta.wma`): the newest value has
/// weight `len`, the oldest weight 1. O(1) per bar.
pub fn wma(x: impl Fn(usize) -> f64, len: usize, out: &mut [f64], from: usize, n: usize) {
    wma_each(x, len, from, n, |i, v| out[i] = v);
}

/// [`wma`] that hands each `(bar, value)` to `emit` (lets [`hma`] write into
/// a buffer that does not start at bar 0).
fn wma_each(
    x: impl Fn(usize) -> f64,
    len: usize,
    from: usize,
    n: usize,
    mut emit: impl FnMut(usize, f64),
) {
    let len = len.max(1);
    let denom = (len * (len + 1) / 2) as f64;
    // num = Σ weight·x over the window, sum = Σ x over the window.
    let (mut num, mut sum) = (0.0_f64, 0.0_f64);
    let mut have = false;
    for i in from..n {
        if i + 1 < len {
            emit(i, f64::NAN);
            continue;
        }
        if !have || !num.is_finite() || !sum.is_finite() {
            (num, sum) = (0.0, 0.0);
            for (w, j) in (i + 1 - len..=i).enumerate() {
                let v = x(j);
                num += (w + 1) as f64 * v;
                sum += v;
            }
            have = true;
        } else {
            // Every old weight drops by one (the oldest to zero), the new
            // value enters with weight `len`.
            let xi = x(i);
            num += len as f64 * xi - sum;
            sum += xi - x(i - len);
        }
        emit(i, num / denom);
    }
}

/// Volume-weighted moving average (Pine `ta.vwma`):
/// `sma(x·vol, len) / sma(vol, len)`. NaN while the window volume is zero.
pub fn vwma(
    x: impl Fn(usize) -> f64,
    vol: impl Fn(usize) -> f64,
    len: usize,
    out: &mut [f64],
    from: usize,
    n: usize,
) {
    let len = len.max(1);
    let (mut sxv, mut sv) = (0.0_f64, 0.0_f64);
    let mut have = false;
    for i in from..n {
        if i + 1 < len {
            out[i] = f64::NAN;
            continue;
        }
        if !have || !sxv.is_finite() || !sv.is_finite() {
            (sxv, sv) = (0.0, 0.0);
            for j in i + 1 - len..=i {
                let v = vol(j);
                sxv += x(j) * v;
                sv += v;
            }
            have = true;
        } else {
            let (vi, vo) = (vol(i), vol(i - len));
            sxv += x(i) * vi - x(i - len) * vo;
            sv += vi - vo;
        }
        out[i] = if sv != 0.0 { sxv / sv } else { f64::NAN };
    }
}

/// Hull moving average (Pine `ta.hma`):
/// `wma(2·wma(x, len/2) − wma(x, len), floor(sqrt(len)))`.
pub fn hma(x: impl Fn(usize) -> f64, len: usize, out: &mut [f64], from: usize, n: usize) {
    let len = len.max(1);
    let half = (len / 2).max(1);
    let sq = ((len as f64).sqrt().floor() as usize).max(1);
    if from >= n {
        return;
    }
    // The outer WMA at `from` reads the inner series back to `from - sq + 1`.
    let start = (from + 1).saturating_sub(sq);
    let mut diff = vec![f64::NAN; n - start];
    wma_each(&x, half, start, n, |i, v| diff[i - start] = 2.0 * v);
    wma_each(&x, len, start, n, |i, v| diff[i - start] -= v);
    wma_each(|i| diff[i - start], sq, from, n, |i, v| out[i] = v);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn run(f: impl Fn(&[f64], &mut [f64], usize), x: &[f64]) -> Vec<f64> {
        let mut out = vec![f64::NAN; x.len()];
        f(x, &mut out, 0);
        out
    }

    fn assert_series(got: &[f64], want: &[f64]) {
        assert_eq!(got.len(), want.len());
        for (i, (g, w)) in got.iter().zip(want).enumerate() {
            assert!(
                (g.is_nan() && w.is_nan()) || close(*g, *w),
                "bar {i}: got {g}, want {w}"
            );
        }
    }

    const X: [f64; 6] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    const V: [f64; 6] = [1.0, 2.0, 1.0, 2.0, 1.0, 2.0];
    const NAN: f64 = f64::NAN;

    #[test]
    fn sma_values() {
        let o = run(|x, o, f| sma(|i| x[i], 3, o, f, x.len()), &X);
        assert_series(&o, &[NAN, NAN, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn ema_values() {
        // seed SMA(1,2,3) = 2, alpha = 0.5: 3, 4, 5.
        let o = run(|x, o, f| ema(|i| x[i], 3, o, f, x.len()), &X);
        assert_series(&o, &[NAN, NAN, 2.0, 3.0, 4.0, 5.0]);
        let x = [2.0, 4.0, 10.0, 0.0];
        // seed 3 (len 2), alpha 2/3: 10·2/3 + 3/3 = 23/3, then 23/9.
        let o = run(|x, o, f| ema(|i| x[i], 2, o, f, x.len()), &x);
        assert_series(&o, &[NAN, 3.0, 23.0 / 3.0, 23.0 / 9.0]);
    }

    #[test]
    fn rma_values() {
        // seed 3 (len 2), then (3·1 + 10)/2 = 6.5, (6.5 + 0)/2 = 3.25.
        let x = [2.0, 4.0, 10.0, 0.0];
        let o = run(|x, o, f| rma(|i| x[i], 2, o, f, x.len()), &x);
        assert_series(&o, &[NAN, 3.0, 6.5, 3.25]);
    }

    #[test]
    fn wma_values() {
        // (1·1 + 2·2 + 3·3)/6 = 14/6, then (2 + 6 + 12)/6 = 20/6 …
        let o = run(|x, o, f| wma(|i| x[i], 3, o, f, x.len()), &X);
        assert_series(
            &o,
            &[NAN, NAN, 14.0 / 6.0, 20.0 / 6.0, 26.0 / 6.0, 32.0 / 6.0],
        );
        let x = [5.0, 1.0, 4.0, 2.0];
        // (5 + 2 + 12)/6, (1 + 8 + 6)/6
        let o = run(|x, o, f| wma(|i| x[i], 3, o, f, x.len()), &x);
        assert_series(&o, &[NAN, NAN, 19.0 / 6.0, 15.0 / 6.0]);
    }

    #[test]
    fn vwma_values() {
        let o = run(|x, o, f| vwma(|i| x[i], |i| V[i], 2, o, f, x.len()), &X);
        // (1·1 + 2·2)/3, (2·2 + 3·1)/3, (3 + 8)/3, (8 + 5)/3, (5 + 12)/3
        assert_series(
            &o,
            &[
                NAN,
                5.0 / 3.0,
                7.0 / 3.0,
                11.0 / 3.0,
                13.0 / 3.0,
                17.0 / 3.0,
            ],
        );
        // Zero volume: undefined.
        let mut out = [0.0; 2];
        vwma(|_| 1.0, |_| 0.0, 2, &mut out, 0, 2);
        assert!(out[1].is_nan());
    }

    #[test]
    fn hma_values() {
        // HMA has no lag on a straight line: HMA(4) of 0, 1, 2… is the input
        // from the first defined bar (len − 1 + sqrt − 1 = 4).
        let x: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let o = run(|x, o, f| hma(|i| x[i], 4, o, f, x.len()), &x);
        assert!(o[3].is_nan());
        for i in 4..10 {
            assert!(close(o[i], i as f64), "bar {i}: {}", o[i]);
        }
        // Hand-computed HMA(4) (half 2, sqrt 2) of 1, 3, 2, 6, 4:
        // wma2 = [_, 7/3, 7/3, 14/3, 14/3], wma4 = [_, _, _, 37/10, 41/10]
        // diff = 2·wma2 − wma4 → bar 3: 28/3 − 3.7, bar 4: 28/3 − 4.1
        let x = [1.0, 3.0, 2.0, 6.0, 4.0];
        let o = run(|x, o, f| hma(|i| x[i], 4, o, f, x.len()), &x);
        let (d3, d4) = (28.0 / 3.0 - 3.7, 28.0 / 3.0 - 4.1);
        assert_series(&o, &[NAN, NAN, NAN, NAN, (d3 + 2.0 * d4) / 3.0]);
    }

    #[test]
    fn ma_dispatch_and_incremental_tail() {
        let x: Vec<f64> = (0..200)
            .map(|i| 100.0 + (i as f64 * 0.31).sin() * 9.0)
            .collect();
        let v: Vec<f64> = (0..200).map(|i| 1.0 + (i % 7) as f64).collect();
        let n = x.len();
        for kind in MaType::ALL {
            for len in [1, 2, 9, 20] {
                let mut full = vec![f64::NAN; n];
                ma(kind, |i| x[i], &v, len, &mut full, 0, n);
                let mut tail = full.clone();
                tail[n - 5..].fill(0.0);
                ma(kind, |i| x[i], &v, len, &mut tail, n - 5, n);
                assert_series(&tail, &full);
            }
        }
        for len in [1, 2, 9, 20] {
            let mut full = vec![f64::NAN; n];
            hma(|i| x[i], len, &mut full, 0, n);
            let mut tail = full.clone();
            tail[n - 5..].fill(0.0);
            hma(|i| x[i], len, &mut tail, n - 5, n);
            assert_series(&tail, &full);
        }
    }

    #[test]
    fn running_sums_recover_after_nan() {
        // A NaN warm-up (another study's output) must not poison the result.
        let x = [NAN, NAN, 1.0, 2.0, 3.0, 4.0];
        let o = run(|x, o, f| wma(|i| x[i], 2, o, f, x.len()), &x);
        assert_series(&o, &[NAN, NAN, NAN, 5.0 / 3.0, 8.0 / 3.0, 11.0 / 3.0]);
        let o = run(|x, o, f| vwma(|i| x[i], |_| 1.0, 2, o, f, x.len()), &x);
        assert_series(&o, &[NAN, NAN, NAN, 1.5, 2.5, 3.5]);
    }

    #[test]
    fn labels() {
        assert_eq!(MaType::Smma.label(), "SMMA (RMA)");
        assert_eq!(SmoothingType::SmaBb.label(), "SMA + Bollinger Bands");
        assert_eq!(SmoothingType::SmaBb.ma_type(), Some(MaType::Sma));
        assert_eq!(Smoothing::default(), Smoothing::NONE);
    }
}
