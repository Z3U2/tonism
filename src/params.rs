//! Lock-free parameter system for the cpal-direct standalone path (C3).
//!
//! Replaces `nih_plug::params` for the standalone target. The GUI thread
//! sets target values via cheap atomic stores; the audio thread reads
//! the latest target each frame and walks toward it via a per-param
//! [`LinearSmoother`]. No locks. No allocations on the audio thread.
//!
//! # Split shape
//!
//! Each parameter exposes two halves:
//!
//! - A [`FloatParamHandle`] / [`BoolParamHandle`] — cloneable, sent to
//!   the GUI thread. Holds the metadata + the shared atomic. `set()`
//!   updates the target; `target()` reads it back.
//! - A [`SmoothedFloatParam`] / [`BoolParamReader`] — owned by the
//!   audio thread. Reads the same atomic each frame; the float variant
//!   advances its private smoother per [`SmoothedFloatParam::next`].
//!
//! Both halves point at the same `Arc<Atomic*>`, so the storage
//! survives stream restart (C3 persistence guarantee): tear down the
//! audio thread + smoother, build a new one from the surviving
//! handle, the target value carries over.
//!
//! # Persistence vs lifecycle
//!
//! The atomic storage is the persistent state. The smoother is per-
//! session: it gets a fresh [`SmoothedFloatParam`] constructed off the
//! handle when a new audio stream comes up. This matches F1 — one
//! authoritative copy (the atomic) + derived state (the smoother)
//! that recomputes from the source.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::domain::smoother::LinearSmoother;
use crate::domain::types::SampleRate;

// ----------------------------------------------------------------------
// FloatParam
// ----------------------------------------------------------------------

/// Static metadata for a float parameter. Allocated once at param
/// construction, shared by handle + audio-side reader via `Arc`.
#[derive(Debug)]
pub struct FloatParamMetadata {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    /// Smoothing time constant in seconds. Audio-side
    /// [`SmoothedFloatParam`] uses this to build its smoother.
    pub smoothing_time_secs: f32,
}

/// GUI-side handle to a float parameter. Cheaply cloneable. `set()`
/// stores a clamped target into the shared atomic; `target()` reads
/// the most recent value back.
#[derive(Clone)]
pub struct FloatParamHandle {
    storage: Arc<AtomicU32>,
    metadata: Arc<FloatParamMetadata>,
}

impl FloatParamHandle {
    /// Store a new target. Value is clamped into the metadata range.
    /// Lock-free; safe to call from any thread.
    pub fn set(&self, value: f32) {
        let clamped = value.clamp(self.metadata.min, self.metadata.max);
        self.storage.store(clamped.to_bits(), Ordering::Relaxed);
    }

    /// Read the most recent target. Returns the raw stored value
    /// (already clamped on store).
    pub fn target(&self) -> f32 {
        f32::from_bits(self.storage.load(Ordering::Relaxed))
    }

    /// Access to the static metadata for GUI rendering.
    pub fn metadata(&self) -> &FloatParamMetadata {
        &self.metadata
    }

    /// Build a fresh [`SmoothedFloatParam`] attached to the same
    /// storage. Used on audio-stream (re)construction so the smoother
    /// is rebuilt against the surviving target value (C3 persistence).
    pub fn build_smoothed(&self) -> SmoothedFloatParam {
        let initial = self.target();
        SmoothedFloatParam {
            storage: self.storage.clone(),
            smoother: LinearSmoother::new(initial, self.metadata.smoothing_time_secs),
        }
    }
}

/// Audio-side reader for a float parameter. Owns a private
/// [`LinearSmoother`]; reads the shared atomic each frame to pick up
/// GUI-side changes.
///
/// Not Clone — the smoother state is per-stream.
pub struct SmoothedFloatParam {
    storage: Arc<AtomicU32>,
    smoother: LinearSmoother,
}

impl SmoothedFloatParam {
    /// Configure the smoother for the session sample rate. Must be
    /// called before [`Self::next`] on the audio thread.
    pub fn prepare(&mut self, sr: SampleRate) {
        self.smoother.prepare(sr);
    }

    /// Advance one frame; returns the current smoothed value.
    /// A2-clean: one relaxed atomic load + a smoother step.
    ///
    /// Name shadows [`Iterator::next`] intentionally — see
    /// [`LinearSmoother::next`] for the rationale.
    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn next(&mut self) -> f32 {
        let target = f32::from_bits(self.storage.load(Ordering::Relaxed));
        self.smoother.set_target(target);
        self.smoother.next()
    }

    /// Snap the smoother to its target. Useful for the first frame of
    /// a new stream so callers don't have to wait out a full ramp from
    /// the previous session's smoother state.
    pub fn snap_to_target(&mut self) {
        let target = f32::from_bits(self.storage.load(Ordering::Relaxed));
        self.smoother.set_target(target);
        self.smoother.snap_to_target();
    }
}

/// Construct a paired (handle, smoothed) float parameter. Both halves
/// share the same backing `Arc<AtomicU32>` so changes via the handle
/// flow to the audio thread on the next frame.
pub fn float_param(
    name: &'static str,
    default: f32,
    min: f32,
    max: f32,
    unit: &'static str,
    smoothing_time_secs: f32,
) -> (FloatParamHandle, SmoothedFloatParam) {
    let storage = Arc::new(AtomicU32::new(default.to_bits()));
    let metadata = Arc::new(FloatParamMetadata {
        name,
        min,
        max,
        default,
        unit,
        smoothing_time_secs,
    });
    let handle = FloatParamHandle {
        storage: storage.clone(),
        metadata,
    };
    let smoothed = SmoothedFloatParam {
        storage,
        smoother: LinearSmoother::new(default, smoothing_time_secs),
    };
    (handle, smoothed)
}

// ----------------------------------------------------------------------
// BoolParam — no smoothing; a bool transition is one atomic flip.
// ----------------------------------------------------------------------

/// GUI- and audio-side handle for a bool parameter. Cloneable. There
/// is no smoothed variant — a bool transition is one cycle's worth of
/// state change; smoothing would not be audible-meaningful.
#[derive(Clone)]
pub struct BoolParam {
    storage: Arc<AtomicBool>,
    name: &'static str,
}

impl BoolParam {
    pub fn new(name: &'static str, default: bool) -> Self {
        Self {
            storage: Arc::new(AtomicBool::new(default)),
            name,
        }
    }

    pub fn set(&self, value: bool) {
        self.storage.store(value, Ordering::Relaxed);
    }

    pub fn value(&self) -> bool {
        self.storage.load(Ordering::Relaxed)
    }

    pub fn name(&self) -> &'static str {
        self.name
    }
}

// ----------------------------------------------------------------------
// TonismParams — the registry of all user-visible parameters.
// ----------------------------------------------------------------------

/// All Tonism parameters in their GUI/control-thread shape. Build the
/// audio-side smoothed reads via [`TonismParams::build_audio_side`].
pub struct TonismParams {
    pub input_gain: FloatParamHandle,
    pub output_gain: FloatParamHandle,
    pub bypass: BoolParam,
    pub test_signal: BoolParam,
}

/// Audio-thread reads of all parameters. Smoothed float params,
/// direct-read bool params.
pub struct TonismParamsAudio {
    pub input_gain: SmoothedFloatParam,
    pub output_gain: SmoothedFloatParam,
    pub bypass: BoolParam,
    pub test_signal: BoolParam,
}

impl TonismParams {
    /// Production smoothing time for the input/output gain trims, in
    /// seconds. Matches `SmoothingStyle::Linear(20.0)` from the
    /// nih-plug surface this replaces — fast enough to be inaudible
    /// as a "fade", slow enough to be click-free under a real param
    /// twist. The `--ramp` test in `src/cpal_direct.rs` overrides this
    /// with a longer time so the smoother's curve is audibly perceptible.
    pub const PRODUCTION_SMOOTHING_SECS: f32 = 0.020;

    /// Build the Tonism param registry with `smoothing_time_secs`
    /// applied to both float trims. Pass [`Self::PRODUCTION_SMOOTHING_SECS`]
    /// for normal use.
    ///
    /// Returns the GUI-side handles + the audio-side readers as a
    /// pair. The handles can be cloned freely; the audio-side struct
    /// is moved into the cpal callback.
    pub fn new(smoothing_time_secs: f32) -> (Self, TonismParamsAudio) {
        let (in_handle, in_smoothed) =
            float_param("Input Gain", 0.0, -60.0, 12.0, "dB", smoothing_time_secs);
        let (out_handle, out_smoothed) =
            float_param("Output Gain", 0.0, -60.0, 12.0, "dB", smoothing_time_secs);
        let bypass = BoolParam::new("Bypass", false);
        let test_signal = BoolParam::new("Test Signal", false);
        let gui = Self {
            input_gain: in_handle,
            output_gain: out_handle,
            bypass: bypass.clone(),
            test_signal: test_signal.clone(),
        };
        let audio = TonismParamsAudio {
            input_gain: in_smoothed,
            output_gain: out_smoothed,
            bypass,
            test_signal,
        };
        (gui, audio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_param_default_round_trips_through_handle() {
        let (handle, mut smoothed) = float_param("test", 3.0, -10.0, 10.0, "", 0.020);
        smoothed.prepare(SampleRate::new(48_000.0));
        assert_eq!(handle.target(), 3.0);
        smoothed.snap_to_target();
        assert_eq!(smoothed.next(), 3.0);
    }

    #[test]
    fn float_param_handle_set_reaches_audio_after_ramp() {
        // 10 ms smoothing at 1 kHz → 10 sample ramp from default to new
        // target.
        let (handle, mut smoothed) = float_param("test", 0.0, -1.0, 1.0, "", 0.010);
        smoothed.prepare(SampleRate::new(1_000.0));
        smoothed.snap_to_target();

        handle.set(1.0);
        for _ in 0..9 {
            smoothed.next();
        }
        assert_eq!(smoothed.next(), 1.0);
    }

    #[test]
    fn float_param_handle_clamps_to_range() {
        let (handle, _smoothed) = float_param("test", 0.0, -1.0, 1.0, "", 0.020);
        handle.set(5.0);
        assert_eq!(handle.target(), 1.0);
        handle.set(-5.0);
        assert_eq!(handle.target(), -1.0);
    }

    #[test]
    fn float_param_rebuild_smoothed_preserves_target() {
        let (handle, _smoothed) = float_param("test", 0.0, -1.0, 1.0, "", 0.020);
        handle.set(0.7);
        // Simulate stream restart: drop the original smoothed, build a
        // fresh one from the handle. The new smoother sees the latest
        // target on its first frame.
        let mut fresh = handle.build_smoothed();
        fresh.prepare(SampleRate::new(48_000.0));
        fresh.snap_to_target();
        assert_eq!(fresh.next(), 0.7);
    }

    #[test]
    fn bool_param_round_trips_across_handle_clones() {
        let a = BoolParam::new("test", false);
        let b = a.clone();
        a.set(true);
        assert!(b.value());
        b.set(false);
        assert!(!a.value());
    }

    #[test]
    fn tonism_params_pair_share_storage() {
        let (gui, mut audio) = TonismParams::new(TonismParams::PRODUCTION_SMOOTHING_SECS);
        audio.input_gain.prepare(SampleRate::new(48_000.0));
        audio.output_gain.prepare(SampleRate::new(48_000.0));

        // Defaults: 0 dB on the gain trims, bypass + test_signal off.
        assert_eq!(gui.input_gain.target(), 0.0);
        assert_eq!(gui.output_gain.target(), 0.0);
        assert!(!gui.bypass.value());
        assert!(!gui.test_signal.value());

        // GUI sets bypass → audio sees true.
        gui.bypass.set(true);
        assert!(audio.bypass.value());
    }
}
