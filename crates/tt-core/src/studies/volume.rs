//! Volume: columns colored by bar direction, plus an optional MA.

use super::meta::*;
use super::{Placement, Study, StudyKind, as_u32, ma, split2};
use crate::CandleSeries;

const FALLING: Rgba = rgba(0xf7525f, 128);
const GROWING: Rgba = rgba(0x22ab94, 128);

fn show_ma(s: &Study) -> bool {
    matches!(s, Study::Volume { show_ma: true, .. })
}

pub(super) static META: StudyMeta = StudyMeta {
    kind: StudyKind::Volume,
    name: "Volume",
    short: "Vol",
    placement: Placement::VolumeOverlay,
    format: ValueFormat::Volume,
    mtf: false,
    inputs: &[
        InputDef {
            in_status_line: false,
            ..short_length("ma_length", "MA Length", 20)
        },
        InputDef::new(
            "prev_close",
            "Color based on previous close",
            InputKind::Bool,
            InputValue::Bool(false),
            false,
        ),
        // On the Style tab today; becomes the `vol_ma` plot's visibility.
        InputDef::new(
            "show_ma",
            "Volume MA",
            InputKind::Bool,
            InputValue::Bool(false),
            false,
        ),
    ],
    plots: &[
        PlotDef {
            id: "vol",
            title: "Volume",
            kind: PlotKind::VolumeColumns,
            default: PlotDefault::columns(FALLING),
            // The renderer still uses the theme's volume colors.
            palette: &[("Falling", FALLING), ("Growing", GROWING)],
            enabled_if: None,
        },
        PlotDef {
            enabled_if: Some(show_ma),
            ..PlotDef::line("vol_ma", "Volume MA", rgb(0x2196f3))
        },
    ],
    hlines: &[],
    fills: &[],
    bounds: None,
};

pub(super) fn compute(src: &CandleSeries, out: &mut [Vec<f64>], from: usize, ma_length: u32) {
    let n = src.len();
    let v = src.volumes();
    out[0][from..n].copy_from_slice(&v[from..n]);
    let (vol, avg) = split2(out);
    ma::sma(|i| vol[i], ma_length as usize, avg, from, n);
}

pub(super) fn get(s: &Study, id: &str) -> Option<InputValue> {
    let Study::Volume {
        ma_length,
        color_by_prev_close,
        show_ma,
    } = s
    else {
        return None;
    };
    Some(match id {
        "ma_length" => InputValue::Int(*ma_length as i64),
        "prev_close" => InputValue::Bool(*color_by_prev_close),
        "show_ma" => InputValue::Bool(*show_ma),
        _ => return None,
    })
}

pub(super) fn set(s: &mut Study, id: &str, v: InputValue) -> bool {
    let Study::Volume {
        ma_length,
        color_by_prev_close,
        show_ma,
    } = s
    else {
        return false;
    };
    match (id, v) {
        ("ma_length", InputValue::Int(x)) => *ma_length = as_u32(x),
        ("prev_close", InputValue::Bool(b)) => *color_by_prev_close = b,
        ("show_ma", InputValue::Bool(b)) => *show_ma = b,
        _ => return false,
    }
    true
}
