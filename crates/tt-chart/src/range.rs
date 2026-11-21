//! Date ranges and view targets: TradingView's bottom toolbar
//! (docs/specs/bottom-panel.md).
//!
//! - [`DateRange`]: the `1D 5D 1M 3M 6M YTD 1Y 5Y All` buttons, each with the
//!   interval TradingView switches to and the span it shows.
//! - [`ViewTarget`]: what [`crate::ChartView::show_target`] scrolls / zooms
//!   to once the bars are there (a range button, the Go to dialog's date or
//!   custom range).
//! - Pure math for both: span start in the chart's zone ([`DateRange::start`]),
//!   fitting a slot span into the plot ([`fit_span`]), centering a slot
//!   ([`center_offset`]) and the Go to dialog's input parsing
//!   ([`parse_date_time`]).

use crate::TzOffsets;
use tt_core::{TimeUnit, Timeframe, civil_from_days, days_from_civil};

const DAY_MS: i64 = 86_400_000;

/// A bottom-toolbar range button (TradingView's date ranges).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateRange {
    D1,
    D5,
    M1,
    M3,
    M6,
    Ytd,
    Y1,
    Y5,
    All,
}

impl DateRange {
    /// Left to right, as on TradingView.
    pub const ALL: [DateRange; 9] = [
        DateRange::D1,
        DateRange::D5,
        DateRange::M1,
        DateRange::M3,
        DateRange::M6,
        DateRange::Ytd,
        DateRange::Y1,
        DateRange::Y5,
        DateRange::All,
    ];

    /// Button text.
    pub fn label(self) -> &'static str {
        match self {
            DateRange::D1 => "1D",
            DateRange::D5 => "5D",
            DateRange::M1 => "1M",
            DateRange::M3 => "3M",
            DateRange::M6 => "6M",
            DateRange::Ytd => "YTD",
            DateRange::Y1 => "1Y",
            DateRange::Y5 => "5Y",
            DateRange::All => "All",
        }
    }

    /// TradingView's tooltip (read from its `data-tooltip`, 2026-11).
    pub fn tooltip(self) -> &'static str {
        match self {
            DateRange::D1 => "1 day in 1 minute intervals",
            DateRange::D5 => "5 days in 5 minutes intervals",
            DateRange::M1 => "1 month in 30 minutes intervals",
            DateRange::M3 => "3 months in 1 hour intervals",
            DateRange::M6 => "6 months in 2 hours intervals",
            DateRange::Ytd => "Year to day in 1 day intervals",
            DateRange::Y1 => "1 year in 1 day intervals",
            DateRange::Y5 => "5 years in 1 week intervals",
            DateRange::All => "All data in 1 month intervals",
        }
    }

    /// The interval TradingView switches to.
    pub fn timeframe(self) -> Timeframe {
        match self {
            DateRange::D1 => Timeframe::M1,
            DateRange::D5 => Timeframe::M5,
            DateRange::M1 => Timeframe::M30,
            DateRange::M3 => Timeframe::H1,
            DateRange::M6 => Timeframe::new(TimeUnit::Hour, 2),
            DateRange::Ytd | DateRange::Y1 => Timeframe::D1,
            DateRange::Y5 => Timeframe::W1,
            DateRange::All => Timeframe::MN1,
        }
    }

    /// Start of the span that ends at `end_ms` (UTC), with calendar months
    /// and years in `tz` (a month back from 31 March is the last day of
    /// February). `None` for All (everything the venue has).
    pub fn start(self, end_ms: i64, tz: &TzOffsets) -> Option<i64> {
        Some(match self {
            DateRange::D1 => end_ms - DAY_MS,
            DateRange::D5 => end_ms - 5 * DAY_MS,
            DateRange::M1 => months_back(end_ms, 1, tz),
            DateRange::M3 => months_back(end_ms, 3, tz),
            DateRange::M6 => months_back(end_ms, 6, tz),
            DateRange::Y1 => months_back(end_ms, 12, tz),
            DateRange::Y5 => months_back(end_ms, 60, tz),
            DateRange::Ytd => {
                let local = tz.local_ms(end_ms);
                let (y, _, _) = civil_from_days(local.div_euclid(DAY_MS));
                to_utc(days_from_civil(y, 1, 1) * DAY_MS, tz)
            }
            DateRange::All => return None,
        })
    }
}

/// `months` calendar months before `utc_ms`, same wall-clock time in `tz`
/// (the day clamped to the target month's length).
pub fn months_back(utc_ms: i64, months: u32, tz: &TzOffsets) -> i64 {
    let local = tz.local_ms(utc_ms);
    let days = local.div_euclid(DAY_MS);
    let tod = local.rem_euclid(DAY_MS);
    let (y, m, d) = civil_from_days(days);
    let total = y * 12 + (m as i64 - 1) - months as i64;
    let (ty, tm) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
    let d = d.min(days_in_month(ty, tm));
    to_utc(days_from_civil(ty, tm, d) * DAY_MS + tod, tz)
}

fn days_in_month(y: i64, m: u32) -> u32 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    (days_from_civil(ny, nm, 1) - days_from_civil(y, m, 1)) as u32
}

/// UTC time of the wall-clock time `local_ms` in `tz` (the offset in effect
/// there; ambiguous / skipped DST hours resolve to the earlier offset).
pub fn to_utc(local_ms: i64, tz: &TzOffsets) -> i64 {
    let guess = local_ms - tz.offset_ms(local_ms);
    local_ms - tz.offset_ms(guess)
}

/// Where the chart should go once its bars are loaded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewTarget {
    /// A range button: its span ending at the newest bar.
    Range(DateRange),
    /// Go to › Custom range: `from..to` (UTC ms) fills the plot.
    Span { from: i64, to: i64 },
    /// Go to › Date: the bar at `time` (UTC ms) in the middle, zoom kept.
    Date(i64),
}

/// Bar spacing and right offset (in bars) that make slots
/// `slot_from..slot_to` fill a plot `plot_w` points wide, for a series of
/// `n` bars. Slot `i` is bar `i`'s open time (bar `i` spans
/// `i..i + 1`), so the span's right end at slot `n` (the newest bar's
/// close) means a right offset of 0. Spacing is clamped to
/// `min_spacing..=max_spacing`.
pub fn fit_span(
    slot_from: f64,
    slot_to: f64,
    n: usize,
    plot_w: f32,
    min_spacing: f32,
    max_spacing: f32,
) -> (f32, f32) {
    let bars = (slot_to - slot_from).max(1.0) as f32;
    let spacing = (plot_w / bars).clamp(min_spacing, max_spacing);
    (spacing, (slot_to - n as f64) as f32)
}

/// Right offset (in bars) that puts the bar containing slot `slot` in the
/// middle of a plot `plot_w` wide at `spacing`, for `n` bars.
pub fn center_offset(slot: f64, n: usize, plot_w: f32, spacing: f32) -> f32 {
    let bar = slot.floor() as f32;
    // Bar `bar`'s center is index `bar`; the right edge index is
    // `n - 1 + offset + 0.5`.
    bar + plot_w / spacing / 2.0 - n as f32 + 0.5
}

/// The Go to dialog's fields: `YYYY-MM-DD` and an optional `HH:MM` (empty
/// = 00:00) as a wall-clock time in ms since the local epoch. `None` when
/// either is malformed or the date does not exist.
pub fn parse_date_time(date: &str, time: &str) -> Option<i64> {
    let date = date.trim();
    let time = time.trim();
    // Only a date and hours:minutes (no seconds, no "T").
    if date.contains([' ', 'T']) || time.matches(':').count() > 1 {
        return None;
    }
    let s = if time.is_empty() {
        date.to_owned()
    } else {
        format!("{date} {time}")
    };
    crate::drawings::form::parse_time(&s)
}

/// `YYYY-MM-DD` and `HH:MM` of a wall-clock time (inverse of
/// [`parse_date_time`]).
pub fn format_date_time(local_ms: i64) -> (String, String) {
    let (y, m, d) = civil_from_days(local_ms.div_euclid(DAY_MS));
    let min = local_ms.rem_euclid(DAY_MS) / 60_000;
    (
        format!("{y:04}-{m:02}-{d:02}"),
        format!("{:02}:{:02}", min / 60, min % 60),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-11-21 15:30 UTC.
    const NOW: i64 = 1_795_275_000_000;

    fn ymd_hm(y: i64, m: u32, d: u32, h: i64, mi: i64) -> i64 {
        days_from_civil(y, m, d) * DAY_MS + (h * 60 + mi) * 60_000
    }

    #[test]
    fn range_intervals_match_tradingview() {
        let codes: Vec<String> = DateRange::ALL
            .iter()
            .map(|r| r.timeframe().code())
            .collect();
        assert_eq!(
            codes,
            ["1m", "5m", "30m", "1h", "2h", "1D", "1D", "1W", "1M"]
        );
        let labels: Vec<&str> = DateRange::ALL.iter().map(|r| r.label()).collect();
        assert_eq!(
            labels,
            ["1D", "5D", "1M", "3M", "6M", "YTD", "1Y", "5Y", "All"]
        );
        assert_eq!(DateRange::M6.tooltip(), "6 months in 2 hours intervals");
    }

    #[test]
    fn range_starts() {
        let utc = TzOffsets::utc();
        assert_eq!(NOW, ymd_hm(2026, 11, 21, 15, 30));
        assert_eq!(DateRange::D1.start(NOW, &utc), Some(NOW - DAY_MS));
        assert_eq!(DateRange::D5.start(NOW, &utc), Some(NOW - 5 * DAY_MS));
        assert_eq!(
            DateRange::M1.start(NOW, &utc),
            Some(ymd_hm(2026, 10, 21, 15, 30))
        );
        assert_eq!(
            DateRange::M6.start(NOW, &utc),
            Some(ymd_hm(2026, 5, 21, 15, 30))
        );
        assert_eq!(
            DateRange::Y1.start(NOW, &utc),
            Some(ymd_hm(2025, 11, 21, 15, 30))
        );
        assert_eq!(
            DateRange::Y5.start(NOW, &utc),
            Some(ymd_hm(2021, 11, 21, 15, 30))
        );
        assert_eq!(
            DateRange::Ytd.start(NOW, &utc),
            Some(ymd_hm(2026, 1, 1, 0, 0))
        );
        assert_eq!(DateRange::All.start(NOW, &utc), None);
    }

    #[test]
    fn month_end_clamps_and_zone_is_wall_clock() {
        let utc = TzOffsets::utc();
        // 31 March → 28 February (2026 is not a leap year), 2028 is.
        assert_eq!(
            months_back(ymd_hm(2026, 3, 31, 0, 0), 1, &utc),
            ymd_hm(2026, 2, 28, 0, 0)
        );
        assert_eq!(
            months_back(ymd_hm(2028, 3, 31, 0, 0), 1, &utc),
            ymd_hm(2028, 2, 29, 0, 0)
        );
        // January → December of the year before.
        assert_eq!(
            months_back(ymd_hm(2026, 1, 15, 0, 0), 3, &utc),
            ymd_hm(2025, 10, 15, 0, 0)
        );
        // UTC-5: YTD starts at local midnight, 05:00 UTC; on 1 Jan 02:00
        // UTC it is still the old year locally.
        let ny = TzOffsets::fixed(-5 * 3_600_000);
        assert_eq!(
            DateRange::Ytd.start(NOW, &ny),
            Some(ymd_hm(2026, 1, 1, 5, 0))
        );
        assert_eq!(
            DateRange::Ytd.start(ymd_hm(2026, 1, 1, 2, 0), &ny),
            Some(ymd_hm(2025, 1, 1, 5, 0))
        );
    }

    #[test]
    fn to_utc_inverts_local_across_a_transition() {
        // +1 h until t0, +2 h after.
        let t0 = ymd_hm(2026, 3, 29, 1, 0);
        let tz = TzOffsets::from_transitions(vec![(i64::MIN, 3_600_000), (t0, 7_200_000)]);
        for utc in [t0 - 5 * 3_600_000, t0 + 5 * 3_600_000] {
            assert_eq!(to_utc(tz.local_ms(utc), &tz), utc);
        }
    }

    #[test]
    fn fit_span_fills_the_plot() {
        // 100 bars, the last 50 slots (to the newest close) in 1000 pt.
        let (spacing, offset) = fit_span(50.0, 100.0, 100, 1000.0, 0.01, 80.0);
        assert_eq!(spacing, 20.0);
        assert_eq!(offset, 0.0);
        // The span's left edge lands on the plot's left edge: the right
        // edge index is n - 1 + offset + 0.5 = 99.5; minus 1000 / 20 bars
        // = 49.5, the left edge of bar 50.
        assert_eq!(99.5 + offset - 1000.0 / spacing, 49.5);
        // A span ending inside the data scrolls back.
        let (_, offset) = fit_span(10.0, 60.0, 100, 1000.0, 0.01, 80.0);
        assert_eq!(offset, -40.0);
        // Too few bars: spacing is capped.
        let (spacing, _) = fit_span(98.0, 100.0, 100, 1000.0, 0.01, 80.0);
        assert_eq!(spacing, 80.0);
        // 1D on 1m in a narrow plot: below one point per bar is fine.
        let (spacing, _) = fit_span(0.0, 1440.0, 1440, 720.0, 0.0005, 80.0);
        assert_eq!(spacing, 0.5);
    }

    #[test]
    fn center_offset_centers_the_bar() {
        let (n, w, s) = (200, 800.0, 10.0);
        let off = center_offset(120.4, n, w, s);
        // Center x of the plot ↔ index right_edge - w / s / 2.
        let right_edge = n as f32 - 1.0 + off + 0.5;
        assert_eq!(right_edge - w / s / 2.0, 120.0);
    }

    #[test]
    fn go_to_parsing() {
        assert_eq!(
            parse_date_time("2026-11-21", "10:30"),
            Some(ymd_hm(2026, 11, 21, 10, 30))
        );
        assert_eq!(
            parse_date_time(" 2026-01-05 ", ""),
            Some(ymd_hm(2026, 1, 5, 0, 0))
        );
        assert_eq!(
            parse_date_time("2026-1-5", "9:05"),
            Some(ymd_hm(2026, 1, 5, 9, 5))
        );
        for (d, t) in [
            ("2026-02-30", "00:00"),
            ("2026-13-01", ""),
            ("2026/11/21", ""),
            ("2026-11-21", "24:00"),
            ("2026-11-21", "10:60"),
            ("2026-11-21", "10:30:15"),
            ("2026-11-21 10:00", ""),
            ("", ""),
            ("abc", "10:00"),
        ] {
            assert_eq!(parse_date_time(d, t), None, "{d:?} {t:?}");
        }
        let t = ymd_hm(2026, 11, 21, 10, 30);
        assert_eq!(
            format_date_time(t),
            ("2026-11-21".to_owned(), "10:30".to_owned())
        );
        let (d, tm) = format_date_time(t);
        assert_eq!(parse_date_time(&d, &tm), Some(t));
    }
}
