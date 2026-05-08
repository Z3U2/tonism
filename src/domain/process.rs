use crate::domain::types::SampleRate;

/// Core processing contract for all DSP blocks.
///
/// Lifecycle:
/// 1. `prepare(sr, max_block_size)` is called once per audio session, before
///    any `process()` call.  Implementations may allocate here (e.g. delay
///    lines sized from `max_block_size`, coefficient arrays sized from `sr`).
///    nih-plug guarantees that after `Plugin::initialize` it calls
///    `Plugin::reset`, mirroring this lifecycle.
/// 2. `reset()` may be called multiple times mid-session to clear internal
///    state (zero delay lines, snap smoothers to target, reset oscillator
///    phase).  **Must not allocate** — `reset()` may be called from the
///    audio thread.
/// 3. `process(buffer)` is called repeatedly during the session.  Mutates
///    the buffer in place per A2 (no alloc, no lock, no syscall).
///
/// Implementations are free to override only the methods they need;
/// `prepare` and `reset` have no-op default impls for stateless blocks.
pub trait Process {
    /// Configure the block for the upcoming session.
    /// Default: no-op.  Stateful blocks override to allocate / cache state.
    fn prepare(&mut self, _sr: SampleRate, _max_block_size: usize) {}

    /// Clear all internal state.  Must not allocate.
    /// Default: no-op.  Stateful blocks override to reset filters, smoothers, etc.
    fn reset(&mut self) {}

    /// Process one buffer in place.
    fn process(&mut self, buffer: &mut [f32]);
}
