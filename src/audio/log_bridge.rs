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

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
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
/// Wraps an `rtrb::Producer` in a way that makes `log` safe to call as `&self`
/// from the audio thread.  The `UnsafeCell` is sound because only one thread
/// (the audio thread) ever calls `log()`.
pub struct AudioLogger {
    // SAFETY: only ever accessed from the single audio thread.
    producer: std::cell::UnsafeCell<rtrb::Producer<AudioLogEvent>>,
    dropped: Arc<AtomicU64>,
}

// The audio thread is the sole writer; no other thread holds the Producer.
unsafe impl Send for AudioLogger {}
// We never hand out &mut references across threads.
unsafe impl Sync for AudioLogger {}

impl AudioLogger {
    /// Push an event to the drain thread.  Non-blocking; if the ring buffer is
    /// full the event is counted in `dropped` and silently discarded.
    ///
    /// A2-safe: one non-blocking ring-buffer push; no alloc, no lock, no syscall.
    pub fn log(&self, event: AudioLogEvent) {
        // SAFETY: only the audio thread calls this.
        let producer = unsafe { &mut *self.producer.get() };
        if producer.push(event).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Handle to the drain thread.  Dropping this signals the thread to exit and
/// joins it.
pub struct LogDrainHandle {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for LogDrainHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            // Best-effort join; if the thread panicked just move on.
            let _ = handle.join();
        }
    }
}

/// Construct a matched `(AudioLogger, LogDrainHandle)` pair with the given
/// channel capacity (in event slots).
///
/// Spawns a drain thread that pulls events off the consumer and forwards each
/// one to `tracing`.  The drain thread sleeps for 10 ms when the queue is
/// empty.
pub fn channel(capacity: usize) -> (AudioLogger, LogDrainHandle) {
    let (producer, mut consumer) = rtrb::RingBuffer::new(capacity);
    let dropped = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));

    let dropped_clone = Arc::clone(&dropped);
    let stop_clone = Arc::clone(&stop);

    let thread = thread::Builder::new()
        .name("tonism-log-drain".into())
        .spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                // Drain all queued events.
                let mut drained = 0usize;
                while let Ok(event) = consumer.pop() {
                    match event {
                        AudioLogEvent::Xrun => {
                            tracing::warn!(target: "tonism::audio", "audio xrun detected");
                        }
                    }
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

                if drained == 0 {
                    thread::sleep(Duration::from_millis(10));
                }
            }

            // Final drain before exit.
            while let Ok(event) = consumer.pop() {
                match event {
                    AudioLogEvent::Xrun => {
                        tracing::warn!(target: "tonism::audio", "audio xrun detected");
                    }
                }
            }
        })
        .expect("failed to spawn log drain thread");

    let logger = AudioLogger {
        producer: std::cell::UnsafeCell::new(producer),
        dropped,
    };

    let handle = LogDrainHandle {
        stop,
        thread: Some(thread),
    };

    (logger, handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_thread_exits_cleanly_after_one_xrun() {
        let (logger, handle) = channel(64);

        // Push one Xrun from the test thread (simulates audio thread usage).
        logger.log(AudioLogEvent::Xrun);

        // Drop the handle — signals the drain thread to exit and joins it.
        drop(handle);

        // If we reach here without hanging the thread exited cleanly.
    }
}
