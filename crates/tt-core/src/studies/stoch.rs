//! Stochastic: %K (smoothed) and its SMA %D.

use super::meta::*;
use super::{Placement, Study, StudyKind, as_u32, ma, split2};
use crate::CandleSeries;

const BLUE: u32 = 0x2962ff;

pub(super) static META: StudyMeta = StudyMeta {
    kind: StudyKind::Stochastic,
    name: "Stochastic",
    short: "Stoch",
    placement: Placement::Pane,
    format: ValueFormat::Fixed(2),
    mtf: false,
    inputs: &[
        length("k", "%K Length", 14),
        short_length("k_smooth", "%K Smoothing", 1),
        short_length("d", "%D Smoothing", 3),
    ],
    plots: &[
        PlotDef::line("k", "%K", rgb(BLUE)),
        PlotDef::line("d", "%D", rgb(0xff6d00)),
    ],
    hlines: &oscillator_levels(["Upper Band", "Middle Band", "Lower Band"], 80.0, 20.0),
    // Drawn today in the %K color at 8 % alpha.
    fills: &[FillDef {
        id: "bg",
        title: "Background",
        between: (FillEdge::HLine("upper"), FillEdge::HLine("lower")),
        color: rgba(BLUE, 20),
        color2: None,
        visible: true,
    }],
    bounds: Some((0.0, 100.0)),
};

pub(super) fn compute(src: &CandleSeries, out: &mut [Vec<f64>], k: u32, k_smooth: u32, d: u32) {
    let n = src.len();
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
    ma::sma(|i| raw[i], k_smooth.max(1) as usize, kout, 0, n);
    let kk = kout.clone();
    ma::sma(|i| kk[i], d.max(1) as usize, dout, 0, n);
}

pub(super) fn get(s: &Study, id: &str) -> Option<InputValue> {
    let Study::Stochastic { k, k_smooth, d } = s else {
        return None;
    };
    Some(match id {
        "k" => InputValue::Int(*k as i64),
        "k_smooth" => InputValue::Int(*k_smooth as i64),
        "d" => InputValue::Int(*d as i64),
        _ => return None,
    })
}

pub(super) fn set(s: &mut Study, id: &str, v: InputValue) -> bool {
    let Study::Stochastic { k, k_smooth, d } = s else {
        return false;
    };
    match (id, v) {
        ("k", InputValue::Int(x)) => *k = as_u32(x),
        ("k_smooth", InputValue::Int(x)) => *k_smooth = as_u32(x),
        ("d", InputValue::Int(x)) => *d = as_u32(x),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::*;

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
}
