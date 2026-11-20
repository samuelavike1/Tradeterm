//! Rolling performance statistics for the F12 overlay.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Number of frames the averages/maxima cover.
pub const WINDOW: usize = 120;

/// Fixed-size window of samples (milliseconds).
#[derive(Debug, Default, Clone)]
pub struct Samples {
    values: VecDeque<f32>,
}

impl Samples {
    pub fn push(&mut self, ms: f32) {
        if self.values.len() == WINDOW {
            self.values.pop_front();
        }
        self.values.push_back(ms);
    }

    pub fn avg(&self) -> f32 {
        if self.values.is_empty() {
            0.0
        } else {
            self.values.iter().sum::<f32>() / self.values.len() as f32
        }
    }

    pub fn max(&self) -> f32 {
        self.values.iter().copied().fold(0.0, f32::max)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }
}

/// Events-per-second over a sliding one-second window.
#[derive(Debug, Default)]
pub struct RateCounter {
    hits: VecDeque<(Instant, u32)>,
}

impl RateCounter {
    pub fn add(&mut self, now: Instant, n: u32) {
        if n > 0 {
            self.hits.push_back((now, n));
        }
        self.trim(now);
    }

    pub fn per_second(&mut self, now: Instant) -> u32 {
        self.trim(now);
        self.hits.iter().map(|(_, n)| n).sum()
    }

    fn trim(&mut self, now: Instant) {
        while let Some((t, _)) = self.hits.front() {
            if now.saturating_duration_since(*t) > Duration::from_secs(1) {
                self.hits.pop_front();
            } else {
                break;
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct PerfStats {
    pub visible: bool,
    /// Whole-frame CPU time reported by eframe (UI + tessellation).
    pub frame: Samples,
    /// Time spent in our `ui()` (all panels + chart scene building).
    pub ui: Samples,
    /// Chart scene building and painting only.
    pub chart: Samples,
    pub events: RateCounter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_window() {
        let mut s = Samples::default();
        assert_eq!(s.avg(), 0.0);
        for i in 0..200 {
            s.push(i as f32);
        }
        assert_eq!(s.len(), WINDOW);
        // Only the last 120 (80..199) remain.
        assert_eq!(s.max(), 199.0);
        assert!((s.avg() - 139.5).abs() < 1e-3);
    }

    #[test]
    fn rate_counts_last_second_only() {
        let mut r = RateCounter::default();
        let t0 = Instant::now();
        r.add(t0, 5);
        r.add(t0 + Duration::from_millis(500), 3);
        assert_eq!(r.per_second(t0 + Duration::from_millis(900)), 8);
        assert_eq!(r.per_second(t0 + Duration::from_millis(1200)), 3);
        assert_eq!(r.per_second(t0 + Duration::from_secs(5)), 0);
    }
}
