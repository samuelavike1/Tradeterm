//! Checkpointed recursive state, so a live tick resumes a study at the
//! forming bar instead of recomputing its history.
//!
//! Recursive formulas whose state is not visible in the outputs (RSI's two
//! RMAs, MACD's fast and slow EMAs, running window sums, VWAP's session
//! sums) save the state they had *before* the last bar. The next
//! [`Study::compute_with`](super::Study::compute_with) from that bar (a tick
//! on the forming bar, or bars appended after it) restores it and replays
//! the same arithmetic in the same order as a full pass, so the result is
//! bit-identical to a full recompute at O(1) (or O(window)) per tick.
//!
//! Studies whose state is the previous output (EMA, RMA/ATR, OBV, the MACD
//! signal line) or that only read a window of inputs (Bollinger Bands) need
//! no checkpoint.

use super::ma::MaState;

/// The resumable state of one study instance. Pair it with one study and
/// one series: [`Study::compute_with`](super::Study::compute_with) from
/// 0 resets it, and a `from` other than the checkpoint falls back to a full
/// pass, so a stale state is never wrong, only slower.
#[derive(Debug, Clone, Default)]
pub struct StudyState {
    /// The checkpoint is the state before bar `at` was processed.
    at: usize,
    ck: Ck,
    /// Reusable per-study buffer (no allocation once it has grown).
    pub(super) scratch: Vec<f64>,
}

impl StudyState {
    /// Where to start computing for a request from `from`, and the state to
    /// start with: the checkpoint if it was taken at `from`, otherwise bar 0
    /// and a fresh state.
    pub(super) fn resume<S: Resumable>(&self, from: usize) -> (usize, S) {
        match S::get(&self.ck) {
            Some(s) if from > 0 && self.at == from => (from, s),
            _ => (0, S::default()),
        }
    }

    /// Saves `s`, the state before bar `at`.
    pub(super) fn save<S: Resumable>(&mut self, at: usize, s: S) {
        self.at = at;
        self.ck = s.put();
    }

    /// Bar the checkpoint resumes at (`None` when there is none).
    pub fn checkpoint(&self) -> Option<usize> {
        (!matches!(self.ck, Ck::None)).then_some(self.at)
    }
}

/// A state type a study can checkpoint.
pub(super) trait Resumable: Copy + Default {
    fn get(c: &Ck) -> Option<Self>;
    fn put(self) -> Ck;
}

macro_rules! checkpoints {
    ($($variant:ident($ty:ty)),* $(,)?) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub(super) enum Ck {
            #[default]
            None,
            $($variant($ty)),*
        }
        $(impl Resumable for $ty {
            fn get(c: &Ck) -> Option<Self> {
                match c {
                    Ck::$variant(s) => Some(*s),
                    _ => None,
                }
            }
            fn put(self) -> Ck {
                Ck::$variant(self)
            }
        })*
    };
}

checkpoints! {
    Ma(MaState),
    Ma2([MaState; 2]),
    Rsi(super::rsi::State),
    Macd(super::macd::State),
    Vwap(super::vwap::State),
}
