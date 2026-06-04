//! Mono sine-wave oscillator for the test-signal path.
//!
//! Pure domain block (A1-clean): no audio I/O, no GUI, no filesystem.

use std::f32::consts::TAU;

use crate::domain::process::Process;
use crate::domain::types::SampleRate;

/// A 440 Hz sine generator used when the test-signal toggle is active.
///
/// Provides both [`Process::process`] (fills a buffer with sine) and
/// [`Self::next_sample`] (returns one sample and advances phase) for
/// per-sample use inside the input callback.
pub struct TestOscillator {
    phase: f32,
    phase_inc: f32,
}

impl TestOscillator {
    /// Create a new oscillator at 440 Hz. Call [`Process::prepare`] with
    /// the session sample rate before use.
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            phase_inc: 0.0,
        }
    }

    /// Return the next sine sample and advance the phase accumulator.
    ///
    /// A2-safe: pure FP arithmetic.
    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        let sample = self.phase.sin();
        self.phase = (self.phase + self.phase_inc) % TAU;
        sample
    }
}

impl Default for TestOscillator {
    fn default() -> Self {
        Self::new()
    }
}

impl Process for TestOscillator {
    fn prepare(&mut self, sr: SampleRate, _max_block_size: usize) {
        self.phase_inc = TAU * 440.0 / sr.value();
    }

    fn reset(&mut self) {
        self.phase = 0.0;
    }

    fn process(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.next_sample();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero() {
        let mut osc = TestOscillator::new();
        osc.prepare(SampleRate::new(48_000.0), 0);
        // sin(0) = 0
        assert!((osc.next_sample()).abs() < 1e-6);
    }

    #[test]
    fn output_bounded() {
        let mut osc = TestOscillator::new();
        osc.prepare(SampleRate::new(48_000.0), 0);
        for _ in 0..48_000 {
            let s = osc.next_sample();
            assert!((-1.0..=1.0).contains(&s), "sample out of bounds: {s}");
        }
    }

    #[test]
    fn frequency_is_440() {
        // At 44100 Hz, one full cycle of 440 Hz = 44100/440 ≈ 100.23 samples.
        // After ~100 samples the phase should be near 2π (i.e. sin ≈ 0 again).
        let sr = 44_100.0;
        let mut osc = TestOscillator::new();
        osc.prepare(SampleRate::new(sr), 0);
        let period_samples = (sr / 440.0).round() as usize;
        for _ in 0..period_samples {
            osc.next_sample();
        }
        // After one period, should be near zero again.
        let val = osc.next_sample();
        assert!(val.abs() < 0.1, "expected near-zero after one period, got {val}");
    }

    #[test]
    fn reset_rewinds_phase() {
        let mut osc = TestOscillator::new();
        osc.prepare(SampleRate::new(48_000.0), 0);
        for _ in 0..1000 {
            osc.next_sample();
        }
        osc.reset();
        assert!((osc.next_sample()).abs() < 1e-6, "should be sin(0) after reset");
    }

    #[test]
    fn process_fills_buffer() {
        let mut osc = TestOscillator::new();
        osc.prepare(SampleRate::new(48_000.0), 512);
        let mut buf = vec![0.0f32; 512];
        osc.process(&mut buf);
        // At least some non-zero values
        assert!(buf.iter().any(|s| s.abs() > 0.01));
        // All bounded
        assert!(buf.iter().all(|s| (-1.0..=1.0).contains(s)));
    }
}
