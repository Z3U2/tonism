use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use egui::Visuals;
use nih_plug::prelude::Editor;
use nih_plug_egui::{EguiSettings, EguiState, create_egui_editor, widgets};

use crate::audio::params::TonismParams;
use crate::audio::xrun::XrunCounter;

/// Default window size in logical pixels: width × height.
pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(400, 300)
}

/// Build the egui editor for Tonism.
///
/// Draws two [`widgets::ParamSlider`] rows (input_gain, output_gain), two
/// checkbox rows (bypass, test_signal), a live xrun counter label, and a
/// static latency placeholder.
///
/// # xrun observability note
///
/// The cpal standalone wrapper does NOT forward xrun/underflow events back to
/// `Plugin::process`.  The cpal error callback only calls `unparker.unpark()`
/// to stop the stream; there is no channel or hook the plugin can register.
/// The counter stays at 0 in MVP.  Tracked for future work.
///
/// Polling uses `ctx.request_repaint_after(16 ms)` so the xrun label refreshes
/// at ~60 Hz without spinning a core.
pub fn create(params: Arc<TonismParams>, xrun_counter: XrunCounter) -> Option<Box<dyn Editor>> {
    let state = default_state();

    create_egui_editor(
        state,
        // No persistent GUI-only state needed; use () as the user state.
        (),
        EguiSettings::default(),
        |ctx, _queue, _state| {
            ctx.set_visuals(Visuals::dark());
        },
        move |ui, setter, _queue, _state| {
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

                // Latency label: static placeholder — algorithm is dev work post-Phase 4.
                ui.label("latency: -- ms");

                // Repaint at ~60 Hz so the xrun counter keeps polling.
                // request_repaint_after avoids busy-spinning that request_repaint() would cause.
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            });
        },
    )
}
