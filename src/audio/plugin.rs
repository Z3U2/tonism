use std::f32::consts::TAU;
use std::num::NonZeroU32;
use std::sync::Arc;

use nih_plug::prelude::{
    AsyncExecutor, AudioIOLayout, AuxiliaryBuffers, Buffer, BufferConfig, Editor, InitContext,
    Params, Plugin, PortNames, ProcessContext, ProcessStatus,
};

use crate::domain::blocks::gain::Gain;
use crate::domain::process::Process;
use crate::domain::types::{Decibels, GainLinear, SampleRate};

use super::latency::LatencyMeter;
use super::log_bridge::{self, AudioLogger, LogDrainHandle};
use super::params::TonismParams;
use super::xrun::XrunCounter;

/// Pre-`initialize()` placeholder sample rate. Always overwritten in
/// `Plugin::initialize()` before any `process()` call (nih-plug calls
/// `initialize` once before processing starts), so this value never
/// reaches the audio path. Pinned at 44.1 kHz purely as a defensive
/// default for the f32 phase-increment math during the brief window
/// between `Default::default()` and `initialize()`.
const DEFAULT_SAMPLE_RATE: f32 = 44_100.0;

/// The Tonism audio plugin struct.
pub struct TonismPlugin {
    params: Arc<TonismParams>,

    /// Domain gain block applied in the middle of the signal path.
    gain_block: Gain,

    /// Lock-free latency meter block.  Sits before `gain_block` on channel 0:
    /// captures the inbound loopback and emits a Kronecker impulse when armed.
    latency_meter: LatencyMeter,

    /// Phase accumulator for the 440 Hz test-signal sine generator.
    /// Advanced by `2π * 440 / sample_rate` each sample; wrapped with `% TAU`.
    phase: f32,

    /// Sample rate set in `initialize()`.  Needed for test-signal frequency.
    sample_rate: f32,

    /// Xrun counter shared with the GUI editor.
    /// Pre-cloned before processing starts to keep `process()` alloc-free.
    xrun_counter: XrunCounter,

    // DROP-ORDER INVARIANT: audio_logger MUST be declared before log_drain.
    // Rust drops struct fields in declaration order.  LogDrainHandle::drop
    // joins the drain thread, which exits only after Consumer::is_abandoned()
    // returns true — which only happens after the Producer (inside AudioLogger)
    // is dropped.  If log_drain dropped first, join() would block forever.
    /// Write end of the audio→log bridge.  Only the audio thread calls `log()`.
    /// Currently unused because xrun events are not observable from Plugin::process
    /// in the cpal standalone backend (Phase 4 stop condition).  Kept for v0.2.
    /// See PR #1 thread T11 and docs/specs/tech-quality/audio-thread-token-pattern.md.
    #[allow(dead_code)]
    audio_logger: AudioLogger,

    /// Holds the audio→log drain thread's join handle.  Drop semantics
    /// are load-bearing: when this field drops, its `Drop` impl joins
    /// the drain thread.  Removing this field (or dropping it without
    /// joining) leaks the drain thread on plugin unload, which is UB
    /// in plugin contexts (the thread would reference unloaded library
    /// code after `dlclose`).  See PR #1 thread T14.
    #[allow(dead_code)]
    log_drain: LogDrainHandle,
}

impl Default for TonismPlugin {
    fn default() -> Self {
        let (audio_logger, log_drain) = log_bridge::channel(1024);
        Self {
            params: Arc::new(TonismParams::default()),
            gain_block: Gain {
                db: Decibels::default(),
            },
            latency_meter: LatencyMeter::default(),
            phase: 0.0,
            sample_rate: DEFAULT_SAMPLE_RATE,
            xrun_counter: XrunCounter::default(),
            audio_logger,
            log_drain,
        }
    }
}

impl Plugin for TonismPlugin {
    const NAME: &'static str = "Tonism";
    const VENDOR: &'static str = "Z3U2";
    const URL: &'static str = "https://github.com/Z3U2/tonism";
    const EMAIL: &'static str = "ilyassnasr@gmail.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            aux_input_ports: &[],
            aux_output_ports: &[],
            names: PortNames::const_default(),
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const HARD_REALTIME_ONLY: bool = true;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        crate::gui::editor::create(
            self.params.clone(),
            self.xrun_counter.clone(),
            self.latency_meter.handle(),
        )
    }

    /// Called once before processing begins.  Record the sample rate and
    /// configure the domain chain for the new session.
    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        // Configure the gain block for the new session (no-op for static gain,
        // but establishes the prepare→reset→process pattern for stateful blocks).
        self.gain_block.prepare(
            SampleRate::new(buffer_config.sample_rate),
            buffer_config.max_buffer_size as usize,
        );
        self.latency_meter.prepare(
            SampleRate::new(buffer_config.sample_rate),
            buffer_config.max_buffer_size as usize,
        );
        true
    }

    /// Called by the host on transport-restart, sample-rate change, etc.
    /// Forwards to the chain's reset() so stateful blocks can clear state.
    fn reset(&mut self) {
        self.gain_block.reset();
        self.latency_meter.reset();
    }

    /// Process one block of audio.
    ///
    /// Signal path (when not bypassed), three explicit stages:
    ///
    /// **Stage 1** — test-signal injection + input_gain (sample-accurate smoothed).
    ///   Iterates samples frame-by-frame: optionally replaces input with a 440 Hz
    ///   sine, then scales by the current smoothed input_gain value.  Phase
    ///   accumulator and smoother both advance once per frame.
    ///
    /// **Stage 2** — domain Gain block (per-channel slice).
    ///   Calls `gain_block.process(channel)` on each raw channel slice.  This is
    ///   the extension point for v0.2 non-linear DSP (distortion, saturation, …);
    ///   it must sit between the two gain trims so the overall path remains
    ///   `input_gain → [domain block] → output_gain`.
    ///
    /// **Stage 3** — output_gain (sample-accurate smoothed).
    ///   A second frame-by-frame pass advances the output smoother once per frame.
    ///
    /// A2-safety: no alloc, no lock, no syscall anywhere in this function.
    /// - `bypass.value()` / `test_signal.value()` — atomic reads.
    /// - `smoothed.next()` — lock-free smoother step.
    /// - `gain_block.process()` — tight numeric loop.
    /// - `audio_logger.log()` — non-blocking rtrb push.
    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Hard bypass: the Buffer is already in-place (input == output),
        // so returning immediately passes the signal through unchanged.
        if self.params.bypass.value() {
            // Cancel any in-progress measurement so the GUI receives a clean
            // Cancelled sentinel rather than a partial / stale result.
            self.latency_meter.cancel();
            return ProcessStatus::Normal;
        }

        let test_signal = self.params.test_signal.value();
        let phase_inc = TAU * 440.0 / self.sample_rate;

        // Stage 1: optionally replace input with the 440 Hz sine, then apply input_gain.
        // Smoother::next() advances one step per frame; phase advances once per frame.
        for channel_samples in buffer.iter_samples() {
            let in_gain: GainLinear = Decibels::new(self.params.input_gain.smoothed.next()).into();
            let sine = self.phase.sin();
            self.phase = (self.phase + phase_inc) % TAU;
            for sample in channel_samples {
                if test_signal {
                    // Replace input with the 440 Hz sine (AC1 test-signal path).
                    // Phase 4 ships the toggle; latency-measurement algorithm is dev work.
                    *sample = sine;
                }
                *sample *= in_gain.value();
            }
        }

        // Stage 2: latency meter (channel 0 only) then domain Gain block (all channels).
        // `as_slice()` returns `&mut [&mut [f32]]` — one contiguous slice per channel.
        // The latency meter runs only on channel 0: it captures the loopback signal and,
        // when armed, overwrites channel 0 with a single Kronecker impulse sample.
        for (idx, channel) in buffer.as_slice().iter_mut().enumerate() {
            if idx == 0 {
                self.latency_meter.process(channel);
            }
            self.gain_block.process(channel);
        }

        // Stage 3: apply output_gain.  Smoother advances one step per frame.
        for channel_samples in buffer.iter_samples() {
            let out_gain: GainLinear =
                Decibels::new(self.params.output_gain.smoothed.next()).into();
            for sample in channel_samples {
                *sample *= out_gain.value();
            }
        }

        ProcessStatus::Normal
    }
}
