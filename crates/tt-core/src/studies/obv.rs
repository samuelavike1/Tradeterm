//! On Balance Volume.

use super::meta::*;
use super::{Placement, StudyKind};
use crate::CandleSeries;

pub(super) static META: StudyMeta = StudyMeta {
    kind: StudyKind::Obv,
    name: "On Balance Volume",
    short: "OBV",
    placement: Placement::Pane,
    format: ValueFormat::Volume,
    mtf: false,
    inputs: &[],
    plots: &[PlotDef::line("obv", "OBV", rgb(0x2962ff))],
    hlines: &[],
    fills: &[],
    bounds: None,
};

pub(super) fn compute(src: &CandleSeries, out: &mut [Vec<f64>], from: usize) {
    let n = src.len();
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

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use crate::Study;

    #[test]
    fn obv_values() {
        let s = series(&[10.0, 11.0, 10.5, 12.0]);
        let o = run(&Study::Obv, &s);
        // up (+11), down (-12), up (+13)
        assert_eq!(o[0], vec![0.0, 11.0, -1.0, 12.0]);
    }
}
