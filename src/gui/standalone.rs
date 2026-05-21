//! eframe-based standalone GUI host (C6) for the cpal-direct path.
//!
//! Phase E: sliders and checkboxes are wired to the audio param system
//! via [`FloatParamHandle::set`] / [`BoolParam::set`]. The xrun counter
//! reads C5's shared atomic each frame.

use std::time::Duration;

use crate::audio::xrun::XrunCounter;
use crate::params::TonismParams;

const WINDOW_WIDTH: f32 = 400.0;
const WINDOW_HEIGHT: f32 = 320.0;

pub struct TonismApp {
    params: TonismParams,
    xrun_counter: XrunCounter,
}

impl TonismApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        params: TonismParams,
        xrun_counter: XrunCounter,
    ) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Self {
            params,
            xrun_counter,
        }
    }
}

impl eframe::App for TonismApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut input_gain_db = self.params.input_gain.target();
        ui.label("Input Gain");
        if ui
            .add(egui::Slider::new(&mut input_gain_db, -60.0..=12.0).suffix(" dB"))
            .changed()
        {
            self.params.input_gain.set(input_gain_db);
        }

        let mut output_gain_db = self.params.output_gain.target();
        ui.label("Output Gain");
        if ui
            .add(egui::Slider::new(&mut output_gain_db, -60.0..=12.0).suffix(" dB"))
            .changed()
        {
            self.params.output_gain.set(output_gain_db);
        }

        let mut bypass = self.params.bypass.value();
        if ui.checkbox(&mut bypass, "Bypass").changed() {
            self.params.bypass.set(bypass);
        }

        let mut test_signal = self.params.test_signal.value();
        if ui.checkbox(&mut test_signal, "Test Signal").changed() {
            self.params.test_signal.set(test_signal);
        }

        ui.separator();

        ui.label(format!("xrun: {}", self.xrun_counter.read()));
        ui.label("latency: -- ms");

        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }
}

pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT]),
        ..Default::default()
    }
}
