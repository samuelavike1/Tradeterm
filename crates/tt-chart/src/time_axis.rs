//! Time-axis ticks on an index-based x axis.
//!
//! The x axis is bar index, not wall time, so weekends and closed sessions
//! take no space. Candidate ticks are the bars where a calendar unit rolls
//! over; each gets a weight (year > month > day > 12h > … > 1m). Candidates
//! are placed greedily by weight, skipping any whose label would overlap one
//! already placed, so the most meaningful boundaries always win and labels
//! never collide.

use crate::scale::{DateTime, HOUR_MS, MINUTE_MS};
use std::ops::Range;

/// Significance of a time boundary; higher wins when labels compete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TickWeight {
    Minute,
    Minute5,
    Minute15,
    Minute30,
    Hour,
    Hour3,
    Hour6,
    Hour12,
    Day,
    Month,
    Year,
}

/// The most significant boundary crossed between two bar open times.
pub fn boundary_weight(prev_ms: i64, ms: i64) -> Option<TickWeight> {
    if ms <= prev_ms {
        return None;
    }
    let (a, b) = (DateTime::from_unix_ms(prev_ms), DateTime::from_unix_ms(ms));
    if a.year != b.year {
        return Some(TickWeight::Year);
    }
    if a.month != b.month {
        return Some(TickWeight::Month);
    }
    if a.day != b.day {
        return Some(TickWeight::Day);
    }
    const UNITS: [(i64, TickWeight); 8] = [
        (12 * HOUR_MS, TickWeight::Hour12),
        (6 * HOUR_MS, TickWeight::Hour6),
        (3 * HOUR_MS, TickWeight::Hour3),
        (HOUR_MS, TickWeight::Hour),
        (30 * MINUTE_MS, TickWeight::Minute30),
        (15 * MINUTE_MS, TickWeight::Minute15),
        (5 * MINUTE_MS, TickWeight::Minute5),
        (MINUTE_MS, TickWeight::Minute),
    ];
    UNITS
        .iter()
        .find(|(unit, _)| prev_ms.div_euclid(*unit) != ms.div_euclid(*unit))
        .map(|(_, w)| *w)
}

/// Label text for a tick of the given weight.
pub fn tick_label(ms: i64, weight: TickWeight) -> String {
    let dt = DateTime::from_unix_ms(ms);
    match weight {
        TickWeight::Year => dt.year.to_string(),
        TickWeight::Month => dt.month_name().to_owned(),
        TickWeight::Day => dt.day.to_string(),
        _ => format!("{:02}:{:02}", dt.hour, dt.minute),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeTick {
    pub index: usize,
    pub x: f32,
    pub weight: TickWeight,
    pub label: String,
}

/// Picks non-overlapping time ticks for the visible bars.
///
/// - `x_of(i)`: screen x of bar `i`;
/// - `label_width(text)`: rendered width of a label;
/// - `stride`: only every `stride`-th bar is a candidate (keeps the work
///   bounded when thousands of bars share a pixel). Candidates sit on
///   multiples of `stride`, so they stay put while panning.
pub fn time_ticks(
    times: &[i64],
    visible: Range<usize>,
    x_range: (f32, f32),
    stride: usize,
    min_gap: f32,
    x_of: impl Fn(usize) -> f32,
    mut label_width: impl FnMut(&str) -> f32,
) -> Vec<TimeTick> {
    let stride = stride.max(1);
    let end = visible.end.min(times.len());
    let first = visible.start.div_ceil(stride).max(1) * stride;

    let mut cands: Vec<(TickWeight, usize)> = (first..end)
        .step_by(stride)
        .filter_map(|i| boundary_weight(times[i - stride], times[i]).map(|w| (w, i)))
        .collect();
    // Use one regular unit: drop weights so fine that their ticks could not
    // all fit (otherwise leftovers like "08:52" fill the gaps unevenly).
    let span = (x_range.1 - x_range.0).max(1.0);
    let slot = label_width("00:00") + min_gap;
    let mut counts = [0usize; 11];
    for (w, _) in &cands {
        counts[*w as usize] += 1;
    }
    let mut threshold = TickWeight::Year as usize;
    let mut at_least = 0;
    for w in (0..counts.len()).rev() {
        at_least += counts[w];
        if at_least as f32 * slot > span {
            break;
        }
        threshold = w;
    }
    cands.retain(|(w, _)| *w as usize >= threshold);
    // Heaviest first; among equals, left to right.
    cands.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

    let mut placed: Vec<TimeTick> = Vec::new();
    let mut spans: Vec<(f32, f32)> = Vec::new();
    for (weight, i) in cands {
        let x = x_of(i);
        let label = tick_label(times[i], weight);
        let half = label_width(&label) / 2.0 + min_gap / 2.0;
        let (l, r) = (x - half, x + half);
        if l < x_range.0 || r > x_range.1 {
            continue;
        }
        if spans.iter().any(|(a, b)| l < *b && r > *a) {
            continue;
        }
        spans.push((l, r));
        placed.push(TimeTick {
            index: i,
            x,
            weight,
            label,
        });
    }
    placed.sort_by(|a, b| a.x.total_cmp(&b.x));
    placed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::DAY_MS;

    const NOV20: i64 = 1_795_132_800_000; // 2026-11-20 00:00 UTC

    #[test]
    fn weights() {
        let m = MINUTE_MS;
        assert_eq!(
            boundary_weight(NOV20 + m, NOV20 + 2 * m),
            Some(TickWeight::Minute)
        );
        assert_eq!(
            boundary_weight(NOV20 + 4 * m, NOV20 + 5 * m),
            Some(TickWeight::Minute5)
        );
        assert_eq!(
            boundary_weight(NOV20 + 59 * m, NOV20 + 60 * m),
            Some(TickWeight::Hour)
        );
        assert_eq!(
            boundary_weight(NOV20 + 11 * HOUR_MS, NOV20 + 12 * HOUR_MS),
            Some(TickWeight::Hour12)
        );
        assert_eq!(boundary_weight(NOV20 - m, NOV20), Some(TickWeight::Day));
        // 2026-12-01 and 2027-01-01
        assert_eq!(
            boundary_weight(NOV20, NOV20 + 11 * DAY_MS),
            Some(TickWeight::Month)
        );
        assert_eq!(
            boundary_weight(NOV20, NOV20 + 42 * DAY_MS),
            Some(TickWeight::Year)
        );
        assert_eq!(boundary_weight(NOV20, NOV20), None);
    }

    #[test]
    fn labels() {
        assert_eq!(
            tick_label(NOV20 + 14 * HOUR_MS + 5 * MINUTE_MS, TickWeight::Minute5),
            "14:05"
        );
        assert_eq!(tick_label(NOV20, TickWeight::Day), "20");
        assert_eq!(tick_label(NOV20, TickWeight::Month), "Nov");
        assert_eq!(tick_label(NOV20, TickWeight::Year), "2026");
    }

    /// 1m bars with a weekend-style gap: indices are contiguous, times are not.
    fn bars_with_gap() -> Vec<i64> {
        let mut t: Vec<i64> = (0..600).map(|i| NOV20 + i * MINUTE_MS).collect();
        let resume = NOV20 + 3 * DAY_MS;
        t.extend((0..600).map(|i| resume + i * MINUTE_MS));
        t
    }

    #[test]
    fn ticks_never_overlap_and_prefer_heavy_boundaries() {
        let times = bars_with_gap();
        for spacing in [0.5_f32, 2.0, 6.0, 20.0] {
            let x_of = |i: usize| i as f32 * spacing;
            let width = times.len() as f32 * spacing;
            let stride = (3.0 / spacing).ceil() as usize;
            let ticks = time_ticks(
                &times,
                0..times.len(),
                (0.0, width),
                stride,
                12.0,
                x_of,
                |s| s.len() as f32 * 7.0,
            );
            assert!(!ticks.is_empty(), "spacing {spacing}");
            for w in ticks.windows(2) {
                let r0 = w[0].x + w[0].label.len() as f32 * 3.5;
                let l1 = w[1].x - w[1].label.len() as f32 * 3.5;
                assert!(
                    l1 - r0 >= 12.0 - 1e-3,
                    "overlap at spacing {spacing}: {w:?}"
                );
            }
            // The gap (day change at index 600) is a Day tick, labelled "23", whenever it fits.
            if let Some(t) = ticks.iter().find(|t| t.index == 600) {
                assert_eq!(t.weight, TickWeight::Day);
                assert_eq!(t.label, "23");
            }
        }
    }

    #[test]
    fn intraday_labels_use_one_regular_unit() {
        // 1m bars, 6px apart, 900px wide: 15-minute labels (90px apart) fit, 5-minute ones don't.
        let times: Vec<i64> = (0..150)
            .map(|i| NOV20 + 8 * HOUR_MS + i * MINUTE_MS)
            .collect();
        let ticks = time_ticks(
            &times,
            0..150,
            (0.0, 900.0),
            1,
            18.0,
            |i| i as f32 * 6.0,
            |s| s.len() as f32 * 7.0,
        );
        assert!(ticks.len() >= 5, "{ticks:?}");
        for t in &ticks {
            assert!(t.weight >= TickWeight::Minute15, "{t:?}");
        }
    }

    #[test]
    fn gap_takes_no_space() {
        let times = bars_with_gap();
        // Bars 599 and 600 are 3 days apart in time but adjacent on screen.
        let x_of = |i: usize| i as f32 * 4.0;
        assert_eq!(x_of(600) - x_of(599), 4.0);
        let ticks = time_ticks(&times, 550..650, (0.0, 1e6), 1, 10.0, x_of, |s| {
            s.len() as f32 * 7.0
        });
        assert!(
            ticks
                .iter()
                .any(|t| t.index == 600 && t.weight == TickWeight::Day)
        );
    }

    #[test]
    fn candidates_are_stable_under_panning() {
        let times = bars_with_gap();
        let tick_idx = |range: Range<usize>| {
            time_ticks(
                &times,
                range,
                (0.0, 1e9),
                4,
                10.0,
                |i| i as f32 * 1.0,
                |s| s.len() as f32,
            )
            .into_iter()
            .map(|t| t.index)
            .collect::<Vec<_>>()
        };
        for i in tick_idx(100..500) {
            assert_eq!(i % 4, 0);
        }
        // Overlapping windows agree on shared candidates' positions.
        let a = tick_idx(100..500);
        let b = tick_idx(101..500);
        assert!(a.iter().filter(|i| **i >= 104).all(|i| i % 4 == 0));
        assert!(b.iter().all(|i| i % 4 == 0));
    }
}
