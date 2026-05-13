//! Lock-free round-trip latency meter — see
//! docs/specs/mvp/stories/mvp-02-latency-readout-in-standalone-implementation-plan.md

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

use crate::domain::process::Process;
use crate::domain::types::SampleRate;

/// Number of samples captured per measurement window.
/// Covers > 90 ms at 44.1 kHz — comfortable headroom over the < 10 ms dev target.
pub const CAPTURE_LEN: usize = 4096;

/// 1024-sample Kronecker reference used for GUI-side cross-correlation.
/// Sample 0 is 1.0; the rest are zero.  Mirrors the test fixture in mvp-01.
pub const KRONECKER_REF: [f32; 1024] = {
    let mut a = [0.0_f32; 1024];
    a[0] = 1.0;
    a
};

/// State of the capture state machine.
///
/// `#[repr(u8)]` so the discriminant round-trips through `AtomicU8`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureState {
    Idle = 0,
    Capturing = 1,
    Done = 2,
    Cancelled = 3,
}

impl CaptureState {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => CaptureState::Idle,
            1 => CaptureState::Capturing,
            2 => CaptureState::Done,
            3 => CaptureState::Cancelled,
            _ => {
                debug_assert!(false, "unknown CaptureState discriminant: {v}");
                CaptureState::Idle
            }
        }
    }
}

// ---------------------------------------------------------------------------
// LatencyMeter
// ---------------------------------------------------------------------------

/// Audio-shell `Process` block that captures a loopback signal and emits a
/// Kronecker impulse to bootstrap round-trip latency measurement.
///
/// ## Realtime safety invariants (`process()`)
///
/// - No allocation: capture buffer is `Arc<[AtomicU32; CAPTURE_LEN]>` allocated
///   once in `Default::default()`.
/// - No locking: only `AtomicU8`, `AtomicU32`, `AtomicBool` operations.
/// - No syscall / filesystem / logging on the per-buffer path.
/// - `write_idx` and `emit_impulse_pending` are audio-thread-local scratch;
///   they are never shared.
pub struct LatencyMeter {
    capture_buffer: Arc<[AtomicU32; CAPTURE_LEN]>,
    state: Arc<AtomicU8>,
    arm_request: Arc<AtomicBool>,
    /// Audio-thread-local write cursor into `capture_buffer`.
    write_idx: usize,
    /// Audio-thread-local flag: true while the single-sample impulse has not
    /// yet been emitted for the current capture window.
    emit_impulse_pending: bool,
}

impl Default for LatencyMeter {
    fn default() -> Self {
        Self {
            capture_buffer: Arc::new(std::array::from_fn(|_| AtomicU32::new(0))),
            state: Arc::new(AtomicU8::new(CaptureState::Idle as u8)),
            arm_request: Arc::new(AtomicBool::new(false)),
            write_idx: 0,
            emit_impulse_pending: false,
        }
    }
}

impl Process for LatencyMeter {
    /// No-op: capture buffer is fixed-size, allocated in `Default`.
    fn prepare(&mut self, _sr: SampleRate, _max_block_size: usize) {}

    fn reset(&mut self) {
        self.write_idx = 0;
        self.emit_impulse_pending = false;
        self.state
            .store(CaptureState::Idle as u8, Ordering::Release);
        // Note: `arm_request` is intentionally NOT cleared here.  It is a
        // GUI→audio signal; if the user clicks "Measure latency" before the
        // audio session resets (e.g. transport restart), the pending arm is
        // preserved and takes effect on the first `process()` call after reset.
    }

    /// Per-buffer capture + impulse-emission.  A2-safe: no alloc, no lock,
    /// no syscall.
    ///
    /// State machine:
    /// 1. If `arm_request` was true AND state == Idle: transition → Capturing,
    ///    reset `write_idx`, set `emit_impulse_pending`.
    /// 2. While not Capturing: leave the buffer untouched.
    /// 3. While Capturing: store the inbound sample (before overwriting), then
    ///    overwrite with the impulse on the very first sample of the window
    ///    (`emit_impulse_pending` cleared immediately after that one sample).
    ///    Transition → Done once `CAPTURE_LEN` samples have been stored.
    fn process(&mut self, buffer: &mut [f32]) {
        // Step 1: consume the arm request.  swap(false) is atomic with AcqRel
        // so we see the GUI's store(true, Release) before this read.
        let armed = self.arm_request.swap(false, Ordering::AcqRel);
        if armed && self.state.load(Ordering::Acquire) == CaptureState::Idle as u8 {
            self.write_idx = 0;
            self.emit_impulse_pending = true;
            self.state
                .store(CaptureState::Capturing as u8, Ordering::Release);
        }

        // Step 2: fast-path exit when not Capturing.
        if self.state.load(Ordering::Acquire) != CaptureState::Capturing as u8 {
            return;
        }

        // Step 3: per-sample capture + optional impulse overwrite.
        for sample in buffer.iter_mut() {
            if self.write_idx >= CAPTURE_LEN {
                break;
            }

            // Capture the inbound value BEFORE any overwrite.
            self.capture_buffer[self.write_idx].store(sample.to_bits(), Ordering::Release);

            // Emit the Kronecker impulse on the very first sample of the window
            // only (one sample wide).
            if self.emit_impulse_pending {
                *sample = if self.write_idx == 0 { 1.0 } else { 0.0 };
                if self.write_idx == 0 {
                    // Impulse is exactly one sample — clear flag immediately so
                    // subsequent samples in this buffer are NOT overwritten.
                    self.emit_impulse_pending = false;
                }
            }

            self.write_idx += 1;

            if self.write_idx == CAPTURE_LEN {
                self.state
                    .store(CaptureState::Done as u8, Ordering::Release);
                break;
            }
        }
    }
}

impl LatencyMeter {
    /// Audio-thread cancel.  No-op unless state == Capturing.
    ///
    /// Called by `TonismPlugin` when bypass flips on during a measurement so the
    /// GUI receives a `Cancelled` sentinel instead of a partial result.
    pub fn cancel(&mut self) {
        if self.state.load(Ordering::Acquire) == CaptureState::Capturing as u8 {
            self.state
                .store(CaptureState::Cancelled as u8, Ordering::Release);
        }
    }

    /// Clone the shared Arcs into a `LatencyHandle` the GUI can hold.
    pub fn handle(&self) -> LatencyHandle {
        LatencyHandle {
            capture_buffer: self.capture_buffer.clone(),
            state: self.state.clone(),
            arm_request: self.arm_request.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// LatencyHandle
// ---------------------------------------------------------------------------

/// GUI-side facet of `LatencyMeter`.  Cheap to clone (three `Arc` clones).
///
/// All methods are safe to call from any thread; operations on the capture
/// buffer are only meaningful when `state() == Done`.
#[derive(Clone)]
pub struct LatencyHandle {
    capture_buffer: Arc<[AtomicU32; CAPTURE_LEN]>,
    state: Arc<AtomicU8>,
    arm_request: Arc<AtomicBool>,
}

impl LatencyHandle {
    /// GUI button `on_press` handler.
    ///
    /// Sets the arm flag.  The audio thread consumes it on its next `process()`
    /// call.  A second call while already Capturing is silently ignored by the
    /// audio thread (`arm_request` transitions Idle → Capturing only).
    pub fn request_measurement(&self) {
        self.arm_request.store(true, Ordering::Release);
    }

    /// 60 Hz GUI poll — returns the current capture state.
    pub fn state(&self) -> CaptureState {
        CaptureState::from_u8(self.state.load(Ordering::Acquire))
    }

    /// Copy the capture buffer's atomic slots into `out`.
    ///
    /// `out` is cleared then extended to `CAPTURE_LEN` elements.  The caller
    /// should pre-reserve `CAPTURE_LEN` capacity to avoid reallocation.
    ///
    /// Safe to call at any time, but values are only meaningful once
    /// `state() == Done`.
    pub fn read_capture_into(&self, out: &mut Vec<f32>) {
        out.clear();
        for slot in self.capture_buffer.iter() {
            out.push(f32::from_bits(slot.load(Ordering::Acquire)));
        }
    }

    /// After consuming a `Done` or `Cancelled` result, transition back to
    /// `Idle` so the user can trigger another measurement.
    ///
    /// Implemented with `compare_exchange` so it is a no-op while `Capturing`
    /// (refuses to clobber an in-progress measurement).
    pub fn reset_to_idle(&self) {
        let _ = self.state.compare_exchange(
            CaptureState::Done as u8,
            CaptureState::Idle as u8,
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
        let _ = self.state.compare_exchange(
            CaptureState::Cancelled as u8,
            CaptureState::Idle as u8,
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a fresh meter + handle pair.
    fn make() -> (LatencyMeter, LatencyHandle) {
        let m = LatencyMeter::default();
        let h = m.handle();
        (m, h)
    }

    // Helper: drive the meter with `n` calls of a buffer filled with `value`.
    fn drive_n(meter: &mut LatencyMeter, buf_size: usize, value: f32, n: usize) {
        let mut buf = vec![value; buf_size];
        for _ in 0..n {
            meter.process(&mut buf);
        }
    }

    #[test]
    fn meter_idle_with_no_arm_passes_buffer_unchanged() {
        let (mut meter, _h) = make();
        let mut buf = vec![0.5_f32; 64];
        meter.process(&mut buf);
        assert!(buf.iter().all(|&s| (s - 0.5).abs() < 1e-9));
        assert_eq!(meter.write_idx, 0);
        assert_eq!(
            CaptureState::from_u8(meter.state.load(Ordering::Acquire)),
            CaptureState::Idle
        );
    }

    #[test]
    fn meter_arm_transitions_idle_to_capturing() {
        let (mut meter, handle) = make();
        handle.request_measurement();
        let mut buf = vec![0.5_f32; 64];
        meter.process(&mut buf);
        // 64 < CAPTURE_LEN so state should still be Capturing.
        assert_eq!(handle.state(), CaptureState::Capturing);
        // arm_request must have been consumed (swapped to false).
        assert!(!meter.arm_request.load(Ordering::Acquire));
    }

    #[test]
    fn meter_emits_impulse_on_first_frame_of_capture_window() {
        let (mut meter, handle) = make();
        handle.request_measurement();
        let mut buf = vec![0.5_f32; 64];
        meter.process(&mut buf);
        // Only the very first sample should be overwritten with 1.0.
        assert!(
            (buf[0] - 1.0).abs() < 1e-9,
            "buf[0] should be impulse 1.0, got {}",
            buf[0]
        );
        // All subsequent samples should be untouched (0.5).
        for (i, &s) in buf[1..].iter().enumerate() {
            assert!(
                (s - 0.5).abs() < 1e-9,
                "buf[{i}] should be unchanged 0.5, got {s}"
            );
        }
    }

    #[test]
    fn meter_captures_input_before_overwriting() {
        let (mut meter, handle) = make();
        handle.request_measurement();
        let mut buf = vec![0.5_f32; 64];
        meter.process(&mut buf);
        // The capture buffer stores the ORIGINAL input, not the overwritten 1.0.
        let captured_slot_0 = f32::from_bits(meter.capture_buffer[0].load(Ordering::Acquire));
        assert!(
            (captured_slot_0 - 0.5).abs() < 1e-9,
            "capture[0] should be original 0.5, got {captured_slot_0}"
        );
        let captured_slot_1 = f32::from_bits(meter.capture_buffer[1].load(Ordering::Acquire));
        assert!(
            (captured_slot_1 - 0.5).abs() < 1e-9,
            "capture[1] should be 0.5, got {captured_slot_1}"
        );
    }

    #[test]
    fn meter_completes_after_capture_len_samples() {
        let (mut meter, handle) = make();
        handle.request_measurement();
        // 4096 / 256 = 16 calls.
        let mut buf = vec![0.0_f32; 256];
        for _ in 0..16 {
            meter.process(&mut buf);
        }
        assert_eq!(handle.state(), CaptureState::Done);
    }

    #[test]
    fn meter_arm_while_capturing_is_ignored() {
        let (mut meter, handle) = make();
        handle.request_measurement();
        // First partial process — starts capturing.
        let mut buf = vec![0.0_f32; 64];
        meter.process(&mut buf);
        assert_eq!(handle.state(), CaptureState::Capturing);
        let idx_after_first = meter.write_idx;

        // Second arm attempt during Capturing — should be ignored.
        handle.request_measurement();
        meter.process(&mut buf);

        // write_idx should have advanced naturally, not reset to 0.
        assert_eq!(meter.write_idx, idx_after_first + 64);
        assert_eq!(handle.state(), CaptureState::Capturing);
    }

    #[test]
    fn meter_cancel_during_capture_transitions_to_cancelled() {
        let (mut meter, handle) = make();
        handle.request_measurement();
        // Partial process.
        drive_n(&mut meter, 64, 0.0, 1);
        assert_eq!(handle.state(), CaptureState::Capturing);

        meter.cancel();
        assert_eq!(handle.state(), CaptureState::Cancelled);
    }

    #[test]
    fn handle_request_measurement_sets_arm_request() {
        let (meter, handle) = make();
        assert!(!meter.arm_request.load(Ordering::Acquire));
        handle.request_measurement();
        assert!(meter.arm_request.load(Ordering::Acquire));
    }

    #[test]
    fn handle_read_capture_into_copies_full_buffer_when_done() {
        let (mut meter, handle) = make();
        handle.request_measurement();

        // Drive with a ramp pattern so we can check a midpoint value.
        let step = 1.0_f32 / CAPTURE_LEN as f32;
        let mut total_written = 0usize;
        let buf_size = 256usize;
        while total_written < CAPTURE_LEN {
            let end = (total_written + buf_size).min(CAPTURE_LEN);
            let mut buf: Vec<f32> = (total_written..end).map(|i| i as f32 * step).collect();
            // Pad to buf_size if the last chunk is shorter.
            buf.resize(buf_size, 0.0);
            meter.process(&mut buf);
            total_written += buf_size;
        }
        assert_eq!(handle.state(), CaptureState::Done);

        let mut captured = Vec::with_capacity(CAPTURE_LEN);
        handle.read_capture_into(&mut captured);
        assert_eq!(captured.len(), CAPTURE_LEN);

        // Midpoint: index CAPTURE_LEN/2 should equal (CAPTURE_LEN/2) * step.
        let mid = CAPTURE_LEN / 2;
        let expected = mid as f32 * step;
        assert!(
            (captured[mid] - expected).abs() < 1e-6,
            "captured[{mid}] = {}, expected {expected}",
            captured[mid]
        );
    }

    #[test]
    fn handle_reset_to_idle_only_transitions_from_done_or_cancelled() {
        // Sub-case 1: from Idle — no-op, stays Idle.
        {
            let (_, handle) = make();
            assert_eq!(handle.state(), CaptureState::Idle);
            handle.reset_to_idle();
            assert_eq!(handle.state(), CaptureState::Idle);
        }

        // Sub-case 2: from Capturing — CAS no-op, stays Capturing.
        {
            let (mut meter, handle) = make();
            handle.request_measurement();
            meter.process(&mut vec![0.0_f32; 64]);
            assert_eq!(handle.state(), CaptureState::Capturing);
            handle.reset_to_idle();
            assert_eq!(handle.state(), CaptureState::Capturing);
        }

        // Sub-case 3: from Done — transitions to Idle.
        {
            let (mut meter, handle) = make();
            handle.request_measurement();
            let mut buf = vec![0.0_f32; 256];
            for _ in 0..16 {
                meter.process(&mut buf);
            }
            assert_eq!(handle.state(), CaptureState::Done);
            handle.reset_to_idle();
            assert_eq!(handle.state(), CaptureState::Idle);
        }

        // Sub-case 4: from Cancelled — transitions to Idle.
        {
            let (mut meter, handle) = make();
            handle.request_measurement();
            meter.process(&mut vec![0.0_f32; 64]);
            meter.cancel();
            assert_eq!(handle.state(), CaptureState::Cancelled);
            handle.reset_to_idle();
            assert_eq!(handle.state(), CaptureState::Idle);
        }
    }

    /// Smoke-level alloc-free steady-state test.
    ///
    /// The `debug-assert-no-alloc` feature wires the nih-plug assert-no-alloc
    /// shim through `Plugin::process`; it is not applicable to arbitrary code
    /// paths.  What this test verifies instead is that the `process()` call
    /// sequence compiles and runs without incident, and that the no-alloc
    /// invariants documented on `LatencyMeter::process` hold by construction:
    ///
    /// - Capture buffer: allocated once in `Default::default()`.
    /// - No `Vec::push`, `Box::new`, or other heap allocation in `process()`.
    /// - No `Mutex` / `RwLock` — only atomics.
    /// - No syscall / logging inside the per-buffer path.
    #[test]
    fn meter_alloc_free_steady_state() {
        let (mut meter, handle) = make();
        handle.request_measurement();
        // Run through a full capture window to exercise all branches.
        let mut buf = vec![0.0_f32; 256];
        for _ in 0..16 {
            meter.process(&mut buf);
        }
        assert_eq!(handle.state(), CaptureState::Done);
    }
}
