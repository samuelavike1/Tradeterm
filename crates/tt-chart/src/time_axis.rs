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
    /// Day of month 5, 10, 15, 20, 25, 30.
    Day5,
    Month,
    /// April, July, October (January is a year boundary).
    Quarter,
    Year,
}

const WEIGHT_COUNT: usize = TickWeight::Year as usize + 1;

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
        return Some(if (a.month - 1) / 3 != (b.month - 1) / 3 {
            TickWeight::Quarter
        } else {
            TickWeight::Month
        });
    }
    if a.day != b.day {
        return Some(if a.day / 5 != b.day / 5 {
            TickWeight::Day5
        } else {
            TickWeight::Day
        });
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
        TickWeight::Month | TickWeight::Quarter => dt.month_name().to_owned(),
        TickWeight::Day | TickWeight::Day5 => dt.day.to_string(),
        _ => format!("{:02}:{:02}", dt.hour, dt.minute),
    }
}

/// Time of any bar *slot* on the x axis, like TradingView's time scale: real
/// bar times inside the data, extrapolated by the bar interval before the
/// first bar and after the last one. The axis and grid are a continuous ruler
/// over slots, so they continue into empty space where no candles exist.
#[derive(Debug, Clone, Copy)]
pub struct SlotTimes<'a> {
    times: &'a [i64],
    interval_ms: i64,
}

impl<'a> SlotTimes<'a> {
    pub fn new(times: &'a [i64], interval_ms: i64) -> Self {
        Self {
            times,
            interval_ms: interval_ms.max(1),
        }
    }

    /// Open time of slot `i` (may be negative or past the last bar).
    /// `None` only when there is no data at all.
    pub fn time_at(&self, i: i64) -> Option<i64> {
        let n = self.times.len() as i64;
        if n == 0 {
            None
        } else if i < 0 {
            Some(self.times[0] + i * self.interval_ms)
        } else if i >= n {
            Some(self.times[n as usize - 1] + (i - (n - 1)) * self.interval_ms)
        } else {
            Some(self.times[i as usize])
        }
    }

    /// Time at a fractional slot, interpolated between neighboring slots.
    pub fn time_at_frac(&self, slot: f64) -> Option<i64> {
        let i = slot.floor();
        let (a, b) = (self.time_at(i as i64)?, self.time_at(i as i64 + 1)?);
        Some(a + ((b - a) as f64 * (slot - i)).round() as i64)
    }

    /// Fractional slot of `time`: exact on bar open times, interpolated
    /// between bars (e.g. a 1m time on an 1h chart) and extrapolated by the
    /// interval outside the data. Inverse of [`Self::time_at_frac`].
    pub fn slot_of(&self, time: i64) -> Option<f64> {
        let n = self.times.len();
        let (first, last) = (*self.times.first()?, *self.times.last()?);
        let iv = self.interval_ms as f64;
        if time <= first {
            return Some((time - first) as f64 / iv);
        }
        if time >= last {
            return Some((n - 1) as f64 + (time - last) as f64 / iv);
        }
        let k = self.times.partition_point(|t| *t <= time) - 1;
        let (a, b) = (self.times[k], self.times[k + 1]);
        Some(k as f64 + (time - a) as f64 / (b - a).max(1) as f64)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeTick {
    /// Slot index (can lie outside the data).
    pub index: i64,
    pub x: f32,
    pub weight: TickWeight,
    pub label: String,
    /// Heavier than the regular unit shown at this zoom (e.g. a full hour
    /// among half-hours, or a new day); drawn bold like TradingView.
    pub major: bool,
}

/// Picks non-overlapping time ticks for the visible bars.
///
/// - `x_of(i)`: screen x of bar `i`;
/// - `label_width(text)`: rendered width of a label;
/// - `stride`: only every `stride`-th bar is a candidate (keeps the work
///   bounded when thousands of bars share a pixel). Candidates sit on
///   multiples of `stride`, so they stay put while panning.
pub fn time_ticks(
    slots: &SlotTimes<'_>,
    visible: Range<i64>,
    x_range: (f32, f32),
    stride: usize,
    min_gap: f32,
    x_of: impl Fn(i64) -> f32,
    mut label_width: impl FnMut(&str) -> f32,
) -> Vec<TimeTick> {
    let stride = stride.max(1) as i64;
    // Candidates on multiples of `stride`, so they stay put while panning.
    let first = visible.start.div_euclid(stride) * stride;
    let mut cands: Vec<(TickWeight, i64)> = Vec::new();
    let mut i = first;
    while i < visible.end {
        if i >= visible.start
            && let (Some(prev), Some(t)) = (slots.time_at(i - stride), slots.time_at(i))
            && let Some(w) = boundary_weight(prev, t)
        {
            // With stride > 1 the boundary lies somewhere in (i - stride, i];
            // put the tick on the exact first slot past it (12:00, not 12:15).
            // The weight from a fixed `prev` only grows with time, so bisect.
            let (mut lo, mut hi) = (i - stride, i);
            while hi - lo > 1 {
                let mid = lo + (hi - lo) / 2;
                let reached = slots
                    .time_at(mid)
                    .and_then(|tm| boundary_weight(prev, tm))
                    .is_some_and(|wm| wm >= w);
                if reached {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            if hi >= visible.start {
                cands.push((w, hi));
            }
        }
        i += stride;
    }
    // Use one regular unit: drop weights so fine that their ticks could not
    // all fit (otherwise leftovers like "08:52" fill the gaps unevenly).
    let span = (x_range.1 - x_range.0).max(1.0);
    let mut counts = [0usize; WEIGHT_COUNT];
    for (w, _) in &cands {
        counts[*w as usize] += 1;
    }
    let mut threshold = TickWeight::Year as usize;
    let mut needed = 0.0;
    for w in (0..counts.len()).rev() {
        // Typical label width for this unit.
        let sample = match w {
            x if x == TickWeight::Year as usize => "2026",
            x if x >= TickWeight::Month as usize => "May",
            x if x >= TickWeight::Day as usize => "28",
            _ => "00:00",
        };
        needed += counts[w] as f32 * (label_width(sample) + min_gap);
        if needed > span {
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
        let label = tick_label(slots.time_at(i).expect("candidates have times"), weight);
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
            major: false,
        });
    }
    placed.sort_by(|a, b| a.x.total_cmp(&b.x));
    // The lightest weight on screen is the regular unit; heavier ones are major.
    if let Some(min) = placed.iter().map(|t| t.weight).min() {
        for t in &mut placed {
            t.major = t.weight > min;
        }
    }
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
        // 19th -> 20th crosses a 5-day mark.
        assert_eq!(boundary_weight(NOV20 - m, NOV20), Some(TickWeight::Day5));
        // 2026-12-01 and 2027-01-01
        assert_eq!(
            boundary_weight(NOV20, NOV20 + 11 * DAY_MS),
            Some(TickWeight::Month)
        );
        assert_eq!(
            boundary_weight(NOV20, NOV20 + 5 * DAY_MS),
            Some(TickWeight::Day5)
        ); // 25th
        assert_eq!(
            boundary_weight(NOV20, NOV20 + DAY_MS),
            Some(TickWeight::Day)
        ); // 21st
        // 2026-09-30 -> 2026-10-01 starts Q4.
        assert_eq!(
            boundary_weight(NOV20 - 51 * DAY_MS, NOV20 - 50 * DAY_MS),
            Some(TickWeight::Quarter)
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
            let x_of = |i: i64| i as f32 * spacing;
            let width = times.len() as f32 * spacing;
            let stride = (3.0 / spacing).ceil() as usize;
            let ticks = time_ticks(
                &SlotTimes::new(&times, MINUTE_MS),
                0..times.len() as i64,
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
            &SlotTimes::new(&times, MINUTE_MS),
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
    fn multi_year_range_shows_months() {
        // Two years of daily bars on 1200 px: months fit, days don't.
        let times: Vec<i64> = (0..730).map(|i| NOV20 + i * crate::scale::DAY_MS).collect();
        let spacing = 1200.0 / 730.0;
        let ticks = time_ticks(
            &SlotTimes::new(&times, MINUTE_MS),
            0..730,
            (0.0, 1200.0),
            2,
            18.0,
            |i| i as f32 * spacing,
            |s| s.len() as f32 * 7.0,
        );
        let months = ticks
            .iter()
            .filter(|t| t.weight >= TickWeight::Month && t.weight < TickWeight::Year)
            .count();
        assert!(months >= 8, "{ticks:?}");
        assert!(ticks.iter().any(|t| t.weight == TickWeight::Year));
        assert!(ticks.iter().all(|t| t.weight >= TickWeight::Month));
    }

    #[test]
    fn million_minutes_show_months() {
        // ~2.7 years of weekday minutes squeezed into 1200 px, sampled with a stride.
        let day = crate::scale::DAY_MS;
        let mut times = Vec::with_capacity(1_000_000);
        let mut t = 1_704_067_200_000_i64; // 2024-01-01, Monday
        while times.len() < 1_000_000 {
            if !matches!(t.div_euclid(day).rem_euclid(7), 2 | 3) {
                times.push(t);
            }
            t += MINUTE_MS;
        }
        let spacing = 1200.0 / times.len() as f32;
        let stride = (3.0 / spacing).ceil() as usize;
        let ticks = time_ticks(
            &SlotTimes::new(&times, MINUTE_MS),
            0..times.len() as i64,
            (0.0, 1200.0),
            stride,
            18.0,
            |i| i as f32 * spacing,
            |s| s.len() as f32 * 7.0,
        );
        // Too long for every month: quarters (Apr/Jul/Oct) between the years.
        let quarters = ticks
            .iter()
            .filter(|t| t.weight == TickWeight::Quarter)
            .count();
        assert!(quarters >= 6, "{ticks:?}");
        assert!(ticks.iter().all(|t| t.weight >= TickWeight::Quarter));
    }

    #[test]
    fn hours_are_major_among_half_hours() {
        // 5m bars, ~80 px apart per 30 min: half-hour labels, hours emphasized.
        let times: Vec<i64> = (0..200).map(|i| NOV20 + i * 5 * MINUTE_MS).collect();
        let spacing = 80.0 / 6.0;
        let ticks = time_ticks(
            &SlotTimes::new(&times, MINUTE_MS),
            1..200,
            (0.0, 200.0 * spacing),
            1,
            45.0,
            |i| i as f32 * spacing,
            |s| s.len() as f32 * 7.0,
        );
        assert!(
            ticks.iter().any(|t| t.label.ends_with(":30") && !t.major),
            "{ticks:?}"
        );
        assert!(
            ticks
                .iter()
                .filter(|t| t.label.ends_with(":00"))
                .all(|t| t.major),
            "{ticks:?}"
        );
    }

    #[test]
    fn slots_extrapolate_beyond_the_data() {
        let times = [NOV20, NOV20 + 5 * MINUTE_MS, NOV20 + 10 * MINUTE_MS];
        let slots = SlotTimes::new(&times, 5 * MINUTE_MS);
        assert_eq!(slots.time_at(1), Some(NOV20 + 5 * MINUTE_MS));
        assert_eq!(slots.time_at(5), Some(NOV20 + 25 * MINUTE_MS));
        assert_eq!(slots.time_at(-2), Some(NOV20 - 10 * MINUTE_MS));
        assert_eq!(SlotTimes::new(&[], 1).time_at(0), None);
    }

    #[test]
    fn ticks_continue_into_empty_space_after_the_last_bar() {
        // 5m bars until 08:40; the view shows slots up to ~10:30, like TradingView.
        let times: Vec<i64> = (0..=104).map(|k| NOV20 + k * 5 * MINUTE_MS).collect();
        let slots = SlotTimes::new(&times, 5 * MINUTE_MS);
        let spacing = 85.0 / 6.0;
        let ticks = time_ticks(
            &slots,
            1..131,
            (0.0, 131.0 * spacing),
            1,
            45.0,
            |i| i as f32 * spacing,
            |s| s.len() as f32 * 7.0,
        );
        let future: Vec<&str> = ticks
            .iter()
            .filter(|t| t.index > 104)
            .map(|t| t.label.as_str())
            .collect();
        assert_eq!(future, ["09:00", "09:30", "10:00", "10:30"], "{ticks:?}");
    }

    #[test]
    fn strided_ticks_land_on_the_exact_boundary() {
        // 5m bars from 11:10, sampled every 4 slots: the 12:00 boundary is
        // found between samples and must be labelled 12:00, not 12:15.
        let start = NOV20 + 11 * HOUR_MS + 10 * MINUTE_MS;
        let times: Vec<i64> = (0..40).map(|k| start + k * 5 * MINUTE_MS).collect();
        let slots = SlotTimes::new(&times, 5 * MINUTE_MS);
        let ticks = time_ticks(
            &slots,
            1..40,
            (0.0, 4000.0),
            4,
            10.0,
            |i| i as f32 * 100.0,
            |s| s.len() as f32 * 7.0,
        );
        let noon = ticks
            .iter()
            .find(|t| t.weight == TickWeight::Hour12)
            .expect("noon tick");
        assert_eq!(noon.label, "12:00");
        assert_eq!(slots.time_at(noon.index), Some(NOV20 + 12 * HOUR_MS));
    }

    #[test]
    fn gap_takes_no_space() {
        let times = bars_with_gap();
        // Bars 599 and 600 are 3 days apart in time but adjacent on screen.
        let x_of = |i: i64| i as f32 * 4.0;
        assert_eq!(x_of(600) - x_of(599), 4.0);
        let ticks = time_ticks(
            &SlotTimes::new(&times, MINUTE_MS),
            550..650,
            (0.0, 1e6),
            1,
            10.0,
            x_of,
            |s| s.len() as f32 * 7.0,
        );
        assert!(
            ticks
                .iter()
                .any(|t| t.index == 600 && t.weight == TickWeight::Day)
        );
    }

    #[test]
    fn candidates_are_stable_under_panning() {
        let times = bars_with_gap();
        let tick_idx = |range: Range<i64>| {
            time_ticks(
                &SlotTimes::new(&times, MINUTE_MS),
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
        // Panning by a few slots must not move any tick that stays in view:
        // ticks sit on exact time boundaries, which depend only on the data.
        let a = tick_idx(100..500);
        for shift in 1..8 {
            let b = tick_idx(100 + shift..500 + shift);
            for i in a.iter().filter(|i| **i >= 108 && **i < 500) {
                assert!(
                    b.contains(i),
                    "tick {i} moved after panning by {shift}: {a:?} vs {b:?}"
                );
            }
        }
        // And every tick is on an actual boundary (its slot starts a new unit).
        let slots = SlotTimes::new(&times, MINUTE_MS);
        for i in &a {
            let w = boundary_weight(slots.time_at(i - 1).unwrap(), slots.time_at(*i).unwrap());
            assert!(w.is_some(), "tick {i} is not on a boundary");
        }
    }
}
