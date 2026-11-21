//! Per-tick cost of every study at 1,000,000 bars.
//!
//! `cargo run --release -p tt-core --example bench_studies [-- BARS]`
//!
//! For each study: one full compute, then a live feed of `TICKS` ticks: the
//! forming bar is updated on every tick and a new bar is appended every
//! tenth tick, and only the tail is recomputed (as `Indicator::sync` does).
//! A counting allocator reports heap allocations per tick (appends may grow
//! an output `Vec`, amortized; updates should allocate nothing).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tt_core::{Candle, CandleSeries, Source, Study, StudyKind, StudyState};

struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const TICKS: usize = 2_000;

fn synthetic(n: usize) -> CandleSeries {
    let mut s = CandleSeries::new();
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut rnd = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut price = 100.0;
    for i in 0..n {
        let open: f64 = price;
        let close = open * (1.0 + (rnd() - 0.5) * 0.002);
        s.upsert(Candle {
            open_time_ms: 1_704_067_200_000 + i as i64 * 60_000,
            open,
            high: open.max(close) * (1.0 + rnd() * 0.0005),
            low: open.min(close) * (1.0 - rnd() * 0.0005),
            close,
            volume: 1.0 + rnd() * 50.0,
        });
        price = close;
    }
    s
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.replace('_', "").parse().ok())
        .unwrap_or(1_000_000);
    let base = synthetic(n);
    let mut studies: Vec<Study> = StudyKind::ALL.iter().map(|k| k.default_study()).collect();
    studies.extend([
        Study::Sma {
            length: 200,
            source: Source::Hlc3,
        },
        Study::Ema {
            length: 200,
            source: Source::Close,
        },
        Study::Stochastic {
            k: 14,
            k_smooth: 3,
            d: 3,
        },
    ]);
    println!(
        "{n} bars, {TICKS} ticks (update forming bar; append every 10th)\n{:<34} {:>9}  {:>18}  {:>11}",
        "study", "full ms", "tick avg/max µs", "allocs/tick"
    );
    for study in &studies {
        let mut s = base.clone();
        let mut out = Vec::new();
        let mut state = StudyState::default();
        let t = Instant::now();
        study.compute_with(&s, &mut out, &mut state, 0);
        let full = t.elapsed();
        let mut times = Vec::with_capacity(TICKS);
        let mut allocs = 0;
        let mut len = s.len();
        for tick in 0..TICKS {
            let mut c = s.last().expect("non-empty");
            if tick % 10 == 9 {
                c.open_time_ms += 60_000;
                c.open = c.close;
            }
            c.close *= 1.0 + if tick % 2 == 0 { 1e-4 } else { -1.3e-4 };
            c.high = c.high.max(c.close);
            c.low = c.low.min(c.close);
            c.volume += 0.5;
            s.upsert(c);
            let a0 = ALLOCS.load(Ordering::Relaxed);
            let t = Instant::now();
            // As `Indicator::sync`: the previously last bar onward.
            study.compute_with(&s, &mut out, &mut state, len - 1);
            times.push(t.elapsed());
            allocs += ALLOCS.load(Ordering::Relaxed) - a0;
            len = s.len();
        }
        std::hint::black_box(&out);
        let avg = times.iter().map(Duration::as_secs_f64).sum::<f64>() / TICKS as f64;
        let max = times.iter().map(Duration::as_secs_f64).fold(0.0, f64::max);
        println!(
            "{:<34} {:>9.2}  {:>8.2} / {:<8.2}  {:>11.3}",
            format!("{study:?}").chars().take(34).collect::<String>(),
            full.as_secs_f64() * 1e3,
            avg * 1e6,
            max * 1e6,
            allocs as f64 / TICKS as f64
        );
    }
}
