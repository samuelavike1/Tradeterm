//! Average True Range (Wilder RMA of the true range).

use super::meta::*;
use super::{Placement, Study, StudyKind, as_u32, ma};
use crate::CandleSeries;

pub(super) static META: StudyMeta = StudyMeta {
    kind: StudyKind::Atr,
    name: "Average True Range",
    short: "ATR",
    placement: Placement::Pane,
    format: ValueFormat::Price,
    mtf: false,
    inputs: &[length("length", "Length", 14)],
    plots: &[PlotDef::line("atr", "ATR", rgb(0xb71c1c))],
    hlines: &[],
    fills: &[],
    bounds: None,
};

pub(super) fn compute(src: &CandleSeries, out: &mut [Vec<f64>], from: usize, length: u32) {
    let n = src.len();
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
    ma::rma(tr, length.max(1) as usize, &mut out[0], from, n);
}

pub(super) fn get(s: &Study, id: &str) -> Option<InputValue> {
    match (s, id) {
        (Study::Atr { length }, "length") => Some(InputValue::Int(*length as i64)),
        _ => None,
    }
}

pub(super) fn set(s: &mut Study, id: &str, v: InputValue) -> bool {
    match (s, id, v) {
        (Study::Atr { length }, "length", InputValue::Int(x)) => {
            *length = as_u32(x);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::*;

    #[test]
    fn atr_values() {
        let s = series(&[10.0, 11.0, 10.5, 12.0]);
        let a = run(&Study::Atr { length: 2 }, &s);
        // TR: 2 (h-l), 2 (h-l=2, |h-pc|=2), 2, |12+1-10.5|=2.5 → RMA(2): seed 2, then 2, 2.25
        assert!(close(a[0][1], 2.0) && close(a[0][3], 2.25));
    }
}
