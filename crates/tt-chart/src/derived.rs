//! Series derived from the raw bars for some chart types: Heikin Ashi
//! (incremental) and the brick types Renko / Line break (rebuilt when the
//! source changes; they reflow anyway when a brick forms).

use crate::ChartType;
use tt_core::{Candle, CandleSeries, HeikinAshi, line_break, renko, renko_box_size};

#[derive(Debug, Clone, Default)]
pub struct Derived {
    ha: HeikinAshi,
    bricks: CandleSeries,
    bricks_kind: Option<ChartType>,
    src_version: Option<u64>,
    src_len: usize,
    src_last: Option<Candle>,
}

impl Derived {
    /// The series to display for `kind`, or `None` to show the raw bars.
    pub fn sync(&mut self, kind: ChartType, src: &CandleSeries) -> Option<&CandleSeries> {
        match kind {
            ChartType::HeikinAshi => Some(self.ha.sync(src)),
            ChartType::Renko | ChartType::LineBreak => {
                let changed = self.bricks_kind != Some(kind)
                    || self.src_version != Some(src.version())
                    || self.src_len != src.len()
                    || self.src_last != src.last();
                if changed {
                    self.bricks = if kind == ChartType::Renko {
                        renko(src, renko_box_size(src))
                    } else {
                        line_break(src, 3)
                    };
                    self.bricks_kind = Some(kind);
                    self.src_version = Some(src.version());
                    self.src_len = src.len();
                    self.src_last = src.last();
                }
                Some(&self.bricks)
            }
            _ => None,
        }
    }
}
