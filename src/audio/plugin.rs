use std::num::NonZeroU32;
use std::sync::Arc;

use nih_plug::prelude::*;

use super::params::TonismParams;

/// The Tonism audio plugin struct.  Holds shared parameter state and
/// any per-instance data needed on the audio thread.
pub struct TonismPlugin {
    params: Arc<TonismParams>,
}

impl Default for TonismPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(TonismParams::default()),
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

    /// Stub: Phase 4 wires bypass, input/output gain, and the domain Gain block.
    fn process(
        &mut self,
        _buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        ProcessStatus::Normal
    }
}
