//! Adapter trait for audio backends.
//!
//! `CpalBackend` is a dormant marker struct; the live cpal audio loop is owned by
//! `src/cpal_direct.rs` (the C8 composition root) and does not go through this trait.
//! Under `--features plugin-export`, nih-plug's standalone wrapper takes over instead.
//! `BufferBackend` is an in-memory fake used by integration tests — it feeds a
//! pre-recorded input vec through a `Process` impl and captures the output.

use crate::domain::process::Process;
use crate::domain::types::SampleRate;

/// Drive a `Process` implementation through one or more buffers and surface
/// any underrun events.  The in-memory fake feeds pre-recorded vectors through
/// this trait; the live cpal path bypasses it and is wired in `cpal_direct`.
pub trait AudioBackend {
    /// Run the backend until input is exhausted or the implementation decides
    /// to stop.  Blocking call; returns when processing is complete.
    ///
    /// Implementations must call `processor.prepare(...)` then
    /// `processor.reset()` before any `process()` call, mirroring nih-plug's
    /// `initialize → reset → process` lifecycle.
    fn run(&mut self, processor: &mut dyn Process, sample_rate: SampleRate);
}

/// Dormant marker for a future cpal-backed `AudioBackend` impl.
///
/// **NOTE**: the default standalone path does NOT use this struct.  The real audio loop
/// is built and owned by `cpal_direct::build_streams()` (`src/cpal_direct.rs`), which
/// wires cpal input/output callbacks directly and never calls `AudioBackend::run`.
/// Under `--features plugin-export` (dormant VST3/CLAP target), nih-plug's
/// `nih_export_standalone!` owns the loop instead.  This struct exists as a placeholder
/// for future non-interactive backends (offline render, file-based capture).
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
