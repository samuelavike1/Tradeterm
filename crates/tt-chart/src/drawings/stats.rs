//! Stats text shared by line tools and measurers, written into fixed stack
//! buffers so drawing a frame allocates nothing.

use super::model::Anchor;
use super::props::Stats;
use crate::render::Canvas;
use crate::scale::DateTime;
use ecolor::Color32;
use emath::{Align2, Pos2, Rect, pos2, vec2};
use std::fmt::{self, Write};

const CAP: usize = 96;

/// A short string on the stack. Writes that don't fit fail whole (the text
/// so far stays valid).
#[derive(Clone, Copy)]
pub(crate) struct Buf {
    b: [u8; CAP],
    n: usize,
}

impl Buf {
    pub const fn new() -> Self {
        Self { b: [0; CAP], n: 0 }
    }

    pub fn as_str(&self) -> &str {
        // Only whole `&str`s are appended, so this never fails.
        std::str::from_utf8(&self.b[..self.n]).unwrap_or("")
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn clear(&mut self) {
        self.n = 0;
    }
}

impl Default for Buf {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Write for Buf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.n + s.len();
        if end > CAP {
            return Err(fmt::Error);
        }
        self.b[self.n..end].copy_from_slice(s.as_bytes());
        self.n = end;
        Ok(())
    }
}

impl fmt::Debug for Buf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

/// Up to [`Lines::MAX`] lines of text.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Lines {
    l: [Buf; Lines::MAX],
    n: usize,
}

impl Lines {
    pub const MAX: usize = 5;

    /// The next line (cleared), or `None` when full.
    pub fn push(&mut self) -> Option<&mut Buf> {
        let b = self.l.get_mut(self.n)?;
        b.clear();
        self.n += 1;
        Some(b)
    }

    /// Drops a trailing empty line.
    fn pop_empty(&mut self) {
        if self.n > 0 && self.l[self.n - 1].is_empty() {
            self.n -= 1;
        }
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.l[..self.n].iter().map(Buf::as_str)
    }
}

/// [`crate::scale::format_price`] without allocating: thousands
/// separators, no sign for values that round to zero.
pub(crate) fn write_price(w: &mut impl Write, price: f64, decimals: usize) -> fmt::Result {
    let mut digits = Buf::new();
    write!(digits, "{:.*}", decimals.min(12), price.abs())?;
    let digits = digits.as_str();
    let (int, frac) = match digits.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (digits, None),
    };
    if price < 0.0 && digits.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
        w.write_char('-')?;
    }
    for (i, ch) in int.char_indices() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            w.write_char(',')?;
        }
        w.write_char(ch)?;
    }
    if let Some(f) = frac {
        w.write_char('.')?;
        w.write_str(f)?;
    }
    Ok(())
}

/// Like [`write_price`], always signed (`+10.20`, `-1,226.90`).
pub(crate) fn write_signed_price(w: &mut impl Write, v: f64, decimals: usize) -> fmt::Result {
    let mut b = Buf::new();
    write_price(&mut b, v, decimals)?;
    if !b.as_str().starts_with('-') {
        w.write_char('+')?;
    }
    w.write_str(b.as_str())
}

/// Compact volume, e.g. `1.23K`, `4.56M`.
pub(crate) fn write_volume(w: &mut impl Write, v: f64) -> fmt::Result {
    let a = v.abs();
    if a >= 1e9 {
        write!(w, "{:.2}B", v / 1e9)
    } else if a >= 1e6 {
        write!(w, "{:.2}M", v / 1e6)
    } else if a >= 1e3 {
        write!(w, "{:.2}K", v / 1e3)
    } else {
        write!(w, "{v:.2}")
    }
}

/// Signed duration with its two largest units, e.g. `3w 2d`, `2d 3h`,
/// `1h 30m`, `45m 10s`, `-30s`.
pub(crate) fn write_span(w: &mut impl Write, ms: i64) -> fmt::Result {
    if ms < 0 {
        w.write_char('-')?;
    }
    let secs = ms.unsigned_abs() / 1000;
    let (wk, d, h, m, s) = (
        secs / 604_800,
        secs / 86_400 % 7,
        secs / 3600 % 24,
        secs / 60 % 60,
        secs % 60,
    );
    let (big, small) = if wk > 0 {
        ((wk, 'w'), (d, 'd'))
    } else if d > 0 {
        ((d, 'd'), (h, 'h'))
    } else if h > 0 {
        ((h, 'h'), (m, 'm'))
    } else if m > 0 {
        ((m, 'm'), (s, 's'))
    } else {
        return write!(w, "{s}s");
    };
    write!(w, "{}{}", big.0, big.1)?;
    if small.0 > 0 {
        write!(w, " {}{}", small.0, small.1)?;
    }
    Ok(())
}

/// [`write_span`] into a `String` (tests and non-frame code).
#[cfg(test)]
pub(super) fn format_span(ms: i64) -> String {
    let mut s = String::new();
    let _ = write_span(&mut s, ms);
    s
}

/// Date and time of a bar, the crosshair's format: `Fri 20 Nov '26  03:35`
/// (no time for daily and longer bars).
pub(crate) fn write_full_time(w: &mut impl Write, ms: i64, bar_interval_ms: i64) -> fmt::Result {
    let dt = DateTime::from_unix_ms(ms);
    write!(
        w,
        "{} {} {} '{:02}",
        DateTime::weekday_name(ms),
        dt.day,
        dt.month_name(),
        dt.year.rem_euclid(100)
    )?;
    if bar_interval_ms < crate::scale::DAY_MS {
        write!(w, "  {:02}:{:02}", dt.hour, dt.minute)?;
    }
    Ok(())
}

/// What the stats of a two-point drawing are computed from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StatInputs {
    pub from: Anchor,
    pub to: Anchor,
    /// Bars between the points (signed).
    pub bars: i64,
    /// Screen angle in degrees (counter-clockwise, 0 = rightwards).
    pub angle_deg: f32,
    pub precision: usize,
    /// Price step of one pip / tick.
    pub tick: f64,
    /// Volume inside the range (range tools only).
    pub volume: Option<f64>,
}

/// Which axes a tool measures (range tools only show their axes' stats).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Axes {
    pub price: bool,
    pub time: bool,
}

impl Axes {
    pub const BOTH: Axes = Axes {
        price: true,
        time: true,
    };
}

/// The stats block (spec 3.1): `Δprice (Δ%) Δpips`, `N bars, span`,
/// `Vol …`, `angle°`. Each line is present when one of its stats is on.
/// "Distance" is not shown until its unit is verified against TradingView.
pub(crate) fn stats_lines(st: &Stats, axes: Axes, inp: &StatInputs, out: &mut Lines) {
    let diff = inp.to.price - inp.from.price;
    if axes.price
        && (st.price_range || st.percent || st.pips)
        && let Some(b) = out.push()
    {
        let mut sep = "";
        if st.price_range {
            let _ = write_signed_price(b, diff, inp.precision);
            sep = " ";
        }
        if st.percent {
            let pct = if inp.from.price != 0.0 {
                diff / inp.from.price * 100.0
            } else {
                0.0
            };
            let _ = if st.price_range {
                write!(b, "{sep}({pct:+.2}%)")
            } else {
                write!(b, "{pct:+.2}%")
            };
            sep = " ";
        }
        if st.pips && inp.tick > 0.0 {
            let _ = b.write_str(sep);
            let _ = write_signed_price(b, (diff / inp.tick).round(), 0);
        }
    }
    if axes.time
        && (st.bars || st.date_time)
        && let Some(b) = out.push()
    {
        if st.bars {
            let _ = write!(b, "{} bars", inp.bars);
        }
        if st.date_time {
            if st.bars {
                let _ = b.write_str(", ");
            }
            let _ = write_span(b, inp.to.time_ms - inp.from.time_ms);
        }
    }
    if axes.time
        && st.volume
        && let Some(v) = inp.volume
        && let Some(b) = out.push()
    {
        let _ = b.write_str("Vol ");
        let _ = write_volume(b, v);
    }
    if st.angle
        && axes == Axes::BOTH
        && let Some(b) = out.push()
    {
        let _ = write!(b, "{:.2}°", inp.angle_deg);
    }
    out.pop_empty();
}

/// Screen angle of `a → b` in degrees, counter-clockwise (up is positive).
pub(crate) fn angle_deg(a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    if d.length_sq() <= f32::EPSILON {
        return 0.0;
    }
    // `0.0 - y`, not `-y`: a level line pointing left is 180°, not -180°.
    (0.0 - d.y).atan2(d.x).to_degrees()
}

/// Price step of one pip: the symbol's tick, else one unit of the last
/// displayed decimal.
pub(crate) fn tick_or_precision(tick: Option<f64>, precision: usize) -> f64 {
    tick.filter(|t| t.is_finite() && *t > 0.0)
        .unwrap_or_else(|| 10f64.powi(-(precision.min(12) as i32)))
}

/// A box with `lines` centered in it, anchored at `at` by `align`.
/// Returns the box.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_box(
    c: &mut dyn Canvas,
    lines: &Lines,
    at: Pos2,
    align: Align2,
    size: f32,
    bold: bool,
    text: Color32,
    bg: Option<Color32>,
) -> Rect {
    let line_h = size + 4.0;
    let mut width: f32 = 0.0;
    for l in lines.iter() {
        width = width.max(c.text_size_styled(l, size, bold).x);
    }
    let size_v = vec2(width + 16.0, line_h * lines.len() as f32 + 8.0);
    let r = align.anchor_size(at, size_v);
    if let Some(bg) = bg {
        c.fill_rounded_rect(r, tt_theme::metrics::R_SM, bg);
    }
    for (i, l) in lines.iter().enumerate() {
        c.text_styled(
            pos2(r.center().x, r.top() + 4.0 + line_h * (i as f32 + 0.5)),
            Align2::CENTER_CENTER,
            l,
            size,
            text,
            bold,
        );
    }
    r
}
