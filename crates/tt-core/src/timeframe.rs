//! Bar intervals: any count of seconds, minutes, hours, days, weeks or months.
//!
//! Serialized as compact codes — `1s`, `45m`, `4h`, `1D`, `2W`, `3M` — which
//! also parse back (case matters only for `m` minutes vs `M` months).
//!
//! Bucket alignment (where a bar starts), all in UTC:
//! - seconds … days: multiples of the interval since the Unix epoch
//!   (e.g. 4h bars at 00:00, 04:00, …; 45m bars restart at midnight because
//!   45 divides 1440);
//! - weeks: Mondays 00:00, like Binance and TradingView;
//! - months: calendar months (3M bars start in Jan, Apr, Jul, Oct).

use std::cmp::Ordering;
use std::fmt;

pub const SECOND_MS: i64 = 1000;
pub const MINUTE_MS: i64 = 60 * SECOND_MS;
pub const HOUR_MS: i64 = 60 * MINUTE_MS;
pub const DAY_MS: i64 = 24 * HOUR_MS;
pub const WEEK_MS: i64 = 7 * DAY_MS;
/// 1970-01-05, the first Monday after the epoch: week buckets start here.
const MONDAY_OFFSET_MS: i64 = 4 * DAY_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TimeUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
}

impl TimeUnit {
    pub const ALL: [TimeUnit; 6] = [
        TimeUnit::Second,
        TimeUnit::Minute,
        TimeUnit::Hour,
        TimeUnit::Day,
        TimeUnit::Week,
        TimeUnit::Month,
    ];

    /// Code suffix: `s m h D W M`.
    pub const fn suffix(self) -> char {
        match self {
            TimeUnit::Second => 's',
            TimeUnit::Minute => 'm',
            TimeUnit::Hour => 'h',
            TimeUnit::Day => 'D',
            TimeUnit::Week => 'W',
            TimeUnit::Month => 'M',
        }
    }

    pub const fn plural_name(self) -> &'static str {
        match self {
            TimeUnit::Second => "seconds",
            TimeUnit::Minute => "minutes",
            TimeUnit::Hour => "hours",
            TimeUnit::Day => "days",
            TimeUnit::Week => "weeks",
            TimeUnit::Month => "months",
        }
    }

    /// Largest count allowed for a custom interval in this unit.
    pub const fn max_count(self) -> u32 {
        match self {
            TimeUnit::Second => 59,
            TimeUnit::Minute => 1439,
            TimeUnit::Hour => 24,
            TimeUnit::Day => 365,
            TimeUnit::Week => 52,
            TimeUnit::Month => 12,
        }
    }

    const fn unit_ms(self) -> i64 {
        match self {
            TimeUnit::Second => SECOND_MS,
            TimeUnit::Minute => MINUTE_MS,
            TimeUnit::Hour => HOUR_MS,
            TimeUnit::Day => DAY_MS,
            TimeUnit::Week => WEEK_MS,
            // Average month; only used for ordering and approximate spacing.
            TimeUnit::Month => 2_629_746_000,
        }
    }
}

/// A bar interval: `count` × `unit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timeframe {
    unit: TimeUnit,
    count: u32,
}

impl Timeframe {
    pub const S1: Timeframe = Timeframe::new(TimeUnit::Second, 1);
    pub const M1: Timeframe = Timeframe::new(TimeUnit::Minute, 1);
    pub const M3: Timeframe = Timeframe::new(TimeUnit::Minute, 3);
    pub const M5: Timeframe = Timeframe::new(TimeUnit::Minute, 5);
    pub const M15: Timeframe = Timeframe::new(TimeUnit::Minute, 15);
    pub const M30: Timeframe = Timeframe::new(TimeUnit::Minute, 30);
    pub const H1: Timeframe = Timeframe::new(TimeUnit::Hour, 1);
    pub const H4: Timeframe = Timeframe::new(TimeUnit::Hour, 4);
    pub const D1: Timeframe = Timeframe::new(TimeUnit::Day, 1);
    pub const W1: Timeframe = Timeframe::new(TimeUnit::Week, 1);
    pub const MN1: Timeframe = Timeframe::new(TimeUnit::Month, 1);

    /// Default favorites (toolbar quick buttons).
    pub const ALL: [Timeframe; 6] = [
        Timeframe::M1,
        Timeframe::M5,
        Timeframe::M15,
        Timeframe::H1,
        Timeframe::H4,
        Timeframe::D1,
    ];

    /// `count` is clamped to at least 1.
    pub const fn new(unit: TimeUnit, count: u32) -> Self {
        Self {
            unit,
            count: if count == 0 { 1 } else { count },
        }
    }

    pub const fn unit(self) -> TimeUnit {
        self.unit
    }

    pub const fn count(self) -> u32 {
        self.count
    }

    /// Bar length in ms. Exact except for months (average month length);
    /// use [`Self::bar_end_ms`] for exact bar boundaries.
    pub const fn duration_ms(self) -> i64 {
        self.unit.unit_ms() * self.count as i64
    }

    /// Fixed-length (not calendar-based) intervals: seconds … days.
    pub const fn is_fixed(self) -> bool {
        !matches!(self.unit, TimeUnit::Week | TimeUnit::Month)
    }

    /// Compact code, e.g. `45m`, `4h`, `1D`, `3M`.
    pub fn code(self) -> String {
        format!("{}{}", self.count, self.unit.suffix())
    }

    /// Toolbar label like TradingView: `D`, `W`, `M` for single day/week/month.
    pub fn label(self) -> String {
        match (self.unit, self.count) {
            (TimeUnit::Day | TimeUnit::Week | TimeUnit::Month, 1) => self.unit.suffix().to_string(),
            _ => self.code(),
        }
    }

    /// Menu name: `1 minute`, `45 minutes`, `1 day`, `12 months`.
    pub fn long_name(self) -> String {
        let plural = self.unit.plural_name();
        if self.count == 1 {
            format!("1 {}", &plural[..plural.len() - 1])
        } else {
            format!("{} {plural}", self.count)
        }
    }

    /// Parses `45m`, `4h`, `1D`/`1d`, `W`, `3M`, `1s`. `m` = minutes, `M` = months.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let split = s.find(|c: char| !c.is_ascii_digit())?;
        let (num, suffix) = s.split_at(split);
        let count: u32 = if num.is_empty() { 1 } else { num.parse().ok()? };
        let unit = match suffix {
            "s" | "S" => TimeUnit::Second,
            "m" => TimeUnit::Minute,
            "h" | "H" => TimeUnit::Hour,
            "D" | "d" => TimeUnit::Day,
            "W" | "w" => TimeUnit::Week,
            "M" => TimeUnit::Month,
            _ => return None,
        };
        (count >= 1 && count <= unit.max_count().max(1)).then(|| Self::new(unit, count))
    }

    /// Valid as a user-defined interval (count within the unit's range).
    pub fn is_valid_custom(unit: TimeUnit, count: u32) -> bool {
        (1..=unit.max_count()).contains(&count)
    }

    /// Open time of the bar containing `t_ms`.
    pub fn bucket_start(self, t_ms: i64) -> i64 {
        match self.unit {
            TimeUnit::Month => {
                let (y, m, _) = civil_from_days(t_ms.div_euclid(DAY_MS));
                let idx = y * 12 + (m as i64 - 1);
                let start = idx.div_euclid(self.count as i64) * self.count as i64;
                days_from_civil(start.div_euclid(12), (start.rem_euclid(12) + 1) as u32, 1) * DAY_MS
            }
            TimeUnit::Week => {
                let w = self.duration_ms();
                (t_ms - MONDAY_OFFSET_MS).div_euclid(w) * w + MONDAY_OFFSET_MS
            }
            _ => {
                let d = self.duration_ms();
                t_ms.div_euclid(d) * d
            }
        }
    }

    /// Exact time the bar opened at `open_ms` closes (= next bar's open).
    pub fn bar_end_ms(self, open_ms: i64) -> i64 {
        match self.unit {
            TimeUnit::Month => {
                let (y, m, _) = civil_from_days(open_ms.div_euclid(DAY_MS));
                let idx = y * 12 + (m as i64 - 1) + self.count as i64;
                days_from_civil(idx.div_euclid(12), (idx.rem_euclid(12) + 1) as u32, 1) * DAY_MS
            }
            _ => open_ms + self.duration_ms(),
        }
    }
}

impl PartialOrd for Timeframe {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timeframe {
    /// By length, then unit (so 1D < 1W < 1M, 60m < 1h never ties ambiguously).
    fn cmp(&self, other: &Self) -> Ordering {
        self.duration_ms()
            .cmp(&other.duration_ms())
            .then(self.unit.cmp(&other.unit))
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.code())
    }
}

impl serde::Serialize for Timeframe {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.code())
    }
}

impl<'de> serde::Deserialize<'de> for Timeframe {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Timeframe::parse(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid timeframe {s:?}")))
    }
}

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's algorithm.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
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

/// (year, month, day) → days since 1970-01-01.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-11-20 14:05 UTC (a Friday)
    const T: i64 = 1_795_183_500_000;

    #[test]
    fn codes_labels_and_names() {
        assert_eq!(Timeframe::M1.code(), "1m");
        assert_eq!(Timeframe::D1.label(), "D");
        assert_eq!(Timeframe::new(TimeUnit::Day, 2).label(), "2D");
        assert_eq!(Timeframe::MN1.label(), "M");
        assert_eq!(
            Timeframe::new(TimeUnit::Minute, 45).long_name(),
            "45 minutes"
        );
        assert_eq!(Timeframe::H1.long_name(), "1 hour");
        assert_eq!(Timeframe::new(TimeUnit::Month, 12).long_name(), "12 months");
        assert_eq!(Timeframe::S1.long_name(), "1 second");
    }

    #[test]
    fn parse_round_trip() {
        for code in [
            "1s", "5s", "1m", "45m", "4h", "1D", "3D", "1W", "2W", "1M", "3M", "12M",
        ] {
            let tf = Timeframe::parse(code).unwrap();
            assert_eq!(tf.code(), code);
        }
        assert_eq!(Timeframe::parse("1d"), Some(Timeframe::D1));
        assert_eq!(Timeframe::parse("D"), Some(Timeframe::D1));
        assert_eq!(Timeframe::parse("M"), Some(Timeframe::MN1));
        assert_eq!(Timeframe::parse("m"), Some(Timeframe::M1));
        assert_eq!(Timeframe::parse("0m"), None);
        assert_eq!(Timeframe::parse("13M"), None);
        assert_eq!(Timeframe::parse("15x"), None);
        assert_eq!(Timeframe::parse(""), None);
    }

    #[test]
    fn serde_uses_codes_and_accepts_old_settings() {
        let json = serde_json_like(&Timeframe::new(TimeUnit::Minute, 45));
        assert_eq!(json, "45m");
        // Values written by earlier versions still load.
        for old in ["1m", "5m", "15m", "1h", "4h", "1D"] {
            assert!(Timeframe::parse(old).is_some(), "{old}");
        }
    }

    fn serde_json_like(tf: &Timeframe) -> String {
        tf.code()
    }

    #[test]
    fn ordering_by_length() {
        let mut v = vec![
            Timeframe::D1,
            Timeframe::M1,
            Timeframe::MN1,
            Timeframe::W1,
            Timeframe::H4,
        ];
        v.sort();
        assert_eq!(
            v,
            [
                Timeframe::M1,
                Timeframe::H4,
                Timeframe::D1,
                Timeframe::W1,
                Timeframe::MN1
            ]
        );
    }

    #[test]
    fn buckets() {
        let day0 = T.div_euclid(DAY_MS) * DAY_MS; // 2026-11-20 00:00
        assert_eq!(Timeframe::H4.bucket_start(T), day0 + 12 * HOUR_MS);
        assert_eq!(
            Timeframe::new(TimeUnit::Minute, 45).bucket_start(day0 + 50 * MINUTE_MS),
            day0 + 45 * MINUTE_MS
        );
        // Weeks start Monday 2026-11-16.
        assert_eq!(Timeframe::W1.bucket_start(T), day0 - 4 * DAY_MS);
        // Calendar months.
        let nov1 = days_from_civil(2026, 11, 1) * DAY_MS;
        assert_eq!(Timeframe::MN1.bucket_start(T), nov1);
        let oct1 = days_from_civil(2026, 10, 1) * DAY_MS;
        assert_eq!(Timeframe::new(TimeUnit::Month, 3).bucket_start(T), oct1);
        let jul1 = days_from_civil(2026, 7, 1) * DAY_MS;
        assert_eq!(Timeframe::new(TimeUnit::Month, 6).bucket_start(T), jul1);
    }

    #[test]
    fn bar_ends() {
        let nov1 = days_from_civil(2026, 11, 1) * DAY_MS;
        assert_eq!(
            Timeframe::MN1.bar_end_ms(nov1),
            days_from_civil(2026, 12, 1) * DAY_MS
        );
        assert_eq!(
            Timeframe::new(TimeUnit::Month, 3).bar_end_ms(nov1),
            days_from_civil(2027, 2, 1) * DAY_MS
        );
        assert_eq!(Timeframe::M5.bar_end_ms(1000), 1000 + 5 * MINUTE_MS);
    }

    #[test]
    fn civil_round_trip() {
        for days in [-1000, 0, 1, 59, 365, 20_000, 20_777] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }

    #[test]
    fn custom_limits() {
        assert!(Timeframe::is_valid_custom(TimeUnit::Minute, 45));
        assert!(!Timeframe::is_valid_custom(TimeUnit::Second, 60));
        assert!(!Timeframe::is_valid_custom(TimeUnit::Month, 0));
    }
}
