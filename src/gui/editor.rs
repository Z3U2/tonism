use std::sync::Arc;

use nih_plug::prelude::Editor;
use vizia_plug::vizia::prelude::*;
use vizia_plug::widgets::*;
use vizia_plug::{create_vizia_editor, ViziaState, ViziaTheming};

use crate::audio::params::TonismParams;

/// Default window size in logical pixels: width × height.
pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (400, 300))
}

/// Build the vizia editor for Tonism.
///
/// Draws four [`ParamSlider`] rows (input_gain, output_gain, bypass,
/// test_signal) plus two static placeholder [`Label`]s for xrun count and
/// latency.  Phase 4 binds those labels to live data.
pub fn create(params: Arc<TonismParams>) -> Option<Box<dyn Editor>> {
    let state = default_state();

    create_vizia_editor(state, ViziaTheming::Custom, move |cx, _gui_context| {
        VStack::new(cx, |cx| {
            Label::new(cx, "Input Gain");
            ParamSlider::new(cx, &params.input_gain);

            Label::new(cx, "Output Gain");
            ParamSlider::new(cx, &params.output_gain);

            Label::new(cx, "Bypass");
            ParamSlider::new(cx, &params.bypass);

            Label::new(cx, "Test Signal");
            ParamSlider::new(cx, &params.test_signal);

            // Placeholder status rows — Phase 4 binds these to live atomics.
            Label::new(cx, "xrun: 0");
            Label::new(cx, "latency: -- ms");
        });
    })
}
