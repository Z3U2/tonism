//! Skeleton for the realtime-safe audio→log bridge (Phase 4).
//!
//! The audio thread must never call `tracing` directly (it allocates and may
//! block).  Phase 4 replaces these stubs with a bounded SPSC channel
//! (backed by `rtrb`) and a drain thread that forwards messages to `tracing`.
#![allow(unused)]

/// Write end of the audio→log channel.  Handed to the audio thread.
pub struct AudioLogger;

/// Read end (drain handle).  Kept on a background thread.
pub struct LogDrainHandle;

/// Construct a matched `(AudioLogger, LogDrainHandle)` pair with the given
/// channel capacity (in log-message slots).
///
/// Phase 4 implementation: spins up an SPSC ring-buffer and a drain thread.
pub fn channel(capacity: usize) -> (AudioLogger, LogDrainHandle) {
    // TODO(phase-4): replace with rtrb SPSC + drain thread
    unimplemented!("log_bridge::channel is a Phase 4 deliverable")
}
