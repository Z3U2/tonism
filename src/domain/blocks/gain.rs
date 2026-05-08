use crate::domain::process::Process;
use crate::domain::types::{Decibels, GainLinear};

/// A simple gain block that scales every sample by a fixed dB value.
///
/// This is a stub block used to prove the domain processing path is alive.
/// Real DSP wiring lands in Phase 4.
pub struct Gain {
    pub db: Decibels,
}

impl Process for Gain {
    fn process(&mut self, buffer: &mut [f32]) {
        let gain_linear: GainLinear = self.db.into();
        for s in buffer {
            *s *= gain_linear.value();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_db_leaves_buffer_unchanged() {
        let mut gain = Gain {
            db: Decibels::new(0.0),
        };
        let original = vec![0.5_f32, -0.3, 1.0, 0.0, -1.0];
        let mut buffer = original.clone();
        gain.process(&mut buffer);
        for (got, expected) in buffer.iter().zip(original.iter()) {
            assert!(
                (got - expected).abs() < 1e-6,
                "expected {expected}, got {got}"
            );
        }
    }

    /// +6 dB roughly doubles amplitude; verifies upward scaling (≈ 1.995 within 1e-3).
    #[test]
    fn positive_db_scales_up() {
        let mut gain = Gain {
            db: Decibels::new(6.0),
        };
        let mut buf = [1.0_f32];
        gain.process(&mut buf);
        assert!((buf[0] - 1.995).abs() < 1e-3, "got {}", buf[0]);
    }

    /// -6 dB roughly halves amplitude; verifies downward scaling (≈ 0.501 within 1e-3).
    #[test]
    fn negative_db_scales_down() {
        let mut gain = Gain {
            db: Decibels::new(-6.0),
        };
        let mut buf = [1.0_f32];
        gain.process(&mut buf);
        assert!((buf[0] - 0.501).abs() < 1e-3, "got {}", buf[0]);
    }

    /// A near-zero dB value (0.001 dB) leaves the sample within 2e-4 of unity.
    #[test]
    fn very_small_db_near_unity() {
        let mut gain = Gain {
            db: Decibels::new(0.001),
        };
        let mut buf = [1.0_f32];
        gain.process(&mut buf);
        assert!((buf[0] - 1.0_f32).abs() < 2e-4, "got {}", buf[0]);
    }

    /// +120 dB yields ~1e6 amplitude; documents that no clipping occurs at extreme gain.
    #[test]
    fn large_positive_db_no_clip() {
        let mut gain = Gain {
            db: Decibels::new(120.0),
        };
        let mut buf = [1.0_f32];
        gain.process(&mut buf);
        assert!((buf[0] - 1e6_f32).abs() < 1e3, "got {}", buf[0]);
    }

    /// Negative-infinity dB is silence; all samples must be exactly 0.0.
    #[test]
    fn neg_infinity_db_silences_buffer() {
        let mut gain = Gain {
            db: Decibels::new(f32::NEG_INFINITY),
        };
        let mut buf = [1.0_f32, -0.5, 0.75];
        gain.process(&mut buf);
        for s in &buf {
            assert_eq!(*s, 0.0, "expected 0.0, got {s}");
        }
    }

    /// NaN dB propagates NaN through every sample (documents IEEE 754 propagation).
    #[test]
    fn nan_db_propagates_nan() {
        let mut gain = Gain {
            db: Decibels::new(f32::NAN),
        };
        let mut buf = [1.0_f32];
        gain.process(&mut buf);
        assert!(buf[0].is_nan(), "expected NaN, got {}", buf[0]);
    }

    /// Processing an empty slice must not panic (boundary: zero-length buffer).
    #[test]
    fn empty_buffer_does_not_panic() {
        let mut gain = Gain {
            db: Decibels::new(6.0),
        };
        gain.process(&mut []);
    }

    /// A single-element buffer scaled by +6 dB yields ≈ 3.99 within 1e-3.
    #[test]
    fn single_sample_buffer() {
        let mut gain = Gain {
            db: Decibels::new(6.0),
        };
        let mut buf = [2.0_f32];
        gain.process(&mut buf);
        assert!((buf[0] - 3.99).abs() < 1e-3, "got {}", buf[0]);
    }
}
