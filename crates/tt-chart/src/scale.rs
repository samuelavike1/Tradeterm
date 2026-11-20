//! Axis tick selection and label formatting. Backend agnostic.

/// A "nice" step (1, 2 or 5 × 10ⁿ) that splits `span` into roughly `target` intervals.
pub fn nice_step(span: f64, target: f64) -> f64 {
    if !(span > 0.0 && span.is_finite()) || target <= 0.0 {
        return 1.0;
    }
    let raw = span / target;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let nice = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * mag
}

/// Multiples of `step` within `[lo, hi]`.
pub fn ticks(lo: f64, hi: f64, step: f64) -> impl Iterator<Item = f64> {
    let first = (lo / step).ceil() as i64;
    let last = (hi / step).floor() as i64;
    (first..=last.max(first - 1)).map(move |k| k as f64 * step)
}

/// Decimal places used for price labels, picked from the price magnitude.
pub fn price_decimals(price: f64) -> usize {
    let p = price.abs();
    if !p.is_finite() || p >= 1.0 || p == 0.0 {
        2
    } else {
        // 0.5 → 4, 0.012 → 5, 0.00001234 → 8
        ((-p.log10()).ceil() as usize + 3).min(10)
    }
}

pub fn format_price(price: f64, decimals: usize) -> String {
    format!("{price:.decimals$}")
}

/// Compact volume, e.g. `1.23K`, `4.56M`.
pub fn format_volume(v: f64) -> String {
    let a = v.abs();
    if a >= 1e9 {
        format!("{:.2}B", v / 1e9)
    } else if a >= 1e6 {
        format!("{:.2}M", v / 1e6)
    } else if a >= 1e3 {
        format!("{:.2}K", v / 1e3)
    } else {
        format!("{v:.2}")
    }
}

pub const MINUTE_MS: i64 = 60_000;
pub const HOUR_MS: i64 = 60 * MINUTE_MS;
pub const DAY_MS: i64 = 24 * HOUR_MS;

/// Candidate spacings between time-axis labels, in ms.
const TIME_STEPS: [i64; 18] = [
    MINUTE_MS,
    2 * MINUTE_MS,
    5 * MINUTE_MS,
    10 * MINUTE_MS,
    15 * MINUTE_MS,
    30 * MINUTE_MS,
    HOUR_MS,
    2 * HOUR_MS,
    3 * HOUR_MS,
    4 * HOUR_MS,
    6 * HOUR_MS,
    12 * HOUR_MS,
    DAY_MS,
    2 * DAY_MS,
    5 * DAY_MS,
    10 * DAY_MS,
    30 * DAY_MS,
    90 * DAY_MS,
];

/// Smallest label step (ms) that is a multiple of the bar interval and keeps
/// labels at least `min_px` apart.
pub fn time_label_step(bar_interval_ms: i64, bar_spacing: f32, min_px: f32) -> i64 {
    let interval = bar_interval_ms.max(1);
    TIME_STEPS
        .iter()
        .copied()
        .filter(|s| *s >= interval && s % interval == 0)
        .find(|s| (s / interval) as f32 * bar_spacing >= min_px)
        .unwrap_or_else(|| {
            // Beyond the table: whole multiples of 90 days.
            let bars = (min_px / bar_spacing.max(0.01)).ceil() as i64;
            let ms = bars * interval;
            let q = 90 * DAY_MS;
            ((ms + q - 1) / q).max(1) * q
        })
}

/// UTC calendar date-time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

impl DateTime {
    pub fn from_unix_ms(ms: i64) -> Self {
        let days = ms.div_euclid(DAY_MS);
        let rem = ms.rem_euclid(DAY_MS);
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month,
            day,
            hour: (rem / HOUR_MS) as u32,
            minute: ((rem % HOUR_MS) / MINUTE_MS) as u32,
        }
    }

    pub fn month_name(&self) -> &'static str {
        const M: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        M[(self.month as usize - 1) % 12]
    }
}

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Short time-axis label: `HH:MM` within a day, `20 Nov` at day boundaries,
/// the year at year boundaries.
pub fn format_axis_time(ms: i64, label_step_ms: i64) -> String {
    let dt = DateTime::from_unix_ms(ms);
    let midnight = ms.rem_euclid(DAY_MS) == 0;
    if label_step_ms < DAY_MS && !midnight {
        format!("{:02}:{:02}", dt.hour, dt.minute)
    } else if dt.month == 1 && dt.day == 1 {
        dt.year.to_string()
    } else {
        format!("{} {}", dt.day, dt.month_name())
    }
}

/// Full crosshair label: `20 Nov 2026 14:05` (time omitted for daily+ bars).
pub fn format_full_time(ms: i64, bar_interval_ms: i64) -> String {
    let dt = DateTime::from_unix_ms(ms);
    if bar_interval_ms >= DAY_MS {
        format!("{} {} {}", dt.day, dt.month_name(), dt.year)
    } else {
        format!(
            "{} {} {}  {:02}:{:02}",
            dt.day,
            dt.month_name(),
            dt.year,
            dt.hour,
            dt.minute
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_steps() {
        assert_eq!(nice_step(100.0, 10.0), 10.0);
        assert_eq!(nice_step(100.0, 6.0), 20.0);
        assert_eq!(nice_step(100.0, 30.0), 5.0);
        assert!((nice_step(0.01, 5.0) - 0.002).abs() < 1e-12);
    }

    #[test]
    fn tick_values() {
        let t: Vec<f64> = ticks(3.0, 27.0, 10.0).collect();
        assert_eq!(t, [10.0, 20.0]);
        assert_eq!(ticks(1.0, 2.0, 10.0).count(), 0);
    }

    #[test]
    fn decimals() {
        assert_eq!(price_decimals(61_000.0), 2);
        assert_eq!(price_decimals(0.5), 4);
        assert_eq!(price_decimals(0.000_012_34), 8);
        assert_eq!(format_price(1.5, 2), "1.50");
    }

    #[test]
    fn dates() {
        // 2026-11-20 14:05 UTC
        let ms = 1_795_183_500_000;
        let dt = DateTime::from_unix_ms(ms);
        assert_eq!(
            (dt.year, dt.month, dt.day, dt.hour, dt.minute),
            (2026, 11, 20, 14, 5)
        );
        assert_eq!(format_full_time(ms, MINUTE_MS), "20 Nov 2026  14:05");
        assert_eq!(format_axis_time(ms, HOUR_MS), "14:05");
        assert_eq!(DateTime::from_unix_ms(0).year, 1970);
        assert_eq!(format_axis_time(1_767_225_600_000, DAY_MS), "2026"); // 2026-01-01
        assert_eq!(format_axis_time(1_795_132_800_000, HOUR_MS), "20 Nov"); // midnight
    }

    #[test]
    fn label_steps() {
        // 1m bars, 8px apart, need 80px → 10 bars → 10 minutes.
        assert_eq!(time_label_step(MINUTE_MS, 8.0, 80.0), 10 * MINUTE_MS);
        // 4h bars → steps must be multiples of 4h.
        assert_eq!(time_label_step(4 * HOUR_MS, 30.0, 80.0), 12 * HOUR_MS);
        // Daily bars zoomed far out fall back to 90-day multiples.
        assert_eq!(time_label_step(DAY_MS, 0.5, 80.0), 180 * DAY_MS);
    }
}
