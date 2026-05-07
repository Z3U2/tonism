use crate::domain::types::SampleRate;

/// Core processing contract for all DSP blocks.
///
/// Implementations must be free of allocation, blocking, or I/O
/// so they are safe to call from a hard-realtime audio thread.
pub trait Process {
    /// Apply in-place processing to a mono interleaved buffer of samples.
    fn process(&mut self, buffer: &mut [f32]);

    /// Reinitialise any internal state (filters, envelopes, phases) for the
    /// given sample rate.  Called after initialisation and on every reset.
    fn reset(&mut self, sr: SampleRate);
}
