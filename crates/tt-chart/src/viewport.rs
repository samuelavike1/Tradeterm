//! Coordinate system: bar index ↔ x and price ↔ y. Pure math, no drawing.
//!
//! X is anchored to the *right* edge: `right_offset_bars` is the empty space
//! (in bars) right of the newest bar. Because positions are measured from the
//! newest bar, prepending older history never moves what is on screen.

use crate::ScaleMode;
use std::ops::Range;

/// Smallest price used by the log scale (log of ≤ 0 is undefined).
pub const MIN_LOG_PRICE: f64 = 1e-12;

/// Plot-area bounds in screen points (y grows downwards).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl PlotRect {
    pub fn width(&self) -> f32 {
        self.right - self.left
    }
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }
}

/// Maps prices into the space in which the y axis is linear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriceScale {
    pub mode: ScaleMode,
    /// Reference price for [`ScaleMode::Percent`] / [`ScaleMode::IndexedTo100`]
    /// (first visible close).
    pub base: f64,
    /// Higher prices lower on screen ("Invert scale").
    pub inverted: bool,
}

impl PriceScale {
    pub fn new(mode: ScaleMode, base: f64) -> Self {
        let base = if base.is_finite() && base > 0.0 {
            base
        } else {
            1.0
        };
        Self {
            mode,
            base,
            inverted: false,
        }
    }

    pub fn linear() -> Self {
        Self::new(ScaleMode::Linear, 1.0)
    }

    /// Price → scaled value (linear in screen space).
    pub fn to_scaled(&self, price: f64) -> f64 {
        match self.mode {
            ScaleMode::Linear => price,
            ScaleMode::Logarithmic => price.max(MIN_LOG_PRICE).ln(),
            ScaleMode::Percent => (price / self.base - 1.0) * 100.0,
            ScaleMode::IndexedTo100 => price / self.base * 100.0,
        }
    }

    /// Scaled value → price.
    pub fn from_scaled(&self, s: f64) -> f64 {
        match self.mode {
            ScaleMode::Linear => s,
            ScaleMode::Logarithmic => s.exp(),
            ScaleMode::Percent => self.base * (1.0 + s / 100.0),
            ScaleMode::IndexedTo100 => self.base * s / 100.0,
        }
    }
}

/// Auto-fit range for prices `lo..hi` with `top` / `bottom` margins as
/// fractions of the pane (TradingView's "Margins").
pub fn fit_price_range_margins(
    lo: f64,
    hi: f64,
    scale: PriceScale,
    top: f64,
    bottom: f64,
) -> (f64, f64) {
    let (s0, s1) = (scale.to_scaled(lo), scale.to_scaled(hi));
    let span = s1 - s0;
    if span.abs() <= f64::EPSILON * s1.abs().max(1.0) {
        return fit_price_range(lo, hi, scale, top.max(bottom));
    }
    let total = span / (1.0 - top - bottom).max(0.1);
    (
        scale.from_scaled(s0 - total * bottom),
        scale.from_scaled(s1 + total * top),
    )
}

/// Everything needed to map data to the screen for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Horizontal distance between bar centers, in points.
    pub bar_spacing: f32,
    /// Empty space right of the newest bar, in bars (negative: newest bar scrolled off).
    pub right_offset_bars: f32,
    pub price_min: f64,
    pub price_max: f64,
    pub plot: PlotRect,
    /// Number of bars in the series.
    pub bar_count: usize,
    pub scale: PriceScale,
}

/// Continuous bar index whose center sits on the plot's right edge.
fn right_edge_index(vp: &Viewport) -> f32 {
    vp.bar_count as f32 - 1.0 + vp.right_offset_bars + 0.5
}

/// X of the center of bar `i` (fractional indices allowed).
pub fn index_to_x(vp: &Viewport, i: f32) -> f32 {
    vp.plot.right - (right_edge_index(vp) - i) * vp.bar_spacing
}

/// Continuous bar index at screen `x`.
pub fn x_to_index(vp: &Viewport, x: f32) -> f32 {
    right_edge_index(vp) - (vp.plot.right - x) / vp.bar_spacing
}

pub fn price_to_y(vp: &Viewport, price: f64) -> f32 {
    let s = vp.scale;
    let lo = s.to_scaled(vp.price_min);
    let hi = s.to_scaled(vp.price_max);
    let span = hi - lo;
    let mut t = if span.abs() > f64::EPSILON {
        (s.to_scaled(price) - lo) / span
    } else {
        0.5
    };
    if s.inverted {
        t = 1.0 - t;
    }
    vp.plot.bottom - (t as f32) * vp.plot.height()
}

pub fn y_to_price(vp: &Viewport, y: f32) -> f64 {
    let s = vp.scale;
    let lo = s.to_scaled(vp.price_min);
    let hi = s.to_scaled(vp.price_max);
    let mut t = ((vp.plot.bottom - y) / vp.plot.height()) as f64;
    if s.inverted {
        t = 1.0 - t;
    }
    s.from_scaled(lo + t * (hi - lo))
}

/// Indices of bars at least partially inside the plot.
pub fn visible_range(vp: &Viewport) -> Range<usize> {
    visible_range_for(
        vp.bar_count,
        vp.bar_spacing,
        vp.right_offset_bars,
        vp.plot.width(),
    )
}

/// Visible bar range from the horizontal state alone, so the price range can
/// be computed from it before the full [`Viewport`] exists.
pub fn visible_range_for(
    bar_count: usize,
    bar_spacing: f32,
    right_offset_bars: f32,
    width: f32,
) -> Range<usize> {
    if bar_count == 0 || bar_spacing <= 0.0 || width <= 0.0 {
        return 0..0;
    }
    let right_idx = bar_count as f32 - 1.0 + right_offset_bars + 0.5;
    let left_idx = right_idx - width / bar_spacing;
    // Include half-visible bars on both edges.
    let lo = (left_idx - 0.5).floor().max(0.0) as usize;
    let hi_f = (right_idx + 0.5).ceil();
    let hi = if hi_f <= 0.0 {
        0
    } else {
        (hi_f as usize).min(bar_count)
    };
    lo.min(hi)..hi
}

impl Viewport {
    pub fn x_of(&self, i: f32) -> f32 {
        index_to_x(self, i)
    }
    pub fn index_at(&self, x: f32) -> f32 {
        x_to_index(self, x)
    }
    pub fn y_of(&self, price: f64) -> f32 {
        price_to_y(self, price)
    }
    pub fn price_at(&self, y: f32) -> f64 {
        y_to_price(self, y)
    }
    pub fn visible_range(&self) -> Range<usize> {
        visible_range(self)
    }

    /// Slot (bar position, possibly without a bar) nearest to screen x.
    pub fn slot_at(&self, x: f32) -> i64 {
        self.index_at(x).round() as i64
    }

    /// Slots at least partially inside the plot, including empty ones before
    /// the first bar and after the last (for the time scale and grid).
    pub fn visible_slots(&self) -> std::ops::Range<i64> {
        let lo = (self.index_at(self.plot.left) - 0.5).floor() as i64;
        let hi = (self.index_at(self.plot.right) + 0.5).ceil() as i64 + 1;
        lo..hi
    }

    /// Nearest existing bar to screen x, if any.
    pub fn bar_at(&self, x: f32) -> Option<usize> {
        let i = self.index_at(x).round();
        (i >= 0.0 && (i as usize) < self.bar_count).then_some(i as usize)
    }
}

/// Pads `[lo, hi]` by `pad_frac` of its span *in scaled space* (so padding
/// looks the same in every scale mode) and returns prices. A flat range is
/// widened so it still gets height.
pub fn fit_price_range(lo: f64, hi: f64, scale: PriceScale, pad_frac: f64) -> (f64, f64) {
    let (slo, shi) = (scale.to_scaled(lo), scale.to_scaled(hi));
    let span = shi - slo;
    if span > 0.0 && span.is_finite() {
        let pad = span * pad_frac;
        (scale.from_scaled(slo - pad), scale.from_scaled(shi + pad))
    } else {
        let mid = if lo.is_finite() { lo } else { 1.0 };
        let half = (mid.abs() * 0.01).max(1e-8);
        let (a, b) = (mid - half, mid + half);
        if scale.mode == ScaleMode::Logarithmic {
            (a.max(MIN_LOG_PRICE), b.max(MIN_LOG_PRICE * 2.0))
        } else {
            (a, b)
        }
    }
}

/// Scales `[min, max]` by `factor` around its center in scaled space.
pub fn zoom_price_range(min: f64, max: f64, scale: PriceScale, factor: f64) -> (f64, f64) {
    let (a, b) = (scale.to_scaled(min), scale.to_scaled(max));
    let mid = (a + b) / 2.0;
    let half = (b - a) / 2.0 * factor;
    (scale.from_scaled(mid - half), scale.from_scaled(mid + half))
}

/// Shifts `[min, max]` by `frac` of its span in scaled space (positive = up).
pub fn pan_price_range(min: f64, max: f64, scale: PriceScale, frac: f64) -> (f64, f64) {
    let (a, b) = (scale.to_scaled(min), scale.to_scaled(max));
    let d = (b - a) * frac;
    (scale.from_scaled(a + d), scale.from_scaled(b + d))
}

#[cfg(test)]
mod tests {

    #[test]
    fn inverted_scale_flips_and_round_trips() {
        let mut vp = Viewport {
            bar_spacing: 10.0,
            right_offset_bars: 0.0,
            price_min: 100.0,
            price_max: 200.0,
            plot: PlotRect {
                left: 0.0,
                top: 0.0,
                right: 100.0,
                bottom: 100.0,
            },
            bar_count: 10,
            scale: PriceScale::linear(),
        };
        assert!(vp.y_of(190.0) < vp.y_of(110.0), "normal: higher is up");
        vp.scale.inverted = true;
        assert!(vp.y_of(190.0) > vp.y_of(110.0), "inverted: higher is down");
        assert!((vp.price_at(vp.y_of(137.5)) - 137.5).abs() < 1e-9);
        // Margins: 10% top / 20% bottom of the pane around 100..200.
        let (lo, hi) = fit_price_range_margins(100.0, 200.0, PriceScale::linear(), 0.1, 0.2);
        assert!((hi - lo - 100.0 / 0.7).abs() < 1e-9);
        assert!(((200.0 - lo) / (hi - lo) - 0.9).abs() < 1e-9);
    }

    use super::*;

    const MODES: [ScaleMode; 4] = ScaleMode::ALL;

    fn vp(n: usize, spacing: f32, off: f32, mode: ScaleMode) -> Viewport {
        Viewport {
            bar_spacing: spacing,
            right_offset_bars: off,
            price_min: 100.0,
            price_max: 200.0,
            plot: PlotRect {
                left: 10.0,
                top: 20.0,
                right: 110.0,
                bottom: 220.0,
            },
            bar_count: n,
            scale: PriceScale::new(mode, 150.0),
        }
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1.0)
    }

    #[test]
    fn scale_transform_round_trips() {
        for mode in MODES {
            let s = PriceScale::new(mode, 123.4);
            for p in [0.001, 1.0, 99.5, 123.4, 61_234.56, 1e7] {
                let back = s.from_scaled(s.to_scaled(p));
                assert!(close(back, p, 1e-12), "{mode:?}: {p} -> {back}");
            }
        }
    }

    #[test]
    fn percent_is_relative_to_base() {
        let s = PriceScale::new(ScaleMode::Percent, 200.0);
        assert_eq!(s.to_scaled(200.0), 0.0);
        assert!(close(s.to_scaled(210.0), 5.0, 1e-12));
        assert!(close(s.to_scaled(150.0), -25.0, 1e-12));
        // Invalid base falls back to 1 instead of producing NaN.
        assert_eq!(PriceScale::new(ScaleMode::Percent, 0.0).base, 1.0);
    }

    #[test]
    fn price_y_round_trip_all_modes() {
        for mode in MODES {
            let v = vp(10, 10.0, 0.0, mode);
            for p in [100.0, 123.0, 150.0, 199.99, 200.0, 250.0, 90.0] {
                let y = price_to_y(&v, p);
                let back = y_to_price(&v, y);
                assert!(close(back, p, 1e-5), "{mode:?}: {p} -> y={y} -> {back}");
            }
            // Range ends map to plot edges in every mode.
            assert!((price_to_y(&v, 100.0) - 220.0).abs() < 1e-3);
            assert!((price_to_y(&v, 200.0) - 20.0).abs() < 1e-3);
            for y in [20.0, 57.5, 120.0, 219.0] {
                let back = price_to_y(&v, y_to_price(&v, y));
                assert!((back - y).abs() < 1e-3, "{mode:?}: y {y} -> {back}");
            }
        }
    }

    #[test]
    fn log_mode_is_geometric() {
        let mut v = vp(1, 10.0, 0.0, ScaleMode::Logarithmic);
        v.price_min = 10.0;
        v.price_max = 1000.0;
        // 100 is the geometric midpoint of 10..1000.
        assert!((price_to_y(&v, 100.0) - 120.0).abs() < 1e-3);
        let lin = Viewport {
            scale: PriceScale::linear(),
            ..v
        };
        assert!(price_to_y(&lin, 100.0) > 200.0);
    }

    #[test]
    fn x_index_round_trip() {
        for off in [-3.0, 0.0, 2.5] {
            let v = vp(50, 10.0, off, ScaleMode::Linear);
            for i in [0.0, 10.0, 25.5, 49.0] {
                assert!((x_to_index(&v, index_to_x(&v, i)) - i).abs() < 1e-3);
            }
            for x in [10.0, 33.3, 110.0] {
                assert!((index_to_x(&v, x_to_index(&v, x)) - x).abs() < 1e-3);
            }
        }
        // Last bar center sits (offset + 0.5) bars left of the right edge.
        let v = vp(50, 10.0, 2.0, ScaleMode::Linear);
        assert!((v.x_of(49.0) - (110.0 - 25.0)).abs() < 1e-4);
        assert_eq!(v.bar_at(v.x_of(49.0) + 4.0), Some(49));
        assert_eq!(v.bar_at(109.0), None);
    }

    #[test]
    fn prepending_does_not_move_existing_bars() {
        let v = vp(100, 7.0, 3.0, ScaleMode::Linear);
        let x_newest = v.x_of(99.0);
        let v2 = Viewport {
            bar_count: 1100,
            ..v
        };
        assert!((v2.x_of(1099.0) - x_newest).abs() < 1e-3);
    }

    #[test]
    fn visible_range_clamps() {
        let r = vp(50, 10.0, 0.0, ScaleMode::Linear).visible_range();
        assert_eq!(r.end, 50);
        assert!((39..=40).contains(&r.start), "{r:?}");
        assert_eq!(
            vp(50, 10.0, 100.0, ScaleMode::Linear).visible_range().len(),
            0
        );
        assert_eq!(vp(0, 10.0, 0.0, ScaleMode::Linear).visible_range(), 0..0);
        assert_eq!(vp(50, 1.0, 0.0, ScaleMode::Linear).visible_range(), 0..50);
        // Scrolled into history: the newest bars are cut off on the right.
        let r = vp(50, 10.0, -20.0, ScaleMode::Linear).visible_range();
        assert!(r.end <= 31 && r.end >= 29, "{r:?}");
    }

    #[test]
    fn fit_pads_in_scaled_space() {
        for mode in MODES {
            let s = PriceScale::new(mode, 100.0);
            let (lo, hi) = fit_price_range(100.0, 200.0, s, 0.08);
            assert!(lo < 100.0 && hi > 200.0, "{mode:?}");
            let (a, b) = (s.to_scaled(lo), s.to_scaled(hi));
            let (c, d) = (s.to_scaled(100.0), s.to_scaled(200.0));
            assert!(close(c - a, (d - c) * 0.08, 1e-9), "{mode:?}");
            assert!(close(b - d, (d - c) * 0.08, 1e-9), "{mode:?}");
        }
        let (lo, hi) = fit_price_range(10.0, 10.0, PriceScale::linear(), 0.08);
        assert!(lo < 10.0 && hi > 10.0);
        let (lo, _) = fit_price_range(0.0, 0.0, PriceScale::new(ScaleMode::Logarithmic, 1.0), 0.08);
        assert!(lo > 0.0);
    }

    #[test]
    fn manual_zoom_and_pan() {
        for mode in MODES {
            let s = PriceScale::new(mode, 150.0);
            let (lo, hi) = zoom_price_range(100.0, 200.0, s, 2.0);
            let (a, b) = (s.to_scaled(lo), s.to_scaled(hi));
            let (c, d) = (s.to_scaled(100.0), s.to_scaled(200.0));
            assert!(close(b - a, 2.0 * (d - c), 1e-9), "{mode:?}");
            assert!(close((a + b) / 2.0, (c + d) / 2.0, 1e-9), "{mode:?}");

            let (lo2, hi2) = pan_price_range(100.0, 200.0, s, 0.5);
            assert!(lo2 > 100.0 && hi2 > 200.0, "{mode:?}");
            // Panning keeps the scaled span, so panning back by the same fraction returns home.
            let (back_lo, back_hi) = pan_price_range(lo2, hi2, s, -0.5);
            assert!(close(back_lo, 100.0, 1e-9), "{mode:?}: {back_lo}");
            assert!(close(back_hi, 200.0, 1e-9), "{mode:?}: {back_hi}");
        }
    }
}
