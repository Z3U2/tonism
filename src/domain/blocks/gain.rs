use crate::domain::process::Process;
use crate::domain::types::{Decibels, GainLinear, SampleRate};

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
            *s *= gain_linear.0;
        }
    }

    /// No state to reset for a static gain block.
    fn reset(&mut self, _sr: SampleRate) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_db_leaves_buffer_unchanged() {
        let mut gain = Gain { db: Decibels(0.0) };
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
}
