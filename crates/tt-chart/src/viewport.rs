//! Coordinate mapping between bars/prices and screen space. Backend agnostic.

use std::ops::Range;

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

/// Everything needed to map data to the screen for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub plot: PlotRect,
    /// Number of bars in the series.
    pub bar_count: usize,
    /// Horizontal distance between bar centers, in points.
    pub bar_spacing: f32,
    /// Empty space to the right of the last bar, in bars (negative = last bar scrolled off).
    pub right_offset: f32,
    pub price_lo: f64,
    pub price_hi: f64,
}

impl Viewport {
    /// Continuous bar index whose center is at the right edge.
    fn right_edge_index(&self) -> f32 {
        self.bar_count as f32 - 1.0 + self.right_offset + 0.5
    }

    /// X coordinate of the center of bar `i` (fractional indices allowed).
    pub fn x_of(&self, i: f32) -> f32 {
        self.plot.right - (self.right_edge_index() - i) * self.bar_spacing
    }

    /// Continuous bar index at screen x.
    pub fn index_at(&self, x: f32) -> f32 {
        self.right_edge_index() - (self.plot.right - x) / self.bar_spacing
    }

    /// Nearest existing bar to screen x, if any.
    pub fn bar_at(&self, x: f32) -> Option<usize> {
        let i = self.index_at(x).round();
        (i >= 0.0 && (i as usize) < self.bar_count).then_some(i as usize)
    }

    /// Indices of bars at least partially inside the plot.
    pub fn visible_range(&self) -> Range<usize> {
        visible_range(
            self.bar_count,
            self.bar_spacing,
            self.right_offset,
            self.plot.width(),
        )
    }

    pub fn y_of(&self, price: f64) -> f32 {
        let span = self.price_hi - self.price_lo;
        let t = if span > 0.0 {
            (price - self.price_lo) / span
        } else {
            0.5
        };
        self.plot.bottom - (t as f32) * self.plot.height()
    }

    pub fn price_at(&self, y: f32) -> f64 {
        let t = ((self.plot.bottom - y) / self.plot.height()) as f64;
        self.price_lo + t * (self.price_hi - self.price_lo)
    }
}

/// Visible bar range for the given horizontal view state.
/// Separate from [`Viewport`] so the price range can be computed from it first.
pub fn visible_range(
    bar_count: usize,
    bar_spacing: f32,
    right_offset: f32,
    width: f32,
) -> Range<usize> {
    if bar_count == 0 || bar_spacing <= 0.0 || width <= 0.0 {
        return 0..0;
    }
    let right_idx = bar_count as f32 - 1.0 + right_offset + 0.5;
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

/// Pads a `(lo, hi)` price range so bars don't touch the plot edges and a
/// flat range still gets some height.
pub fn padded_price_range(lo: f64, hi: f64, pad_frac: f64) -> (f64, f64) {
    let span = hi - lo;
    if span > 0.0 && span.is_finite() {
        (lo - span * pad_frac, hi + span * pad_frac)
    } else {
        let mid = if lo.is_finite() { lo } else { 0.0 };
        let half = (mid.abs() * 0.01).max(1e-8);
        (mid - half, mid + half)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp(n: usize, spacing: f32, off: f32) -> Viewport {
        Viewport {
            plot: PlotRect {
                left: 0.0,
                top: 0.0,
                right: 100.0,
                bottom: 200.0,
            },
            bar_count: n,
            bar_spacing: spacing,
            right_offset: off,
            price_lo: 100.0,
            price_hi: 200.0,
        }
    }

    #[test]
    fn x_and_index_are_inverse() {
        let v = vp(50, 10.0, 2.0);
        for i in [0.0, 10.0, 49.0] {
            assert!((v.index_at(v.x_of(i)) - i).abs() < 1e-4);
        }
        // Last bar center sits (offset + 0.5) bars left of the right edge.
        assert!((v.x_of(49.0) - (100.0 - 25.0)).abs() < 1e-4);
        assert_eq!(v.bar_at(v.x_of(49.0) + 4.0), Some(49));
        assert_eq!(v.bar_at(99.0), None);
    }

    #[test]
    fn visible_range_clamps() {
        // 100pt wide, 10pt bars, no offset → ~10 bars visible at the end.
        let r = vp(50, 10.0, 0.0).visible_range();
        assert_eq!(r.end, 50);
        assert!(r.start <= 40 && r.start >= 39, "{r:?}");
        // Scrolled far into the future → nothing visible.
        assert_eq!(vp(50, 10.0, 100.0).visible_range().len(), 0);
        assert_eq!(vp(0, 10.0, 0.0).visible_range(), 0..0);
        // Zoomed far out → everything.
        assert_eq!(vp(50, 1.0, 0.0).visible_range(), 0..50);
    }

    #[test]
    fn price_mapping() {
        let v = vp(1, 10.0, 0.0);
        assert_eq!(v.y_of(100.0), 200.0);
        assert_eq!(v.y_of(200.0), 0.0);
        assert!((v.price_at(100.0) - 150.0).abs() < 1e-9);
    }

    #[test]
    fn padding_handles_flat_range() {
        let (lo, hi) = padded_price_range(10.0, 10.0, 0.1);
        assert!(lo < 10.0 && hi > 10.0);
        assert_eq!(padded_price_range(0.0, 100.0, 0.1), (-10.0, 110.0));
    }
}
