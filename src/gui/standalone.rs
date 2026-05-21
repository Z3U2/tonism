//! eframe-based standalone GUI host (C6) for the cpal-direct path.
//!
//! Phase F: sliders and checkboxes are wired to the audio param system
//! via [`FloatParamHandle::set`] / [`BoolParam::set`]. The xrun counter
//! reads C5's shared atomic each frame. The latency meter is driven by
//! a per-frame state machine polling [`LatencyHandle::state`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use crate::audio::latency::{CAPTURE_LEN, CaptureState, LatencyHandle, N_IMPULSES};
use crate::audio::xrun::XrunCounter;
use crate::domain::latency::{DEFAULT_MIN_LAG_SAMPLES, measure_latency};
use crate::domain::types::SampleRate;
use crate::params::TonismParams;

const WINDOW_WIDTH: f32 = 400.0;
const WINDOW_HEIGHT: f32 = 320.0;

#[derive(Clone, Debug, PartialEq)]
enum LatencyDisplay {
    Pending,
    Measuring,
    Measured(f32),
    NoSignal,
    Cancelled,
}

impl LatencyDisplay {
    fn format(&self) -> String {
        match self {
            LatencyDisplay::Pending => "latency: -- ms".into(),
            LatencyDisplay::Measuring => "latency: measuring...".into(),
            LatencyDisplay::Measured(ms) => format!("latency: {ms:.1} ms"),
            LatencyDisplay::NoSignal => "latency: no signal".into(),
            LatencyDisplay::Cancelled => "latency: -- ms".into(),
        }
    }
}

struct LatencyState {
    display: LatencyDisplay,
    capture_buf: Vec<f32>,
}

impl Default for LatencyState {
    fn default() -> Self {
        Self {
            display: LatencyDisplay::Pending,
            capture_buf: Vec::with_capacity(CAPTURE_LEN),
        }
    }
}

pub struct TonismApp {
    params: TonismParams,
    xrun_counter: XrunCounter,
    latency_handle: LatencyHandle,
    sample_rate: Arc<AtomicU32>,
    latency_state: LatencyState,
}

impl TonismApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        params: TonismParams,
        xrun_counter: XrunCounter,
        latency_handle: LatencyHandle,
        sample_rate: Arc<AtomicU32>,
    ) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Self {
            params,
            xrun_counter,
            latency_handle,
            sample_rate,
            latency_state: LatencyState::default(),
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

        // Per-frame latency state machine.
        match self.latency_handle.state() {
            CaptureState::Idle => {}
            CaptureState::Capturing => {
                self.latency_state.display = LatencyDisplay::Measuring;
            }
            CaptureState::Done => {
                self.latency_handle
                    .read_capture_into(&mut self.latency_state.capture_buf);

                log_capture_diagnostics(&self.latency_state.capture_buf);

                let sr_bits = self.sample_rate.load(Ordering::Relaxed);
                let sr = SampleRate::new(f32::from_bits(sr_bits));

                let result = measure_latency(
                    &self.latency_state.capture_buf,
                    N_IMPULSES,
                    DEFAULT_MIN_LAG_SAMPLES,
                    sr,
                );
                self.latency_state.display = match result {
                    Ok(ms) => LatencyDisplay::Measured(ms.value()),
                    Err(_) => LatencyDisplay::NoSignal,
                };
                self.latency_handle.reset_to_idle();
            }
            CaptureState::Cancelled => {
                self.latency_state.display = LatencyDisplay::Cancelled;
                self.latency_handle.reset_to_idle();
            }
        }

        if ui.button("Measure latency").clicked() {
            self.latency_handle.request_measurement();
        }

        ui.label(self.latency_state.display.format());

        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }
}

fn log_capture_diagnostics(capture: &[f32]) {
    if capture.len() < N_IMPULSES {
        return;
    }
    let chunk_len = capture.len() / N_IMPULSES;
    for k in 0..N_IMPULSES {
        let start = k * chunk_len;
        let end = start + chunk_len;
        let chunk = &capture[start..end];
        let (best_lag, best_amp) = chunk
            .iter()
            .enumerate()
            .skip(DEFAULT_MIN_LAG_SAMPLES)
            .fold(
                (DEFAULT_MIN_LAG_SAMPLES, 0.0_f32),
                |acc, (i, &v)| {
                    let a = v.abs();
                    if a > acc.1 {
                        (i, a)
                    } else {
                        acc
                    }
                },
            );
        eprintln!(
            "[latency] chunk {k}: peak {best_amp:.4} at lag {best_lag} ({:.2} ms)",
            best_lag as f32 / 48_000.0 * 1000.0,
        );
    }
}

pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT]),
        ..Default::default()
    }
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
