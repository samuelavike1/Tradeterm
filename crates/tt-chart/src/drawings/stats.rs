//! Stats text shared by line tools and measurers.

/// Signed duration, e.g. `2d 3h`, `45m`, `-1h 30m`.
pub(super) fn format_span(ms: i64) -> String {
    let sign = if ms < 0 { "-" } else { "" };
    let m = ms.unsigned_abs() / 60_000;
    let (d, h, mm) = (m / 1440, m / 60 % 24, m % 60);
    let s = match (d, h, mm) {
        (0, 0, mm) => format!("{mm}m"),
        (0, h, 0) => format!("{h}h"),
        (0, h, mm) => format!("{h}h {mm}m"),
        (d, 0, _) => format!("{d}d"),
        (d, h, _) => format!("{d}d {h}h"),
    };
    format!("{sign}{s}")
}
