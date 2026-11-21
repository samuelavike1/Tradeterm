//! Time zones for time labels, without a tz database.
//!
//! The app builds a [`TzOffsets`] table for the chosen zone (from a real tz
//! database, e.g. `jiff` in `tt-session`) and hands it to the view. Labels
//! convert bar times with [`TzOffsets::local_ms`]: one binary search over a
//! few hundred transitions, and none at all for fixed-offset zones (UTC).
//! Bars keep the exchange's alignment; only the labels move.

use std::sync::Arc;

/// UTC-offset table for one zone: sorted `(utc_ms_from, offset_ms)` pairs.
/// The first entry applies to all earlier times too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TzOffsets {
    transitions: Arc<[(i64, i32)]>,
}

impl Default for TzOffsets {
    fn default() -> Self {
        Self::utc()
    }
}

impl TzOffsets {
    /// UTC: a single zero offset.
    pub fn utc() -> Self {
        Self::fixed(0)
    }

    /// A shared UTC table (for scenes without a zone, e.g. tests).
    pub fn utc_static() -> &'static Self {
        static UTC: std::sync::LazyLock<TzOffsets> = std::sync::LazyLock::new(TzOffsets::utc);
        &UTC
    }

    /// A zone that never changes its offset.
    pub fn fixed(offset_ms: i32) -> Self {
        Self {
            transitions: Arc::from([(i64::MIN, offset_ms)]),
        }
    }

    /// From transitions sorted by time (unsorted input is sorted; an empty
    /// list means UTC).
    pub fn from_transitions(mut transitions: Vec<(i64, i32)>) -> Self {
        if transitions.is_empty() {
            return Self::utc();
        }
        transitions.sort_by_key(|t| t.0);
        transitions.dedup_by(|b, a| a.1 == b.1);
        Self {
            transitions: transitions.into(),
        }
    }

    /// The offset in effect at `utc_ms`.
    #[inline]
    pub fn offset_ms(&self, utc_ms: i64) -> i64 {
        let t = &self.transitions;
        if t.len() == 1 {
            return t[0].1 as i64;
        }
        let k = t.partition_point(|(from, _)| *from <= utc_ms);
        t[k.saturating_sub(1)].1 as i64
    }

    /// Wall-clock time in this zone, as ms since the local epoch (feed it to
    /// [`crate::scale::DateTime::from_unix_ms`]).
    #[inline]
    pub fn local_ms(&self, utc_ms: i64) -> i64 {
        utc_ms + self.offset_ms(utc_ms)
    }

    /// `true` when the offset is the same at all times (no binary search).
    pub fn is_fixed(&self) -> bool {
        self.transitions.len() == 1
    }

    /// Number of entries in the table.
    pub fn len(&self) -> usize {
        self.transitions.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }
}

/// `UTC`, `UTC+1`, `UTC-3:30`, `UTC+5:45`: TradingView's offset notation.
pub fn format_offset(offset_ms: i64) -> String {
    let mins = offset_ms / 60_000;
    if mins == 0 {
        return "UTC".to_owned();
    }
    let sign = if mins < 0 { '-' } else { '+' };
    let (h, m) = (mins.abs() / 60, mins.abs() % 60);
    if m == 0 {
        format!("UTC{sign}{h}")
    } else {
        format!("UTC{sign}{h}:{m:02}")
    }
}

/// TradingView's time zone list (`ui_elements/timezones`, 98 zones) as
/// `(IANA id, display name)`, in TradingView's order. `Etc/UTC` is offered
/// as "UTC" separately, so it is not repeated here.
pub const TV_ZONES: &[(&str, &str)] = &[
    ("Africa/Cairo", "Cairo"),
    ("Africa/Casablanca", "Casablanca"),
    ("Africa/Johannesburg", "Johannesburg"),
    ("Africa/Lagos", "Lagos"),
    ("Africa/Nairobi", "Nairobi"),
    ("Africa/Tunis", "Tunis"),
    ("America/Anchorage", "Anchorage"),
    ("America/Argentina/Buenos_Aires", "Buenos Aires"),
    ("America/Bogota", "Bogota"),
    ("America/Caracas", "Caracas"),
    ("America/Chicago", "Chicago"),
    ("America/El_Salvador", "San Salvador"),
    ("America/Halifax", "Halifax"),
    ("America/Juneau", "Juneau"),
    ("America/Lima", "Lima"),
    ("America/Los_Angeles", "Los Angeles"),
    ("America/Mexico_City", "Mexico City"),
    ("America/New_York", "New York"),
    ("America/Phoenix", "Phoenix"),
    ("America/Santiago", "Santiago"),
    ("America/Sao_Paulo", "Sao Paulo"),
    ("America/Toronto", "Toronto"),
    ("America/Vancouver", "Vancouver"),
    ("Asia/Astana", "Astana"),
    ("Asia/Ashkhabad", "Ashgabat"),
    ("Asia/Bahrain", "Bahrain"),
    ("Asia/Bangkok", "Bangkok"),
    ("Asia/Chongqing", "Chongqing"),
    ("Asia/Colombo", "Colombo"),
    ("Asia/Dhaka", "Dhaka"),
    ("Asia/Dubai", "Dubai"),
    ("Asia/Ho_Chi_Minh", "Ho Chi Minh"),
    ("Asia/Hong_Kong", "Hong Kong"),
    ("Asia/Jakarta", "Jakarta"),
    ("Asia/Jerusalem", "Jerusalem"),
    ("Asia/Karachi", "Karachi"),
    ("Asia/Kabul", "Kabul"),
    ("Asia/Kathmandu", "Kathmandu"),
    ("Asia/Kolkata", "Kolkata"),
    ("Asia/Kuala_Lumpur", "Kuala Lumpur"),
    ("Asia/Kuwait", "Kuwait"),
    ("Asia/Manila", "Manila"),
    ("Asia/Muscat", "Muscat"),
    ("Asia/Nicosia", "Nicosia"),
    ("Asia/Qatar", "Qatar"),
    ("Asia/Riyadh", "Riyadh"),
    ("Asia/Seoul", "Seoul"),
    ("Asia/Shanghai", "Shanghai"),
    ("Asia/Singapore", "Singapore"),
    ("Asia/Taipei", "Taipei"),
    ("Asia/Tehran", "Tehran"),
    ("Asia/Tokyo", "Tokyo"),
    ("Asia/Yangon", "Yangon"),
    ("Atlantic/Azores", "Azores"),
    ("Atlantic/Reykjavik", "Reykjavik"),
    ("Australia/Adelaide", "Adelaide"),
    ("Australia/Brisbane", "Brisbane"),
    ("Australia/Perth", "Perth"),
    ("Australia/Sydney", "Sydney"),
    ("Europe/Amsterdam", "Amsterdam"),
    ("Europe/Athens", "Athens"),
    ("Europe/Belgrade", "Belgrade"),
    ("Europe/Berlin", "Berlin"),
    ("Europe/Bratislava", "Bratislava"),
    ("Europe/Brussels", "Brussels"),
    ("Europe/Bucharest", "Bucharest"),
    ("Europe/Budapest", "Budapest"),
    ("Europe/Copenhagen", "Copenhagen"),
    ("Europe/Dublin", "Dublin"),
    ("Europe/Helsinki", "Helsinki"),
    ("Europe/Istanbul", "Istanbul"),
    ("Europe/Lisbon", "Lisbon"),
    ("Europe/Ljubljana", "Ljubljana"),
    ("Europe/London", "London"),
    ("Europe/Luxembourg", "Luxembourg"),
    ("Europe/Madrid", "Madrid"),
    ("Europe/Malta", "Malta"),
    ("Europe/Moscow", "Moscow"),
    ("Europe/Oslo", "Oslo"),
    ("Europe/Paris", "Paris"),
    ("Europe/Prague", "Prague"),
    ("Europe/Riga", "Riga"),
    ("Europe/Rome", "Rome"),
    ("Europe/Sofia", "Sofia"),
    ("Europe/Stockholm", "Stockholm"),
    ("Europe/Tallinn", "Tallinn"),
    ("Europe/Vienna", "Vienna"),
    ("Europe/Vilnius", "Vilnius"),
    ("Europe/Warsaw", "Warsaw"),
    ("Europe/Zagreb", "Zagreb"),
    ("Europe/Zurich", "Zurich"),
    ("Pacific/Auckland", "Auckland"),
    ("Pacific/Chatham", "Chatham Islands"),
    ("Pacific/Fakaofo", "Tokelau"),
    ("Pacific/Honolulu", "Honolulu"),
    ("Pacific/Norfolk", "Norfolk Island"),
    ("US/Mountain", "Denver"),
];

/// Display name of a zone id from [`TV_ZONES`] (the id itself otherwise).
pub fn zone_name(id: &str) -> &str {
    TV_ZONES
        .iter()
        .find(|(z, _)| *z == id)
        .map_or(id, |(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i32 = 3_600_000;

    #[test]
    fn utc_is_zero_without_search() {
        let tz = TzOffsets::utc();
        assert!(tz.is_fixed());
        assert_eq!(tz.offset_ms(0), 0);
        assert_eq!(tz.local_ms(1_795_132_800_000), 1_795_132_800_000);
    }

    #[test]
    fn lookups_pick_the_transition_in_effect() {
        // -5h, then -4h from t=1000, then -5h from t=2000.
        let tz =
            TzOffsets::from_transitions(vec![(2000, -5 * H), (i64::MIN, -5 * H), (1000, -4 * H)]);
        assert_eq!(tz.offset_ms(-1), -5 * H as i64);
        assert_eq!(tz.offset_ms(999), -5 * H as i64);
        assert_eq!(tz.offset_ms(1000), -4 * H as i64);
        assert_eq!(tz.offset_ms(1999), -4 * H as i64);
        assert_eq!(tz.offset_ms(2000), -5 * H as i64);
        assert_eq!(tz.offset_ms(i64::MAX / 2), -5 * H as i64);
        // Equal neighbours collapse to one entry.
        let same = TzOffsets::from_transitions(vec![(0, H), (10, H)]);
        assert!(same.is_fixed());
    }

    #[test]
    fn offsets_format_like_tradingview() {
        assert_eq!(format_offset(0), "UTC");
        assert_eq!(format_offset(H as i64), "UTC+1");
        assert_eq!(format_offset(-5 * H as i64), "UTC-5");
        assert_eq!(format_offset(19_800_000), "UTC+5:30");
        assert_eq!(format_offset(20_700_000), "UTC+5:45");
        assert_eq!(format_offset(-12_600_000), "UTC-3:30");
    }

    #[test]
    fn zone_list_is_tradingviews() {
        // 98 with Etc/UTC, which is listed separately.
        assert_eq!(TV_ZONES.len(), 97);
        assert_eq!(zone_name("America/New_York"), "New York");
        assert_eq!(zone_name("Mars/Olympus"), "Mars/Olympus");
    }
}
