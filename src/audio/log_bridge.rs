//! Realtime-safe audio→log bridge (Phase 4).
//!
//! The audio thread must never call `tracing` directly (it allocates and may
//! block).  This module provides a bounded SPSC channel backed by `rtrb` and a
//! drain thread that forwards events to `tracing`.
//!
//! # Usage
//!
//! ```ignore
//! let (logger, _handle) = log_bridge::channel(1024);
//! // On the audio thread:
//! logger.log(AudioLogEvent::Xrun);
//! // The drain thread picks it up and calls tracing::warn!.
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

/// Fixed-shape event enum for the audio→log bridge.
///
/// Each variant carries only `Copy` types — no `String`, no `Vec` — so
/// `rtrb::Producer::push` never allocates on the audio thread.
#[derive(Clone, Copy, Debug)]
pub enum AudioLogEvent {
    /// An xrun (underflow) was detected.
    Xrun,
}

/// Write end of the audio→log channel.  Handed to the audio thread.
///
/// `log` takes `&mut self`; callers reach it through `Plugin::process`'s
/// `&mut self`, so the borrow checker enforces single-writer access statically.
/// No `UnsafeCell` or manual `unsafe impl Send` is needed.
pub struct AudioLogger {
    producer: rtrb::Producer<AudioLogEvent>,
    dropped: Arc<AtomicU64>,
}

impl AudioLogger {
    /// Push an event to the drain thread.  Non-blocking; if the ring buffer is
    /// full the event is counted in `dropped` and silently discarded.
    ///
    /// A2-safe: one non-blocking ring-buffer push; no alloc, no lock, no syscall.
    pub fn log(&mut self, event: AudioLogEvent) {
        if self.producer.push(event).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Handle to the drain thread.  Dropping this joins the thread.
///
/// Drop semantics rely on `AudioLogger` (the producer side) being dropped
/// before this handle.  When the producer drops, `Consumer::is_abandoned()`
/// returns `true`, and the drain loop exits after a final pass.  If this
/// handle were dropped first, `join()` would block forever waiting on a thread
/// that never sees `is_abandoned()` become true.  See the field-order comment
/// in `TonismPlugin`.
pub struct LogDrainHandle {
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for LogDrainHandle {
    fn drop(&mut self) {
        // The producer (AudioLogger) has already been dropped — Rust drops
        // struct fields in declaration order, and audio_logger is declared
        // before log_drain in TonismPlugin.  The drain thread will see
        // is_abandoned() == true and exit on its own; we just join it.
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

fn forward_event(event: AudioLogEvent) {
    match event {
        AudioLogEvent::Xrun => {
            tracing::warn!(target: "tonism::audio", "audio xrun detected");
        }
    }
}

/// Construct a matched `(AudioLogger, LogDrainHandle)` pair with the given
/// channel capacity (in event slots).
///
/// Spawns a drain thread that pulls events off the consumer and forwards each
/// one to `tracing`.  The drain thread sleeps for 10 ms when the queue is
/// empty, and exits cleanly when the producer is dropped (detected via
/// `Consumer::is_abandoned()`).
pub fn channel(capacity: usize) -> (AudioLogger, LogDrainHandle) {
    let (producer, mut consumer) = rtrb::RingBuffer::new(capacity);
    let dropped = Arc::new(AtomicU64::new(0));

    let dropped_clone = Arc::clone(&dropped);

    let thread = thread::Builder::new()
        .name("tonism-log-drain".into())
        .spawn(move || {
            loop {
                let mut drained = 0usize;
                while let Ok(event) = consumer.pop() {
                    forward_event(event);
                    drained += 1;
                }

                // Report any dropped events since last drain.
                let drops = dropped_clone.swap(0, Ordering::Relaxed);
                if drops > 0 {
                    tracing::warn!(
                        target: "tonism::audio",
                        dropped = drops,
                        "log bridge dropped events (queue was full)"
                    );
                }

                // Exit when producer is dropped AND queue is fully drained.
                // is_abandoned() returns true after the producer drops; pop()
                // continues to return any remaining items before returning Empty.
                if consumer.is_abandoned() {
                    // Final drain: items pushed just before the producer dropped
                    // may still be in the queue.
                    while let Ok(event) = consumer.pop() {
                        forward_event(event);
                    }
                    break;
                }

                if drained == 0 {
                    thread::sleep(Duration::from_millis(10));
                }
            }
        })
        .expect("failed to spawn log drain thread");

    let logger = AudioLogger { producer, dropped };
    let handle = LogDrainHandle {
        thread: Some(thread),
    };

    (logger, handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_thread_exits_cleanly_after_one_xrun() {
        let (mut logger, handle) = channel(64);

        // Push one Xrun from the test thread (simulates audio thread usage).
        logger.log(AudioLogEvent::Xrun);

        // Drop logger first — its producer drop signals is_abandoned() to the
        // drain thread.  Then drop the handle, which joins the thread.
        drop(logger);
        drop(handle);

        // If we reach here without hanging the thread exited cleanly.
    }
}
