use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use egui::Visuals;
use nih_plug::prelude::Editor;
use nih_plug_egui::{EguiSettings, EguiState, create_egui_editor, widgets};

use crate::audio::latency::{CAPTURE_LEN, CaptureState, KRONECKER_REF, LatencyHandle};
use crate::audio::params::TonismParams;
use crate::audio::xrun::XrunCounter;
use crate::domain::latency::measure_latency;
use crate::domain::types::SampleRate;

/// Presentation state for the latency readout label.
///
/// Persisted in egui's user-state slot so the label survives across frames
/// after a measurement completes.
#[derive(Clone, Debug, PartialEq)]
enum LatencyDisplay {
    /// No measurement has been run yet (initial / post-cancel state).
    Pending,
    /// A measurement is currently in flight.
    Measuring,
    /// A successful measurement completed; value is the rounded ms result.
    Measured(f32),
    /// The capture window completed but there was no detectable signal.
    NoSignal,
    /// The measurement was cancelled (e.g. bypass toggled mid-capture).
    Cancelled,
}

impl LatencyDisplay {
    fn format(&self) -> String {
        match self {
            LatencyDisplay::Pending => "latency: -- ms".into(),
            LatencyDisplay::Measuring => "latency: measuring...".into(),
            LatencyDisplay::Measured(ms) => format!("latency: {:.1} ms", ms),
            LatencyDisplay::NoSignal => "latency: no signal".into(),
            LatencyDisplay::Cancelled => "latency: -- ms".into(),
        }
    }
}

/// GUI-thread-only state owned by the egui editor.
///
/// `display` holds the latest `LatencyDisplay` variant; rendered every frame.
/// `capture_buf` is reused across measurements so the per-frame closure does
/// not allocate when state transitions to Done — the inner `Vec` is grown to
/// `CAPTURE_LEN` on the first measurement and reused thereafter.
struct LatencyEditorState {
    display: LatencyDisplay,
    capture_buf: Vec<f32>,
}

impl Default for LatencyEditorState {
    fn default() -> Self {
        Self {
            display: LatencyDisplay::Pending,
            capture_buf: Vec::with_capacity(CAPTURE_LEN),
        }
    }
}

/// Default window size in logical pixels: width × height.
pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(400, 320)
}

/// Build the egui editor for Tonism.
///
/// Draws two [`widgets::ParamSlider`] rows (input_gain, output_gain), two
/// checkbox rows (bypass, test_signal), a live xrun counter label, and a
/// "Measure latency" button with a reactive latency readout.
///
/// # xrun observability note
///
/// The cpal standalone wrapper does NOT forward xrun/underflow events back to
/// `Plugin::process`.  The cpal error callback only calls `unparker.unpark()`
/// to stop the stream; there is no channel or hook the plugin can register.
/// The counter stays at 0 in MVP.  Tracked for future work.
///
/// # Per-frame poll
///
/// `ctx.request_repaint_after(16 ms)` keeps the closure re-firing at ~60 Hz
/// without busy-spinning — drives both the xrun counter atom-read and the
/// latency state machine.
///
/// # Sample rate caveat
///
/// `measure_latency` is invoked with `SampleRate::new(48_000.0)` hard-coded.
/// The editor does not currently see the audio session SR; a v0.2 follow-up
/// will plumb it through an `Arc<AtomicU32>` mirroring the `XrunCounter`
/// pattern.  AC1 manual verification is on the dev machine where the user
/// controls the SR.
pub fn create(
    params: Arc<TonismParams>,
    xrun_counter: XrunCounter,
    latency_handle: LatencyHandle,
) -> Option<Box<dyn Editor>> {
    let state = default_state();

    create_egui_editor(
        state,
        LatencyEditorState::default(),
        EguiSettings::default(),
        |ctx, _queue, _state| {
            ctx.set_visuals(Visuals::dark());
        },
        move |ui, setter, _queue, latency_state: &mut LatencyEditorState| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                ui.label("Input Gain");
                ui.add(widgets::ParamSlider::for_param(&params.input_gain, setter));

                ui.label("Output Gain");
                ui.add(widgets::ParamSlider::for_param(&params.output_gain, setter));

                // BoolParam: use a standard checkbox and drive the setter manually.
                let mut bypass = params.bypass.value();
                if ui.checkbox(&mut bypass, "Bypass").changed() {
                    setter.begin_set_parameter(&params.bypass);
                    setter.set_parameter(&params.bypass, bypass);
                    setter.end_set_parameter(&params.bypass);
                }

                let mut test_signal = params.test_signal.value();
                if ui.checkbox(&mut test_signal, "Test Signal").changed() {
                    setter.begin_set_parameter(&params.test_signal);
                    setter.set_parameter(&params.test_signal, test_signal);
                    setter.end_set_parameter(&params.test_signal);
                }

                // Live xrun counter — reads the atomic each frame.
                ui.label(format!("xrun: {}", xrun_counter.0.load(Ordering::Relaxed)));

                ui.separator();

                // Per-frame latency state machine.  Reads `latency_handle.state()`
                // every frame; on the frame the meter transitions to Done, copies
                // the capture buffer out, runs `measure_latency`, and stores the
                // resulting `LatencyDisplay` in user_state for subsequent frames.
                match latency_handle.state() {
                    CaptureState::Idle => {
                        // Leave the current `display` untouched so the last result
                        // remains visible until the user re-arms.
                    }
                    CaptureState::Capturing => {
                        latency_state.display = LatencyDisplay::Measuring;
                    }
                    CaptureState::Done => {
                        latency_handle.read_capture_into(&mut latency_state.capture_buf);
                        let result = measure_latency(
                            &KRONECKER_REF,
                            &latency_state.capture_buf,
                            // TODO(v0.2): plumb actual session SR from
                            // Plugin::initialize through an Arc<AtomicU32>
                            // mirroring the XrunCounter pattern.
                            SampleRate::new(48_000.0),
                        );
                        latency_state.display = match result {
                            Ok(ms) => LatencyDisplay::Measured(ms.value()),
                            Err(_) => LatencyDisplay::NoSignal,
                        };
                        latency_handle.reset_to_idle();
                    }
                    CaptureState::Cancelled => {
                        latency_state.display = LatencyDisplay::Cancelled;
                        latency_handle.reset_to_idle();
                    }
                }

                if ui.button("Measure latency").clicked() {
                    latency_handle.request_measurement();
                }

                ui.label(latency_state.display.format());

                // Repaint at ~60 Hz so both the xrun counter and the latency
                // state keep polling.  request_repaint_after avoids the
                // busy-spin that request_repaint() would cause.
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            });
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_display_format_table() {
        assert_eq!(LatencyDisplay::Pending.format(), "latency: -- ms");
        assert_eq!(LatencyDisplay::Measuring.format(), "latency: measuring...");
        assert_eq!(LatencyDisplay::Measured(7.3).format(), "latency: 7.3 ms");
        assert_eq!(LatencyDisplay::Measured(0.0).format(), "latency: 0.0 ms");
        assert_eq!(LatencyDisplay::NoSignal.format(), "latency: no signal");
        assert_eq!(LatencyDisplay::Cancelled.format(), "latency: -- ms");
    }
}
