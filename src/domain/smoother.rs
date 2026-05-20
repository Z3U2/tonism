//! Linear per-sample smoother for click-free parameter changes (C4).
//!
//! Owned by the audio thread; reads no atomics, takes no locks, performs
//! no I/O. Pure domain code per architecture rule A1.
//!
//! The smoother advances its internal `current` value one sample per
//! [`LinearSmoother::next`] call. When [`LinearSmoother::set_target`] is
//! called with a new value, it computes a per-sample step from the
//! current value to the new target, taking the configured smoothing
//! time. Subsequent `.next()` calls walk the step until the target is
//! reached, then return the target exactly.
//!
//! # Lifecycle
//!
//! 1. Construct with [`LinearSmoother::new`] (initial value + smoothing
//!    time in seconds).
//! 2. Call [`LinearSmoother::prepare`] with the session sample rate
//!    before the first `.next()`. May be re-called on sample-rate
//!    change (Phase G); preserves `current` and `target` per C3's
//!    persistence guarantee.
//! 3. Call [`LinearSmoother::set_target`] whenever the target changes
//!    (cheap when unchanged: a single FP compare).
//! 4. Call [`LinearSmoother::next`] once per audio frame.

use crate::domain::types::SampleRate;

/// Linear ramp smoother. Matches `SmoothingStyle::Linear(ms)` from the
/// nih-plug surface this replaces.
#[derive(Debug, Clone)]
pub struct LinearSmoother {
    /// Current smoothed value — what `.next()` returns.
    current: f32,
    /// Destination value the smoother is ramping toward.
    target: f32,
    /// Per-sample increment toward `target`. Recomputed when
    /// `set_target` sees a new target or `prepare` updates the SR.
    step: f32,
    /// Samples remaining in the active ramp. When 0, the smoother is
    /// "settled" and `.next()` just returns `current` (== `target`).
    samples_remaining: u32,
    /// Smoothing duration in seconds. Fixed at construction; survives
    /// `prepare` (so SR changes only re-derive `step`).
    smoothing_time_secs: f32,
    /// Session sample rate in Hz. Set by `prepare`. Defaults to 44.1 kHz
    /// before the first `prepare` so `.next()` is well-defined even if
    /// called early.
    sr: f32,
}

impl LinearSmoother {
    /// Construct a settled smoother at `initial`. `smoothing_time_secs`
    /// is the duration of a full target-to-target ramp; typical values
    /// are 0.005–0.050 seconds for audio parameter trims.
    pub fn new(initial: f32, smoothing_time_secs: f32) -> Self {
        Self {
            current: initial,
            target: initial,
            step: 0.0,
            samples_remaining: 0,
            smoothing_time_secs,
            // 44.1 kHz keeps `next()` well-defined before `prepare`;
            // the audio path calls `prepare` before the first `next`.
            sr: 44_100.0,
        }
    }

    /// Configure the smoother for a new sample rate. Preserves
    /// `current` and `target` per C3's "params survive stream restart"
    /// guarantee. If a ramp is in progress, it is re-derived for the
    /// new SR so the total duration in seconds is unchanged.
    pub fn prepare(&mut self, sr: SampleRate) {
        self.sr = sr.value();
        if self.samples_remaining > 0 {
            // Re-derive the in-progress ramp at the new SR.
            let total = self.samples_in_full_ramp();
            self.samples_remaining = total;
            self.step = (self.target - self.current) / total as f32;
        }
    }

    /// Set the destination value. Cheap when unchanged (single FP
    /// compare). On change, computes a fresh ramp from `current` to
    /// `target` taking `smoothing_time_secs` seconds.
    pub fn set_target(&mut self, target: f32) {
        if target == self.target {
            return;
        }
        self.target = target;
        let total = self.samples_in_full_ramp();
        self.samples_remaining = total;
        self.step = (self.target - self.current) / total as f32;
    }

    /// Advance one sample. Returns the current smoothed value.
    ///
    /// A2-clean: no alloc, no lock, no syscall. Pure FP arithmetic
    /// + integer decrement.
    ///
    /// Name shadows [`Iterator::next`] intentionally — matches the
    /// nih-plug `Smoother::next` surface this replaces, and the audio
    /// thread reads it as `smoother.next()` every frame. Implementing
    /// `Iterator` would force `Option<f32>` and per-frame `.unwrap()`,
    /// adding A2 risk for no gain.
    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn next(&mut self) -> f32 {
        if self.samples_remaining > 0 {
            self.current += self.step;
            self.samples_remaining -= 1;
            if self.samples_remaining == 0 {
                // Snap to exact target on completion so accumulated FP
                // error doesn't leave us short.
                self.current = self.target;
            }
        }
        self.current
    }

    /// Jump immediately to `target`. Useful for first-frame init or
    /// when an in-progress ramp must be cut (e.g. bypass toggle).
    pub fn snap_to_target(&mut self) {
        self.current = self.target;
        self.samples_remaining = 0;
        self.step = 0.0;
    }

    /// Read-only access to the current (smoothed) value without
    /// advancing. Useful for diagnostics / tests.
    pub fn current(&self) -> f32 {
        self.current
    }

    /// Read-only access to the target. Useful for diagnostics / tests.
    pub fn target(&self) -> f32 {
        self.target
    }

    /// Sample count for a full-duration ramp at the current SR.
    /// Minimum of 1 so a zero-duration smoother degenerates to "snap
    /// to target on next call" rather than dividing by zero.
    fn samples_in_full_ramp(&self) -> u32 {
        (self.smoothing_time_secs * self.sr).max(1.0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn settled_smoother_returns_initial() {
        let mut s = LinearSmoother::new(0.5, 0.020);
        s.prepare(SampleRate::new(48_000.0));
        for _ in 0..100 {
            assert_eq!(s.next(), 0.5);
        }
    }

    #[test]
    fn ramp_reaches_target_exactly_at_end() {
        // 10 ms ramp at 1000 Hz → 10 samples in the full ramp.
        let mut s = LinearSmoother::new(0.0, 0.010);
        s.prepare(SampleRate::new(1_000.0));
        s.set_target(1.0);
        for _ in 0..9 {
            s.next();
        }
        let final_value = s.next();
        assert_eq!(final_value, 1.0, "expected exact snap to target");
    }

    #[test]
    fn ramp_is_monotonic_and_bounded() {
        // 100 ms ramp at 1000 Hz → 100 samples.
        let mut s = LinearSmoother::new(0.0, 0.100);
        s.prepare(SampleRate::new(1_000.0));
        s.set_target(1.0);
        let mut prev = 0.0_f32;
        for _ in 0..100 {
            let v = s.next();
            assert!(v >= prev, "smoother went backward: {prev} → {v}");
            assert!((0.0..=1.0).contains(&v), "smoother out of bounds: {v}");
            prev = v;
        }
    }

    #[test]
    fn set_same_target_is_noop() {
        let mut s = LinearSmoother::new(0.3, 0.020);
        s.prepare(SampleRate::new(48_000.0));
        s.set_target(0.3); // same as initial
        assert_eq!(s.next(), 0.3);
        assert_eq!(s.next(), 0.3);
    }

    #[test]
    fn snap_to_target_short_circuits_ramp() {
        let mut s = LinearSmoother::new(0.0, 0.500);
        s.prepare(SampleRate::new(48_000.0));
        s.set_target(1.0);
        s.next(); // partial step
        assert!(s.current() > 0.0 && s.current() < 1.0);
        s.snap_to_target();
        assert_eq!(s.current(), 1.0);
        assert_eq!(s.next(), 1.0);
    }

    #[test]
    fn retargeting_mid_ramp_recomputes_step() {
        let mut s = LinearSmoother::new(0.0, 0.010);
        s.prepare(SampleRate::new(1_000.0)); // 10 samples per ramp
        s.set_target(1.0);
        for _ in 0..5 {
            s.next(); // halfway: ~0.5
        }
        assert!(approx_eq(s.current(), 0.5, 1e-6));
        // Retarget to 0.0; new ramp is 10 samples from 0.5 → 0.0.
        s.set_target(0.0);
        for _ in 0..9 {
            s.next();
        }
        assert_eq!(s.next(), 0.0, "expected snap to new target");
    }

    #[test]
    fn negative_target_works() {
        let mut s = LinearSmoother::new(1.0, 0.010);
        s.prepare(SampleRate::new(1_000.0));
        s.set_target(-1.0);
        for _ in 0..9 {
            s.next();
        }
        assert_eq!(s.next(), -1.0);
    }

    #[test]
    fn prepare_reflows_ramp_across_sr_change() {
        // Start a ramp at 1 kHz, then switch SR to 2 kHz partway
        // through. The new ramp should take 2× the remaining samples.
        let mut s = LinearSmoother::new(0.0, 0.010); // 10 ms
        s.prepare(SampleRate::new(1_000.0)); // 10 samples for full ramp
        s.set_target(1.0);
        for _ in 0..3 {
            s.next();
        }
        // SR doubles. Ramp's remaining time is the same in seconds (so
        // remaining samples = 10 at the new rate, not the 7 left at the
        // old rate). Re-deriving from current → target at the new SR
        // is the documented behaviour.
        s.prepare(SampleRate::new(2_000.0));
        for _ in 0..19 {
            s.next();
        }
        assert_eq!(s.next(), 1.0);
    }

    #[test]
    fn next_before_prepare_uses_default_sr() {
        // Useful for callers that construct + advance briefly before
        // wiring up the audio stream. Doesn't panic; uses 44.1 kHz.
        let mut s = LinearSmoother::new(0.0, 0.010);
        s.set_target(1.0);
        // 10 ms at 44.1 kHz ≈ 441 samples.
        for _ in 0..441 {
            s.next();
        }
        assert!(approx_eq(s.current(), 1.0, 1e-6));
    }
}
