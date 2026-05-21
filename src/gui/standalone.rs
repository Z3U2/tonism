//! eframe-based standalone GUI host (C6) for the cpal-direct path.
//!
//! Phase D: static UI — controls are interactive but not wired to the
//! audio param system. Proves "adding a window breaks audio" is
//! isolated from "GUI traffic breaks audio" (Phase E wires the params).

use std::time::Duration;

const WINDOW_WIDTH: f32 = 400.0;
const WINDOW_HEIGHT: f32 = 320.0;

pub struct TonismApp {
    input_gain_db: f32,
    output_gain_db: f32,
    bypass: bool,
    test_signal: bool,
}

impl TonismApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Self {
            input_gain_db: 0.0,
            output_gain_db: 0.0,
            bypass: false,
            test_signal: false,
        }
    }
}

impl eframe::App for TonismApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.label("Input Gain");
        ui.add(egui::Slider::new(&mut self.input_gain_db, -60.0..=12.0).suffix(" dB"));

        ui.label("Output Gain");
        ui.add(egui::Slider::new(&mut self.output_gain_db, -60.0..=12.0).suffix(" dB"));

        ui.checkbox(&mut self.bypass, "Bypass");
        ui.checkbox(&mut self.test_signal, "Test Signal");

        ui.separator();

        ui.label("xrun: 0");
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
