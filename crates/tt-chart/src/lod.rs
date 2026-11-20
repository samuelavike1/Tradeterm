//! Level-of-detail pyramid for drawing huge series.
//!
//! When bars are narrower than ~2 px, drawing each bar is wasted work. This
//! module keeps a min/max pyramid over the series: level `k` aggregates
//! blocks of `8 << k` bars (open of first, max high, min low, close of last,
//! min/max close, summed volume). Per frame, each physical pixel column is
//! built from the coarsest level whose blocks still fit inside one column, so
//! drawing 1,000,000 bars on a 1,600 px plot touches ~2,000 blocks.
//!
//! Live updates touch only the last block of each level (O(levels)); the
//! pyramid is rebuilt only when the series changes structurally.

use crate::bars::DrawBar;
use crate::viewport::Viewport;
use std::ops::Range;
use tt_core::{Candle, CandleSeries};

/// Raw bars per block at level 0.
const BASE_BLOCK: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Agg {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    close_min: f64,
    close_max: f64,
    volume: f64,
}

impl Agg {
    fn bar(s: &CandleSeries, i: usize) -> Self {
        let c = s.closes()[i];
        Self {
            open: s.opens()[i],
            high: s.highs()[i],
            low: s.lows()[i],
            close: c,
            close_min: c,
            close_max: c,
            volume: s.volumes()[i],
        }
    }

    /// `self` followed by `next` in time.
    fn then(self, next: Agg) -> Agg {
        Agg {
            open: self.open,
            high: self.high.max(next.high),
            low: self.low.min(next.low),
            close: next.close,
            close_min: self.close_min.min(next.close_min),
            close_max: self.close_max.max(next.close_max),
            volume: self.volume + next.volume,
        }
    }
}

/// Min/max pyramid; see the module docs.
#[derive(Debug, Clone, Default)]
pub struct Lod {
    /// `levels[k]` holds blocks of `BASE_BLOCK << k` bars.
    levels: Vec<Vec<Agg>>,
    src_version: Option<u64>,
    src_len: usize,
    src_last: Option<Candle>,
}

impl Lod {
    pub fn new() -> Self {
        Self::default()
    }

    fn block(k: usize) -> usize {
        BASE_BLOCK << k
    }

    /// Brings the pyramid up to date with `src`.
    pub fn sync(&mut self, src: &CandleSeries) {
        let same_structure = self.src_version == Some(src.version()) && src.len() >= self.src_len;
        if same_structure && self.src_len == src.len() && self.src_last == src.last() {
            return;
        }
        if !same_structure {
            self.rebuild(src);
        } else {
            // Last bar updated and/or bars appended.
            for i in self.src_len.saturating_sub(1)..src.len() {
                self.update_bar(src, i);
            }
            self.grow_top();
        }
        self.src_version = Some(src.version());
        self.src_len = src.len();
        self.src_last = src.last();
    }

    fn rebuild(&mut self, src: &CandleSeries) {
        self.levels.clear();
        let n = src.len();
        if n == 0 {
            return;
        }
        let level0: Vec<Agg> = (0..n.div_ceil(BASE_BLOCK))
            .map(|j| Self::from_raw(src, j))
            .collect();
        self.levels.push(level0);
        self.grow_top();
    }

    /// Adds coarser levels until the top level is a single block.
    fn grow_top(&mut self) {
        while let Some(top) = self.levels.last()
            && top.len() > 1
        {
            let next: Vec<Agg> = top
                .chunks(2)
                .map(|c| if c.len() == 2 { c[0].then(c[1]) } else { c[0] })
                .collect();
            self.levels.push(next);
        }
    }

    fn from_raw(src: &CandleSeries, j: usize) -> Agg {
        let start = j * BASE_BLOCK;
        let end = (start + BASE_BLOCK).min(src.len());
        (start + 1..end).fold(Agg::bar(src, start), |a, i| a.then(Agg::bar(src, i)))
    }

    /// Recomputes every block containing raw bar `i`.
    fn update_bar(&mut self, src: &CandleSeries, i: usize) {
        if self.levels.is_empty() {
            self.levels.push(Vec::new());
        }
        for k in 0..self.levels.len() {
            let j = i / Self::block(k);
            let agg = if k == 0 {
                Self::from_raw(src, j)
            } else {
                let child = &self.levels[k - 1];
                match child.get(2 * j + 1) {
                    Some(b) => child[2 * j].then(*b),
                    None => child[2 * j],
                }
            };
            let level = &mut self.levels[k];
            if j < level.len() {
                level[j] = agg;
            } else {
                level.push(agg);
            }
        }
    }

    /// Per-pixel-column bars for the visible `range`.
    ///
    /// Each column is one physical pixel wide; `x` of each returned bar is the
    /// left edge of its pixel (in points), suitable for 1 px wide columns.
    pub fn columns(
        &self,
        src: &CandleSeries,
        range: Range<usize>,
        vp: &Viewport,
        ppp: f32,
    ) -> Vec<DrawBar> {
        let mut out: Vec<DrawBar> = Vec::with_capacity((vp.plot.width() * ppp) as usize + 2);
        if range.is_empty() {
            return out;
        }
        let bars_per_px = 1.0 / (vp.bar_spacing * ppp).max(1e-9);
        // Coarsest level whose blocks fit in one pixel column.
        let level = (0..self.levels.len())
            .rev()
            .find(|&k| Self::block(k) as f32 <= bars_per_px);

        let mut acc: Option<(i64, Agg)> = None;
        let mut prev_close: Option<f64> = None;
        let mut flush = |col: i64, a: Agg, out: &mut Vec<DrawBar>| {
            out.push(DrawBar {
                x: col as f32 / ppp,
                open: a.open,
                high: a.high,
                low: a.low,
                close: a.close,
                prev_close: prev_close.unwrap_or(a.open),
                close_min: a.close_min,
                close_max: a.close_max,
                volume: a.volume,
            });
            prev_close = Some(a.close);
        };
        let mut add = |center: f32, a: Agg, out: &mut Vec<DrawBar>| {
            let col = (vp.x_of(center) * ppp).floor() as i64;
            match &mut acc {
                Some((c, sum)) if *c == col => *sum = sum.then(a),
                _ => {
                    if let Some((c, sum)) = acc.take() {
                        flush(c, sum, out);
                    }
                    acc = Some((col, a));
                }
            }
        };

        match level {
            Some(k) => {
                let b = Self::block(k);
                let blocks = &self.levels[k];
                let last = ((range.end - 1) / b).min(blocks.len() - 1);
                for (j, agg) in blocks
                    .iter()
                    .enumerate()
                    .take(last + 1)
                    .skip(range.start / b)
                {
                    let center = (j * b) as f32 + (b as f32 - 1.0) / 2.0;
                    add(center, *agg, &mut out);
                }
            }
            None => {
                for i in range {
                    add(i as f32, Agg::bar(src, i), &mut out);
                }
            }
        }
        if let Some((c, sum)) = acc {
            flush(c, sum, &mut out);
        }
        out
    }

    #[cfg(test)]
    fn level(&self, k: usize) -> &[Agg] {
        &self.levels[k]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScaleMode;
    use crate::viewport::{PlotRect, PriceScale};

    fn series(n: usize) -> CandleSeries {
        let mut s = CandleSeries::new();
        for i in 0..n {
            let p = 100.0 + ((i * 37) % 101) as f64 - 50.0;
            s.upsert(Candle {
                open_time_ms: i as i64 * 60_000,
                open: p,
                high: p + 1.0 + (i % 7) as f64,
                low: p - 1.0 - (i % 5) as f64,
                close: p + 0.5,
                volume: (i % 13) as f64,
            });
        }
        s
    }

    fn brute(s: &CandleSeries, r: Range<usize>) -> Agg {
        let mut a = Agg::bar(s, r.start);
        for i in r.start + 1..r.end {
            a = a.then(Agg::bar(s, i));
        }
        a
    }

    fn vp(n: usize, spacing: f32, width: f32) -> Viewport {
        Viewport {
            bar_spacing: spacing,
            right_offset_bars: 0.0,
            price_min: 0.0,
            price_max: 1.0,
            plot: PlotRect {
                left: 0.0,
                top: 0.0,
                right: width,
                bottom: 100.0,
            },
            bar_count: n,
            scale: PriceScale::new(ScaleMode::Linear, 1.0),
        }
    }

    #[test]
    fn levels_match_brute_force() {
        let s = series(1000);
        let mut lod = Lod::new();
        lod.sync(&s);
        for k in 0..4 {
            let b = Lod::block(k);
            for (j, agg) in lod.level(k).iter().enumerate() {
                let r = j * b..((j + 1) * b).min(1000);
                assert_eq!(*agg, brute(&s, r), "level {k} block {j}");
            }
        }
        assert_eq!(lod.levels.last().unwrap().len(), 1);
    }

    #[test]
    fn incremental_updates_match_rebuild() {
        let mut s = series(500);
        let mut lod = Lod::new();
        lod.sync(&s);
        for step in 0..300 {
            let n = s.len();
            let mut c = s.last().unwrap();
            if step % 3 == 0 {
                c.open_time_ms += 60_000; // append
            }
            c.high += 2.0;
            c.close += 1.0;
            c.volume += 1.0;
            s.upsert(c);
            lod.sync(&s);
            let mut fresh = Lod::new();
            fresh.sync(&s);
            assert_eq!(lod.levels, fresh.levels, "step {step} (len {n})");
        }
    }

    #[test]
    fn structural_change_rebuilds() {
        let mut s = series(100);
        let mut lod = Lod::new();
        lod.sync(&s);
        s.prepend(series(50).open_times().iter().map(|t| Candle {
            open_time_ms: t - 1_000_000_000,
            open: 1.0,
            high: 1000.0,
            low: 0.5,
            close: 1.0,
            volume: 0.0,
        }));
        lod.sync(&s);
        assert_eq!(lod.levels.last().unwrap()[0].high, 1000.0);
    }

    #[test]
    fn columns_preserve_extremes_and_volume() {
        let n = 100_000;
        let s = series(n);
        let mut lod = Lod::new();
        lod.sync(&s);
        // 100k bars on 500 px: 200 bars per pixel.
        let v = vp(n, 500.0 / n as f32, 500.0);
        let cols = lod.columns(&s, 0..n, &v, 1.0);
        assert!(cols.len() <= 502 && cols.len() >= 490, "{}", cols.len());
        let all = brute(&s, 0..n);
        let hi = cols.iter().map(|c| c.high).fold(f64::MIN, f64::max);
        let lo = cols.iter().map(|c| c.low).fold(f64::MAX, f64::min);
        let vol: f64 = cols.iter().map(|c| c.volume).sum();
        assert_eq!((hi, lo), (all.high, all.low));
        assert!((vol - all.volume).abs() < 1e-6);
        assert_eq!(cols[0].open, s.opens()[0]);
        assert!(cols.windows(2).all(|w| w[0].x < w[1].x));
    }

    #[test]
    fn few_bars_per_pixel_uses_raw_bars() {
        let n = 3000;
        let s = series(n);
        let mut lod = Lod::new();
        lod.sync(&s);
        // 3 bars per pixel: below the smallest block, so raw bars are merged.
        let v = vp(n, 1.0 / 3.0, 1000.0);
        let cols = lod.columns(&s, 0..n, &v, 1.0);
        assert!((999..=1001).contains(&cols.len()), "{}", cols.len());
        let vol: f64 = cols.iter().map(|c| c.volume).sum();
        assert!((vol - brute(&s, 0..n).volume).abs() < 1e-6);
    }
}
