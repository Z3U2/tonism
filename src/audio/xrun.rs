use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Counts audio thread xruns (underflows).  Shared between the audio thread
/// and the GUI via `Arc` so the editor can display the running total.
#[derive(Clone, Default)]
pub struct XrunCounter(pub Arc<AtomicU64>);

impl XrunCounter {
    /// Increment the counter by one.  Safe to call from a realtime thread.
    pub fn bump(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    /// Read the current counter value.
    pub fn read(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}
