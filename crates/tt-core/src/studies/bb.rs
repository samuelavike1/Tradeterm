//! Bollinger Bands: SMA basis ± `mult` population standard deviations.

use super::meta::*;
use super::{Placement, Source, Study, StudyKind, as_u32};
use crate::CandleSeries;

const BASIS: Rgba = rgb(0x2196f3);

pub(super) static META: StudyMeta = StudyMeta {
    kind: StudyKind::BollingerBands,
    name: "Bollinger Bands",
    short: "BB",
    placement: Placement::Overlay,
    format: ValueFormat::Price,
    mtf: false,
    inputs: &[
        length("length", "Length", 20),
        InputDef::new(
            "mult",
            "StdDev",
            InputKind::Float {
                min: 0.1,
                max: 10.0,
                step: 0.1,
            },
            InputValue::Float(2.0),
            true,
        ),
        SOURCE,
    ],
    plots: &[
        PlotDef::line("basis", "Basis", BASIS),
        PlotDef::line("upper", "Upper", rgb(0xf23645)),
        PlotDef::line("lower", "Lower", rgb(0x089981)),
    ],
    hlines: &[],
    // Drawn today in the basis color at 8 % alpha.
    fills: &[FillDef {
        id: "bg",
        title: "Background",
        between: (FillEdge::Plot("upper"), FillEdge::Plot("lower")),
        color: rgba(0x2196f3, 20),
        color2: None,
        visible: true,
    }],
    bounds: None,
};

pub(super) fn compute(
    src: &CandleSeries,
    out: &mut [Vec<f64>],
    from: usize,
    length: u32,
    mult: f64,
    source: Source,
) {
    let n = src.len();
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

pub(super) fn get(s: &Study, id: &str) -> Option<InputValue> {
    let Study::BollingerBands {
        length,
        mult,
        source,
    } = s
    else {
        return None;
    };
    Some(match id {
        "length" => InputValue::Int(*length as i64),
        "mult" => InputValue::Float(*mult),
        "src" => InputValue::Source(*source),
        _ => return None,
    })
}

pub(super) fn set(s: &mut Study, id: &str, v: InputValue) -> bool {
    let Study::BollingerBands {
        length,
        mult,
        source,
    } = s
    else {
        return false;
    };
    match (id, v) {
        ("length", InputValue::Int(x)) => *length = as_u32(x),
        ("mult", InputValue::Float(x)) => *mult = x,
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
}
