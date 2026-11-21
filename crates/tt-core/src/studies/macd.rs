//! MACD: fast EMA − slow EMA, its EMA signal line and the histogram.

use super::meta::*;
use super::{Placement, Source, Study, StudyKind, StudyState, as_u32, ma};
use crate::CandleSeries;

const GROW_ABOVE: Rgba = rgb(0x26a69a);
const FALL_BELOW: Rgba = rgb(0xff5252);

pub(super) static META: StudyMeta = StudyMeta {
    kind: StudyKind::Macd,
    name: "MACD",
    short: "MACD",
    placement: Placement::Pane,
    format: ValueFormat::Price,
    mtf: false,
    inputs: &[
        length("fast", "Fast Length", 12),
        length("slow", "Slow Length", 26),
        short_length("signal", "Signal Smoothing", 9),
        SOURCE,
    ],
    plots: &[
        PlotDef {
            id: "hist",
            title: "Histogram",
            kind: PlotKind::SignedColumns,
            default: PlotDefault::columns(GROW_ABOVE),
            // TradingView's 4-color scheme, with the colors drawn today
            // (a shrinking column fades to 45 % alpha).
            palette: &[
                ("Grow above", GROW_ABOVE),
                (
                    "Fall above",
                    [GROW_ABOVE[0], GROW_ABOVE[1], GROW_ABOVE[2], 115],
                ),
                (
                    "Rise below",
                    [FALL_BELOW[0], FALL_BELOW[1], FALL_BELOW[2], 115],
                ),
                ("Fall below", FALL_BELOW),
            ],
            enabled_if: None,
        },
        PlotDef::line("macd", "MACD", rgb(0x2962ff)),
        PlotDef::line("signal", "Signal", rgb(0xff6d00)),
    ],
    hlines: &[],
    fills: &[],
    bounds: None,
};

/// The fast and slow EMAs are not outputs (only their difference is): their
/// values at the bar before the checkpoint are the state. The signal line
/// reads its previous output.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct State {
    fast: f64,
    slow: f64,
}

pub(super) fn compute(
    src: &CandleSeries,
    out: &mut [Vec<f64>],
    from: usize,
    [fast, slow, signal]: [u32; 3],
    source: Source,
    st: &mut StudyState,
) {
    let n = src.len();
    let (from, mut s) = st.resume::<State>(from);
    let (hist, rest) = out.split_at_mut(1);
    let (macd, sig) = rest.split_at_mut(1);
    let x = |i| source.value(src, i);
    let mut ck = s;
    for i in from..n {
        if i + 1 == n {
            ck = s;
        }
        s.fast = ma::ema_next(x, fast as usize, i, s.fast);
        s.slow = ma::ema_next(x, slow as usize, i, s.slow);
        macd[0][i] = s.fast - s.slow;
    }
    st.save(n - 1, ck);
    let m = &macd[0];
    // Defined from the longer EMA's first bar (scans at most that many).
    let first = m.iter().position(|x| x.is_finite()).unwrap_or(n);
    ma::ema_skip_nan(m, signal as usize, &mut sig[0], first, from, n);
    for i in from..n {
        hist[0][i] = m[i] - sig[0][i];
    }
}

pub(super) fn get(s: &Study, id: &str) -> Option<InputValue> {
    let Study::Macd {
        fast,
        slow,
        signal,
        source,
    } = s
    else {
        return None;
    };
    Some(match id {
        "fast" => InputValue::Int(*fast as i64),
        "slow" => InputValue::Int(*slow as i64),
        "signal" => InputValue::Int(*signal as i64),
        "src" => InputValue::Source(*source),
        _ => return None,
    })
}

pub(super) fn set(s: &mut Study, id: &str, v: InputValue) -> bool {
    let Study::Macd {
        fast,
        slow,
        signal,
        source,
    } = s
    else {
        return false;
    };
    match (id, v) {
        ("fast", InputValue::Int(x)) => *fast = as_u32(x),
        ("slow", InputValue::Int(x)) => *slow = as_u32(x),
        ("signal", InputValue::Int(x)) => *signal = as_u32(x),
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
}
