//! Smooth viewport motion: eased mouse-wheel zoom/scroll and momentum after
//! a fling. Pure math, driven frame by frame by the chart view.
//!
//! - Wheel notches (inexact "line" deltas) are added to a pending amount that
//!   is applied with exponential easing: ~95% within 120 ms, so zooming
//!   glides instead of jumping ~12% per notch. Trackpads send exact pixel
//!   deltas, which are already smooth and are applied directly.
//! - Releasing a drag while moving keeps the chart panning with friction
//!   (about a third of a second), like TradingView.

/// Time constant of wheel easing (seconds); 3τ ≈ 95% applied.
const SMOOTH_TAU: f32 = 0.040;
/// Time constant of momentum friction (seconds).
const FRICTION_TAU: f32 = 0.28;
/// Momentum below this (points/s) stops.
const STOP_SPEED: f32 = 20.0;
/// Remaining wheel zoom (ln factor) / pan (points) below these is applied at once.
const ZOOM_EPS: f32 = 1e-4;
const PAN_EPS: f32 = 0.05;
/// Flings slower than this (points/s) don't start momentum.
pub const FLING_MIN_SPEED: f32 = 250.0;
/// Momentum speed cap (points/s).
const FLING_MAX_SPEED: f32 = 6000.0;

/// One frame's worth of motion to apply.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MotionStep {
    /// Zoom by `exp(zoom)` around `anchor_x`.
    pub zoom: f32,
    pub anchor_x: f32,
    /// Pan by this many points (positive = like dragging right).
    pub pan: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ViewMotion {
    zoom: f32,
    anchor_x: f32,
    pan: f32,
    velocity: f32,
}

impl ViewMotion {
    /// Adds a wheel zoom of `ln_factor` around `anchor_x`.
    pub fn add_zoom(&mut self, ln_factor: f32, anchor_x: f32) {
        self.zoom += ln_factor;
        self.anchor_x = anchor_x;
    }

    /// Adds a wheel pan of `dx` points.
    pub fn add_pan(&mut self, dx: f32) {
        self.pan += dx;
    }

    /// Starts momentum at `velocity` points/s (ignored if too slow).
    pub fn fling(&mut self, velocity: f32) {
        if velocity.abs() >= FLING_MIN_SPEED {
            self.velocity = velocity.clamp(-FLING_MAX_SPEED, FLING_MAX_SPEED);
        }
    }

    /// Stops momentum (e.g. the user grabbed the chart again).
    pub fn stop_momentum(&mut self) {
        self.velocity = 0.0;
    }

    pub fn active(&self) -> bool {
        self.zoom.abs() > 0.0 || self.pan.abs() > 0.0 || self.velocity.abs() > 0.0
    }

    /// Advances by `dt` seconds; returns what to apply this frame.
    pub fn step(&mut self, dt: f32) -> MotionStep {
        let dt = dt.clamp(0.0, 0.05);
        let k = 1.0 - (-dt / SMOOTH_TAU).exp();
        let mut zoom = self.zoom * k;
        let mut pan = self.pan * k;
        self.zoom -= zoom;
        self.pan -= pan;
        // Finish tiny remainders instead of easing forever.
        if self.zoom.abs() < ZOOM_EPS {
            zoom += self.zoom;
            self.zoom = 0.0;
        }
        if self.pan.abs() < PAN_EPS {
            pan += self.pan;
            self.pan = 0.0;
        }
        if self.velocity != 0.0 {
            // Exact integral of v·e^(−t/τ) over the frame.
            let decay = (-dt / FRICTION_TAU).exp();
            pan += self.velocity * FRICTION_TAU * (1.0 - decay);
            self.velocity *= decay;
            if self.velocity.abs() < STOP_SPEED {
                self.velocity = 0.0;
            }
        }
        MotionStep {
            zoom,
            anchor_x: self.anchor_x,
            pan,
        }
    }
}

/// Smoothed pointer velocity while dragging (points/s).
#[derive(Debug, Clone, Copy, Default)]
pub struct DragVelocity {
    velocity: f32,
    last: Option<std::time::Instant>,
}

impl DragVelocity {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Records a move of `dx` points at `now`.
    pub fn record(&mut self, dx: f32, now: std::time::Instant) {
        if let Some(last) = self.last {
            let dt = (now - last).as_secs_f32();
            if dt > 0.0 {
                let v = dx / dt;
                self.velocity = 0.6 * v + 0.4 * self.velocity;
            }
        }
        self.last = Some(now);
    }

    /// Velocity at release; zero if the pointer had stopped (no move in the
    /// last 60 ms), so a drag that ends still doesn't fling.
    pub fn release(&self, now: std::time::Instant) -> f32 {
        match self.last {
            Some(t) if (now - t).as_millis() <= 60 => self.velocity,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(m: &mut ViewMotion, frames: usize) -> MotionStep {
        let mut total = MotionStep::default();
        for _ in 0..frames {
            let s = m.step(1.0 / 60.0);
            total.zoom += s.zoom;
            total.pan += s.pan;
        }
        total
    }

    #[test]
    fn wheel_zoom_eases_to_exactly_the_total() {
        let mut m = ViewMotion::default();
        m.add_zoom(0.12, 500.0);
        let first = m.step(1.0 / 60.0);
        assert!(
            first.zoom > 0.03 && first.zoom < 0.07,
            "first frame applies a part: {}",
            first.zoom
        );
        let rest = run(&mut m, 30);
        assert!((first.zoom + rest.zoom - 0.12).abs() < 1e-6);
        assert!(!m.active(), "settles within half a second");
        // ~95% within 120 ms (≈ 7 frames at 60 Hz).
        let mut m = ViewMotion::default();
        m.add_zoom(1.0, 0.0);
        assert!(run(&mut m, 7).zoom > 0.94);
    }

    #[test]
    fn notches_accumulate() {
        let mut m = ViewMotion::default();
        m.add_zoom(0.1, 0.0);
        m.step(1.0 / 60.0);
        m.add_zoom(0.1, 0.0);
        let total = run(&mut m, 60).zoom;
        assert!(total > 0.1 && total < 0.2);
    }

    #[test]
    fn momentum_decays_and_travels_v_times_tau() {
        let mut m = ViewMotion::default();
        m.fling(2000.0);
        let travel = run(&mut m, 120).pan;
        assert!((travel - 2000.0 * FRICTION_TAU).abs() < 10.0, "{travel}");
        assert!(!m.active());
        let mut slow = ViewMotion::default();
        slow.fling(100.0);
        assert!(!slow.active(), "slow releases don't fling");
    }

    #[test]
    fn drag_velocity_only_counts_recent_movement() {
        let t0 = std::time::Instant::now();
        let ms = std::time::Duration::from_millis;
        let mut d = DragVelocity::default();
        d.record(0.0, t0);
        d.record(20.0, t0 + ms(10));
        d.record(20.0, t0 + ms(20));
        assert!(d.release(t0 + ms(25)) > 1000.0);
        assert_eq!(d.release(t0 + ms(200)), 0.0, "paused before release");
    }
}
