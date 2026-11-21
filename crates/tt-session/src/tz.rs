//! Time zones for chart labels: resolves the Symbol › Time zone setting with
//! `jiff`'s tz database and builds the [`TzOffsets`] table the chart uses
//! (the chart crate itself has no tz database).

use jiff::Timestamp;
use jiff::tz::TimeZone;
use std::sync::{Arc, Mutex};
use tt_chart::TzOffsets;
use tt_chart::style::TimeZoneSetting;
use tt_chart::tz::{TV_ZONES, format_offset, zone_name};

/// Range the offset tables cover. Outside it the nearest entry applies.
/// ~2 transitions a year for DST zones: a few hundred entries.
const TABLE_FROM_MS: i64 = 0; // 1970-01-01
const TABLE_TO_MS: i64 = 4_102_444_800_000; // 2100-01-01

/// The zone a setting means. `exchange` is the symbol's exchange zone
/// (`SymbolInfo::timezone`; UTC for Binance and while unknown). Unknown
/// names fall back to UTC.
pub fn resolve(setting: &TimeZoneSetting, exchange: Option<&str>) -> TimeZone {
    let named = |name: &str| get(name).unwrap_or(TimeZone::UTC);
    match setting {
        TimeZoneSetting::Exchange => exchange.map_or(TimeZone::UTC, named),
        TimeZoneSetting::Utc => TimeZone::UTC,
        TimeZoneSetting::Local => TimeZone::try_system().unwrap_or(TimeZone::UTC),
        TimeZoneSetting::Iana(name) => named(name),
    }
}

/// TradingView ids the tz database knows under another name.
const ALIASES: &[(&str, &str)] = &[("Asia/Astana", "Asia/Almaty")];

/// A zone by IANA name (or a TradingView alias of one).
fn get(name: &str) -> Option<TimeZone> {
    let name = ALIASES
        .iter()
        .find(|(a, _)| *a == name)
        .map_or(name, |(_, real)| real);
    TimeZone::get(name).ok()
}

/// Offset table of `tz` from 1970 to 2100.
pub fn offsets(tz: &TimeZone) -> TzOffsets {
    let from = Timestamp::from_millisecond(TABLE_FROM_MS).unwrap_or(Timestamp::UNIX_EPOCH);
    let first = tz.to_offset(from).seconds() * 1000;
    let mut table = vec![(i64::MIN, first)];
    table.extend(
        tz.following(from)
            .map(|t| (t.timestamp().as_millisecond(), t.offset().seconds() * 1000))
            .take_while(|(ms, _)| *ms < TABLE_TO_MS),
    );
    TzOffsets::from_transitions(table)
}

/// Offset table for a setting (see [`resolve`]).
pub fn offsets_for(setting: &TimeZoneSetting, exchange: Option<&str>) -> TzOffsets {
    offsets(&resolve(setting, exchange))
}

/// The current UTC offset of a setting, in ms.
pub fn offset_now_ms(setting: &TimeZoneSetting, exchange: Option<&str>, now_ms: i64) -> i64 {
    let ts = Timestamp::from_millisecond(now_ms).unwrap_or(Timestamp::UNIX_EPOCH);
    resolve(setting, exchange).to_offset(ts).seconds() as i64 * 1000
}

/// Menu / select label of a setting, TradingView style with the current
/// offset: `(UTC-5) New York`, `UTC`, `Exchange (UTC)`, `Local (UTC+1)`.
pub fn label(setting: &TimeZoneSetting, exchange: Option<&str>, now_ms: i64) -> String {
    let off = format_offset(offset_now_ms(setting, exchange, now_ms));
    match setting {
        TimeZoneSetting::Exchange => format!("Exchange ({off})"),
        TimeZoneSetting::Utc => "UTC".to_owned(),
        TimeZoneSetting::Local => format!("Local ({off})"),
        TimeZoneSetting::Iana(name) => format!("({off}) {}", zone_name(name)),
    }
}

/// Every choice in TradingView's order: Exchange, UTC, Local, then the
/// TradingView zone list, each with its label (see [`label`]).
pub fn choices(exchange: Option<&str>, now_ms: i64) -> Vec<(TimeZoneSetting, String)> {
    let mut out: Vec<(TimeZoneSetting, String)> = [
        TimeZoneSetting::Exchange,
        TimeZoneSetting::Utc,
        TimeZoneSetting::Local,
    ]
    .into_iter()
    .map(|z| {
        let l = label(&z, exchange, now_ms);
        (z, l)
    })
    .collect();
    out.extend(zone_choices(now_ms).iter().cloned());
    out
}

/// Labelled zone choices, shared.
pub type Choices = Arc<[(TimeZoneSetting, String)]>;

/// The TradingView zones with labels, cached for the hour (offsets only
/// change at DST transitions, which fall on whole hours).
pub fn zone_choices(now_ms: i64) -> Choices {
    static CACHE: Mutex<Option<(i64, Choices)>> = Mutex::new(None);
    let hour = now_ms.div_euclid(3_600_000);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((h, list)) = cache.as_ref()
        && *h == hour
    {
        return list.clone();
    }
    let list: Choices = TV_ZONES
        .iter()
        .map(|(id, _)| {
            let z = TimeZoneSetting::Iana((*id).to_owned());
            let l = label(&z, None, now_ms);
            (z, l)
        })
        .collect();
    *cache = Some((hour, list.clone()));
    list
}

/// The status-bar clock: `14:05:33 UTC+1` for `now_ms` in `tz`.
pub fn clock_text(tz: &TzOffsets, now_ms: i64) -> String {
    let off = tz.offset_ms(now_ms);
    let secs = (now_ms + off).div_euclid(1000).rem_euclid(86_400);
    format!(
        "{:02}:{:02}:{:02} {}",
        secs / 3600,
        secs / 60 % 60,
        secs % 60,
        format_offset(off)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tt_chart::scale::DateTime;

    const H: i64 = 3_600_000;

    fn iana(name: &str) -> TzOffsets {
        offsets_for(&TimeZoneSetting::Iana(name.to_owned()), None)
    }

    /// Unix ms of a UTC date-time.
    fn utc(y: i16, m: i8, d: i8, h: i8, min: i8) -> i64 {
        TimeZone::UTC
            .to_timestamp(jiff::civil::datetime(y, m, d, h, min, 0, 0))
            .unwrap()
            .as_millisecond()
    }

    #[test]
    fn new_york_across_dst() {
        let ny = iana("America/New_York");
        assert!(!ny.is_fixed());
        // 2026: DST from Sun 8 Mar 07:00 UTC to Sun 1 Nov 06:00 UTC.
        assert_eq!(ny.offset_ms(utc(2026, 3, 8, 6, 59)), -5 * H);
        assert_eq!(ny.offset_ms(utc(2026, 3, 8, 7, 0)), -4 * H);
        assert_eq!(ny.offset_ms(utc(2026, 7, 1, 12, 0)), -4 * H);
        assert_eq!(ny.offset_ms(utc(2026, 11, 1, 5, 59)), -4 * H);
        assert_eq!(ny.offset_ms(utc(2026, 11, 1, 6, 0)), -5 * H);
        // Far future still follows the rule.
        assert_eq!(ny.offset_ms(utc(2090, 7, 1, 0, 0)), -4 * H);
        // Wall clock: 2026-07-01 12:00 UTC is 08:00 in New York.
        let dt = DateTime::from_unix_ms(ny.local_ms(utc(2026, 7, 1, 12, 0)));
        assert_eq!((dt.day, dt.hour), (1, 8));
    }

    #[test]
    fn london_across_dst() {
        let ldn = iana("Europe/London");
        // BST 2026: 29 Mar 01:00 UTC to 25 Oct 01:00 UTC.
        assert_eq!(ldn.offset_ms(utc(2026, 3, 29, 0, 59)), 0);
        assert_eq!(ldn.offset_ms(utc(2026, 3, 29, 1, 0)), H);
        assert_eq!(ldn.offset_ms(utc(2026, 10, 25, 0, 59)), H);
        assert_eq!(ldn.offset_ms(utc(2026, 10, 25, 1, 0)), 0);
    }

    #[test]
    fn exchange_is_utc_for_binance() {
        let ex = offsets_for(&TimeZoneSetting::Exchange, Some("UTC"));
        assert!(ex.is_fixed());
        assert_eq!(ex.offset_ms(utc(2026, 7, 1, 0, 0)), 0);
        // Unknown exchange zone or names: UTC.
        assert!(offsets_for(&TimeZoneSetting::Exchange, None).is_fixed());
        assert!(iana("Mars/Olympus").is_fixed());
        assert_eq!(
            label(&TimeZoneSetting::Exchange, Some("UTC"), 0),
            "Exchange (UTC)"
        );
    }

    #[test]
    fn every_tradingview_zone_resolves() {
        let missing: Vec<&str> = TV_ZONES
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| get(id).is_none())
            .collect();
        assert!(missing.is_empty(), "{missing:?}");
        assert_eq!(choices(None, 0).len(), TV_ZONES.len() + 3);
    }

    #[test]
    fn labels_and_clock_show_offsets() {
        let jan = utc(2026, 1, 15, 12, 0);
        let jul = utc(2026, 7, 15, 12, 0);
        let ny = TimeZoneSetting::Iana("America/New_York".into());
        assert_eq!(label(&ny, None, jan), "(UTC-5) New York");
        assert_eq!(label(&ny, None, jul), "(UTC-4) New York");
        let kathmandu = TimeZoneSetting::Iana("Asia/Kathmandu".into());
        assert_eq!(label(&kathmandu, None, jan), "(UTC+5:45) Kathmandu");
        let berlin = iana("Europe/Berlin");
        assert_eq!(
            clock_text(&berlin, jan + 5 * 60_000 + 33_000),
            "13:05:33 UTC+1"
        );
        assert_eq!(clock_text(&TzOffsets::utc(), jan), "12:00:00 UTC");
    }
}
