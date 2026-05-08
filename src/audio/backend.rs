//! Adapter trait for audio backends.
//!
//! `CpalBackend` is the production impl driven by nih-plug's standalone wrapper.
//! `BufferBackend` is an in-memory fake used by integration tests — it feeds a
//! pre-recorded input vec through a `Process` impl and captures the output.

use crate::domain::process::Process;
use crate::domain::types::SampleRate;

/// Drive a `Process` implementation through one or more buffers and surface
/// any underrun events.  Production wraps cpal via nih-plug; the fake feeds
/// in-memory vectors.
pub trait AudioBackend {
    /// Run the backend until input is exhausted or the implementation decides
    /// to stop.  Blocking call; returns when processing is complete.
    ///
    /// Implementations must call `processor.prepare(...)` then
    /// `processor.reset()` before any `process()` call, mirroring nih-plug's
    /// `initialize → reset → process` lifecycle.
    fn run(&mut self, processor: &mut dyn Process, sample_rate: SampleRate);
}

/// Production backend driven by nih-plug's standalone wrapper.
///
/// **NOTE**: nih-plug owns the audio loop end-to-end via `nih_export_standalone`.
/// We don't drive it via `AudioBackend::run` — production lives in `audio::plugin::TonismPlugin`,
/// which nih-plug invokes directly.  This struct exists only as a marker so future
/// non-nih-plug backends (offline render, file-based capture) can plug into the same trait.
pub struct CpalBackend;

/// In-memory fake.  Constructed with an input vec and a buffer size; collects
/// the processed output for inspection.
pub struct BufferBackend {
    input: Vec<f32>,
    output: Vec<f32>,
    buffer_size: usize,
}

impl BufferBackend {
    pub fn new(input: Vec<f32>, buffer_size: usize) -> Self {
        Self {
            output: Vec::with_capacity(input.len()),
            input,
            buffer_size,
        }
    }

    /// Consume the backend, returning the processed output buffer.
    pub fn into_output(self) -> Vec<f32> {
        self.output
    }
}

impl AudioBackend for BufferBackend {
    fn run(&mut self, processor: &mut dyn Process, sample_rate: SampleRate) {
        processor.prepare(sample_rate, self.buffer_size);
        processor.reset();
        for chunk in self.input.chunks(self.buffer_size) {
            let mut work = chunk.to_vec();
            processor.process(&mut work);
            self.output.extend_from_slice(&work);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::blocks::gain::Gain;
    use crate::domain::types::Decibels;

    #[test]
    fn buffer_backend_runs_gain_block_through_input_in_chunks() {
        let input = vec![0.5_f32; 1024];
        let mut backend = BufferBackend::new(input.clone(), 128);
        let mut gain = Gain {
            db: Decibels::new(0.0),
        };
        backend.run(&mut gain, SampleRate::new(48_000.0));
        let out = backend.into_output();
        assert_eq!(out.len(), input.len());
        for (got, expected) in out.iter().zip(input.iter()) {
            assert!((got - expected).abs() < 1e-6);
        }
    }
}
