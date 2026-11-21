//! Relative Strength Index (Wilder).

use super::meta::*;
use super::{Placement, Source, Study, StudyKind, as_u32};
use crate::CandleSeries;

const PURPLE: u32 = 0x7e57c2;

pub(super) static META: StudyMeta = StudyMeta {
    kind: StudyKind::Rsi,
    name: "Relative Strength Index",
    short: "RSI",
    placement: Placement::Pane,
    format: ValueFormat::Fixed(2),
    mtf: false,
    inputs: &[length("length", "RSI Length", 14), SOURCE],
    plots: &[PlotDef::line("rsi", "RSI", rgb(PURPLE))],
    hlines: &oscillator_levels(
        ["RSI Upper Band", "RSI Middle Band", "RSI Lower Band"],
        70.0,
        30.0,
    ),
    // Drawn today in the RSI color at 8 % alpha.
    fills: &[FillDef {
        id: "bg",
        title: "RSI Background Fill",
        between: (FillEdge::HLine("upper"), FillEdge::HLine("lower")),
        color: rgba(PURPLE, 20),
        color2: None,
        visible: true,
    }],
    bounds: Some((0.0, 100.0)),
};

pub(super) fn compute(src: &CandleSeries, out: &mut [Vec<f64>], length: u32, source: Source) {
    let n = src.len();
    let len = length.max(1) as usize;
    // Wilder's RSI: its state lives in the RMAs, so always run from the
    // start (cheap: one pass).
    rsi(|i| source.value(src, i), len, &mut out[0], n);
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

pub(super) fn get(s: &Study, id: &str) -> Option<InputValue> {
    let Study::Rsi { length, source } = s else {
        return None;
    };
    Some(match id {
        "length" => InputValue::Int(*length as i64),
        "src" => InputValue::Source(*source),
        _ => return None,
    })
}

pub(super) fn set(s: &mut Study, id: &str, v: InputValue) -> bool {
    let Study::Rsi { length, source } = s else {
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
}
