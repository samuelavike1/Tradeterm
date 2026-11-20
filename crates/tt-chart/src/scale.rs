//! Axis tick selection and label formatting. Backend agnostic.

use crate::ScaleMode;
use crate::viewport::Viewport;

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

/// Number of decimals implied by a tick size: `0.01` → 2, `0.5` → 1, `1` → 0.
pub fn precision_from_tick_size(tick_size: f64) -> usize {
    if !(tick_size > 0.0 && tick_size.is_finite()) {
        return 2;
    }
    for d in 0..=12usize {
        let scaled = tick_size * 10f64.powi(d as i32);
        if (scaled - scaled.round()).abs() < 1e-9 * scaled.max(1.0) {
            return d;
        }
    }
    12
}

/// Fallback decimals when no tick size is known, from the price magnitude.
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

pub fn format_percent(pct: f64) -> String {
    format!("{pct:+.2}%")
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

/// Price-axis tick prices for the viewport, at least `min_gap_px` apart.
///
/// - Linear: 1-2-5 steps, never finer than `tick_size`.
/// - Percent: 1-2-5 steps in percent.
/// - Log: 1-2-5 steps when the range is narrow (it is nearly linear there);
///   otherwise "round" prices (1, 2, 5, then 1.5, 3, … × 10ⁿ) picked greedily
///   so they don't crowd at the top.
pub fn price_ticks(vp: &Viewport, min_gap_px: f32, tick_size: Option<f64>) -> Vec<f64> {
    let sc = vp.scale;
    let (lo, hi) = (
        vp.price_min.min(vp.price_max),
        vp.price_min.max(vp.price_max),
    );
    let height = vp.plot.height().max(1.0);
    let target = (height / min_gap_px).max(2.0) as f64;

    match sc.mode {
        ScaleMode::Linear => linear_ticks(lo, hi, target, tick_size),
        ScaleMode::Percent => {
            let (a, b) = (sc.to_scaled(lo), sc.to_scaled(hi));
            ticks(a, b, nice_step(b - a, target))
                .map(|p| sc.from_scaled(p))
                .collect()
        }
        ScaleMode::Logarithmic => {
            if lo <= 0.0 || hi / lo < 3.0 {
                // Narrow range: linear ticks, but drop any that end up too close.
                let mut out = Vec::new();
                let mut last_y = f32::INFINITY;
                for p in linear_ticks(lo, hi, target, tick_size) {
                    let y = vp.y_of(p);
                    if (last_y - y).abs() >= min_gap_px * 0.8 {
                        out.push(p);
                        last_y = y;
                    }
                }
                out
            } else {
                log_ticks(vp, lo, hi, min_gap_px)
            }
        }
    }
}

fn linear_ticks(lo: f64, hi: f64, target: f64, tick_size: Option<f64>) -> Vec<f64> {
    let mut step = nice_step(hi - lo, target);
    if let Some(t) = tick_size.filter(|t| *t > 0.0)
        && step < t
    {
        step = t;
    }
    ticks(lo, hi, step).collect()
}

fn log_ticks(vp: &Viewport, lo: f64, hi: f64, min_gap_px: f32) -> Vec<f64> {
    // (mantissa, priority) — lower priority value wins.
    const MANTISSAS: [(f64, u8); 11] = [
        (1.0, 0),
        (5.0, 1),
        (2.0, 2),
        (2.5, 3),
        (3.0, 3),
        (1.5, 4),
        (4.0, 4),
        (6.0, 4),
        (7.0, 5),
        (8.0, 5),
        (9.0, 5),
    ];
    let k0 = lo.log10().floor() as i32;
    let k1 = hi.log10().ceil() as i32;
    let mut cands: Vec<(u8, f64)> = Vec::new();
    for k in k0..=k1 {
        let mag = 10f64.powi(k);
        for (m, pri) in MANTISSAS {
            let p = m * mag;
            if p >= lo && p <= hi {
                cands.push((pri, p));
            }
        }
    }
    cands.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut placed: Vec<(f64, f32)> = Vec::new();
    for (_, p) in cands {
        let y = vp.y_of(p);
        if placed.iter().all(|(_, py)| (py - y).abs() >= min_gap_px) {
            placed.push((p, y));
        }
    }
    let mut out: Vec<f64> = placed.into_iter().map(|(p, _)| p).collect();
    out.sort_by(f64::total_cmp);
    out
}

pub const MINUTE_MS: i64 = 60_000;
pub const HOUR_MS: i64 = 60 * MINUTE_MS;
pub const DAY_MS: i64 = 24 * HOUR_MS;

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

/// Full crosshair label: `20 Nov 2026  14:05` (time omitted for daily+ bars).
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
    use crate::viewport::{PlotRect, PriceScale};

    fn vp(lo: f64, hi: f64, mode: ScaleMode) -> Viewport {
        Viewport {
            bar_spacing: 5.0,
            right_offset_bars: 0.0,
            price_min: lo,
            price_max: hi,
            plot: PlotRect {
                left: 0.0,
                top: 0.0,
                right: 500.0,
                bottom: 560.0,
            },
            bar_count: 10,
            scale: PriceScale::new(mode, lo),
        }
    }

    fn is_125(step: f64) -> bool {
        let m = step / 10f64.powf(step.log10().floor());
        [1.0, 2.0, 5.0, 10.0].iter().any(|n| (m - n).abs() < 1e-9)
    }

    #[test]
    fn nice_steps_follow_1_2_5() {
        assert_eq!(nice_step(100.0, 10.0), 10.0);
        assert_eq!(nice_step(100.0, 6.0), 20.0);
        assert_eq!(nice_step(100.0, 30.0), 5.0);
        assert!((nice_step(0.01, 5.0) - 0.002).abs() < 1e-12);
        for span in [0.0003, 1.7, 42.0, 9_999.0, 3.3e7] {
            for target in [3.0, 7.0, 12.0] {
                let s = nice_step(span, target);
                assert!(is_125(s), "{span}/{target} -> {s}");
                assert!(span / s <= target * 1.0001, "{span}/{target} -> {s}");
            }
        }
    }

    #[test]
    fn tick_values() {
        let t: Vec<f64> = ticks(3.0, 27.0, 10.0).collect();
        assert_eq!(t, [10.0, 20.0]);
        assert_eq!(ticks(1.0, 2.0, 10.0).count(), 0);
    }

    #[test]
    fn precision_from_tick() {
        assert_eq!(precision_from_tick_size(0.01), 2);
        assert_eq!(precision_from_tick_size(0.00001), 5);
        assert_eq!(precision_from_tick_size(1.0), 0);
        assert_eq!(precision_from_tick_size(0.5), 1);
        assert_eq!(precision_from_tick_size(0.25), 2);
        assert_eq!(precision_from_tick_size(1e-8), 8);
        assert_eq!(precision_from_tick_size(0.0), 2);
    }

    #[test]
    fn fallback_decimals() {
        assert_eq!(price_decimals(61_000.0), 2);
        assert_eq!(price_decimals(0.5), 4);
        assert_eq!(price_decimals(0.000_012_34), 8);
        assert_eq!(format_price(1.5, 2), "1.50");
        assert_eq!(format_percent(1.234), "+1.23%");
    }

    #[test]
    fn linear_ticks_are_nice_and_respect_tick_size() {
        let t = price_ticks(&vp(61_210.0, 62_380.0, ScaleMode::Linear), 56.0, Some(0.01));
        assert!(t.len() >= 3, "{t:?}");
        let step = t[1] - t[0];
        assert!(is_125(step), "{step}");
        assert!(t.iter().all(|p| (61_210.0..=62_380.0).contains(p)));
        // Tiny range: step clamps to the tick size.
        let t = price_ticks(&vp(1.0, 1.02, ScaleMode::Linear), 20.0, Some(0.01));
        assert!(
            t.windows(2).all(|w| (w[1] - w[0] - 0.01).abs() < 1e-9),
            "{t:?}"
        );
    }

    #[test]
    fn percent_ticks_are_nice_percentages() {
        let v = vp(100.0, 130.0, ScaleMode::Percent); // base = 100 → 0%..30%
        let t = price_ticks(&v, 56.0, None);
        let pct: Vec<f64> = t.iter().map(|p| v.scale.to_scaled(*p)).collect();
        assert!(pct.iter().any(|p| p.abs() < 1e-9), "{pct:?}");
        assert!(is_125(pct[1] - pct[0]));
    }

    #[test]
    fn log_ticks_are_round_and_spaced() {
        let v = vp(10.0, 50_000.0, ScaleMode::Logarithmic);
        let t = price_ticks(&v, 40.0, None);
        assert!(
            t.contains(&100.0) && t.contains(&1000.0) && t.contains(&10_000.0),
            "{t:?}"
        );
        let ys: Vec<f32> = t.iter().map(|p| v.y_of(*p)).collect();
        assert!(ys.windows(2).all(|w| (w[0] - w[1]).abs() >= 40.0), "{ys:?}");
        // Narrow log range still yields regular ticks.
        let t = price_ticks(&vp(60_000.0, 62_000.0, ScaleMode::Logarithmic), 56.0, None);
        assert!(t.len() >= 3, "{t:?}");
    }

    #[test]
    fn dates() {
        let ms = 1_795_183_500_000; // 2026-11-20 14:05 UTC
        let dt = DateTime::from_unix_ms(ms);
        assert_eq!(
            (dt.year, dt.month, dt.day, dt.hour, dt.minute),
            (2026, 11, 20, 14, 5)
        );
        assert_eq!(format_full_time(ms, MINUTE_MS), "20 Nov 2026  14:05");
        assert_eq!(format_full_time(ms, DAY_MS), "20 Nov 2026");
        assert_eq!(DateTime::from_unix_ms(0).year, 1970);
        assert_eq!(DateTime::from_unix_ms(-1).year, 1969);
    }
}
