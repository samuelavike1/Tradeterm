//! Moving Average Simple (SMA) and Moving Average Exponential (EMA).

use super::meta::*;
use super::{Placement, Source, Study, StudyKind, as_u32, ma};
use crate::CandleSeries;

const INPUTS: &[InputDef] = &[length("length", "Length", 9), SOURCE];

pub(super) static SMA_META: StudyMeta = StudyMeta {
    kind: StudyKind::Sma,
    name: "Moving Average Simple",
    short: "MA",
    placement: Placement::Overlay,
    format: ValueFormat::Price,
    mtf: false,
    inputs: INPUTS,
    plots: &[PlotDef::line("plot", "Value", rgb(0x2962ff))],
    hlines: &[],
    fills: &[],
    bounds: None,
};

pub(super) static EMA_META: StudyMeta = StudyMeta {
    kind: StudyKind::Ema,
    name: "Moving Average Exponential",
    short: "EMA",
    placement: Placement::Overlay,
    format: ValueFormat::Price,
    mtf: false,
    inputs: INPUTS,
    plots: &[PlotDef::line("plot", "Value", rgb(0x2962ff))],
    hlines: &[],
    fills: &[],
    bounds: None,
};

pub(super) fn sma(src: &CandleSeries, out: &mut [Vec<f64>], from: usize, len: u32, s: Source) {
    let n = src.len();
    ma::sma(|i| s.value(src, i), len as usize, &mut out[0], from, n);
}

pub(super) fn ema(src: &CandleSeries, out: &mut [Vec<f64>], from: usize, len: u32, s: Source) {
    let n = src.len();
    ma::ema(|i| s.value(src, i), len as usize, &mut out[0], from, n);
}

pub(super) fn get(s: &Study, id: &str) -> Option<InputValue> {
    let (Study::Sma { length, source } | Study::Ema { length, source }) = s else {
        return None;
    };
    Some(match id {
        "length" => InputValue::Int(*length as i64),
        "src" => InputValue::Source(*source),
        _ => return None,
    })
}

pub(super) fn set(s: &mut Study, id: &str, v: InputValue) -> bool {
    let (Study::Sma { length, source } | Study::Ema { length, source }) = s else {
        return false;
    };
    match (id, v) {
        ("length", InputValue::Int(x)) => *length = as_u32(x),
        ("src", InputValue::Source(x)) => *source = x,
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::*;

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
}
