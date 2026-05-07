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

use super::log_bridge::{self, AudioLogger, LogDrainHandle};
use super::params::TonismParams;
use super::xrun::XrunCounter;

/// The Tonism audio plugin struct.
pub struct TonismPlugin {
    params: Arc<TonismParams>,

    /// Domain gain block applied in the middle of the signal path.
    gain_block: Gain,

    /// Phase accumulator for the 440 Hz test-signal sine generator.
    /// Advanced by `2π * 440 / sample_rate` each sample; wrapped with `% TAU`.
    phase: f32,

    /// Sample rate set in `initialize()`.  Needed for test-signal frequency.
    sample_rate: f32,

    /// Xrun counter shared with the GUI editor.
    /// Pre-cloned before processing starts to keep `process()` alloc-free.
    xrun_counter: XrunCounter,

    /// Write end of the audio→log bridge.  Only the audio thread calls `log()`.
    /// Currently unused because xrun events are not observable from Plugin::process
    /// in the cpal standalone backend (Phase 4 stop condition).  Kept for v0.2.
    #[allow(dead_code)]
    audio_logger: Option<AudioLogger>,

    /// Keeps the drain thread alive.  Dropped when the plugin is destroyed.
    _log_drain: Option<LogDrainHandle>,
}

impl Default for TonismPlugin {
    fn default() -> Self {
        let (audio_logger, log_drain) = log_bridge::channel(1024);
        Self {
            params: Arc::new(TonismParams::default()),
            gain_block: Gain { db: Decibels(0.0) },
            phase: 0.0,
            sample_rate: 44_100.0,
            xrun_counter: XrunCounter::default(),
            audio_logger: Some(audio_logger),
            _log_drain: Some(log_drain),
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
        crate::gui::editor::create(self.params.clone(), self.xrun_counter.clone())
    }

    /// Called once before processing begins.  Record the sample rate so the
    /// test-signal generator can compute the correct phase increment.
    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        // Reset the gain block for the new sample rate (no-op for static gain,
        // but establishes the pattern for stateful blocks in v0.2).
        self.gain_block.reset(SampleRate(buffer_config.sample_rate as u32));
        true
    }

    /// Process one block of audio.
    ///
    /// Signal path (when not bypassed):
    ///   [optional: replace input with 440 Hz sine]
    ///   → input_gain (sample-accurate smoothed)
    ///   → domain Gain block
    ///   → output_gain (sample-accurate smoothed)
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
            return ProcessStatus::Normal;
        }

        let test_signal = self.params.test_signal.value();
        let phase_inc = TAU * 440.0 / self.sample_rate;

        // Single-pass per-sample processing: apply input gain, then output gain.
        // The domain Gain block is applied per-channel on the raw slices after
        // this loop (channel-slice API, no per-sample overhead).
        for channel_samples in buffer.iter_samples() {
            let in_gain_linear: GainLinear = Decibels(self.params.input_gain.smoothed.next()).into();
            let out_gain_linear: GainLinear =
                Decibels(self.params.output_gain.smoothed.next()).into();

            // Compute sine once per frame; all channels get the same test tone.
            let sine = self.phase.sin();
            self.phase = (self.phase + phase_inc) % TAU;

            for sample in channel_samples {
                if test_signal {
                    // Replace input with the 440 Hz sine (AC1 test-signal path).
                    // Phase 4 ships the toggle; latency-measurement algorithm is dev work.
                    *sample = sine;
                }

                // Apply input gain then output gain in one multiply sequence.
                *sample *= in_gain_linear.0;
                *sample *= out_gain_linear.0;
            }
        }

        // Apply the domain Gain block per channel (proves the Process trait path is live).
        // `as_slice()` returns `&mut [&mut [f32]]` — one contiguous slice per channel.
        for channel in buffer.as_slice() {
            self.gain_block.process(channel);
        }

        ProcessStatus::Normal
    }
}
