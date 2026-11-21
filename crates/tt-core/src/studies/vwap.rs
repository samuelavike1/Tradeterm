//! Session VWAP (reset at 00:00 UTC), hlc3 source.

use super::meta::*;
use super::{Placement, Source, Study, StudyKind};
use crate::CandleSeries;

pub(super) const DAY_MS: i64 = 86_400_000;

pub(super) static META: StudyMeta = StudyMeta {
    kind: StudyKind::Vwap,
    name: "Volume Weighted Average Price",
    short: "VWAP",
    placement: Placement::Overlay,
    format: ValueFormat::Price,
    mtf: false,
    // Only "Session" exists so far (index 0 of TradingView's list).
    inputs: &[InputDef::new(
        "anchor",
        "Anchor Period",
        InputKind::Choice(&["Session"]),
        InputValue::Choice(0),
        true,
    )],
    plots: &[PlotDef::line("vwap", "Value", rgb(0x2962ff))],
    hlines: &[],
    fills: &[],
    bounds: None,
};

pub(super) fn compute(src: &CandleSeries, out: &mut [Vec<f64>], from: usize) {
    let n = src.len();
    // Session VWAP: cumulative typical price × volume since 00:00 UTC.
    // Walk back to the session start of `from` to resume sums.
    let t = src.open_times();
    let start = if from == 0 || from >= n {
        from
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

pub(super) fn get(s: &Study, id: &str) -> Option<InputValue> {
    match (s, id) {
        (Study::Vwap, "anchor") => Some(InputValue::Choice(0)),
        _ => None,
    }
}

pub(super) fn set(s: &mut Study, id: &str, v: InputValue) -> bool {
    matches!((s, id, v), (Study::Vwap, "anchor", InputValue::Choice(0)))
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::*;
    use crate::Candle;

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
}
