//! eframe-based standalone GUI host (C6) for the cpal-direct path.
//!
//! Phase G: adds a device picker panel that lets the user switch input/output
//! devices, sample rates, and buffer sizes at runtime. Selecting new settings
//! and pressing Apply tears down the old cpal streams and builds new ones
//! without touching the param atomics, so gain/bypass/etc survive the rebuild.

use std::sync::atomic::Ordering;
use std::time::Duration;

use cpal::traits::DeviceTrait;

use crate::audio::latency::{CAPTURE_LEN, CaptureState, N_IMPULSES};
use crate::config::{TonismConfig, save_config};
use crate::cpal_direct::{AudioStreams, build_streams};
use crate::device::{
    DeviceInfo, compute_available_buffer_sizes, compute_common_sample_rates, enumerate_devices,
};
use crate::domain::latency::{DEFAULT_MIN_LAG_SAMPLES, measure_latency};
use crate::domain::types::SampleRate;
use crate::params::TonismParams;

const WINDOW_WIDTH: f32 = 400.0;
const WINDOW_HEIGHT: f32 = 480.0;

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
    // Params survive rebuilds (Arc atomics).
    params: TonismParams,

    // Rebuilt on device change — None means audio stopped (error/initial).
    streams: Option<AudioStreams>,

    // Device picker state.
    host: cpal::Host,
    available_inputs: Vec<DeviceInfo>,
    available_outputs: Vec<DeviceInfo>,
    selected_input_idx: usize,
    selected_output_idx: usize,
    available_sample_rates: Vec<u32>,
    selected_sr_idx: usize,
    available_buffer_sizes: Vec<Option<u32>>,
    selected_buf_idx: usize,

    // Config persistence.
    config: TonismConfig,

    // Per-session state.
    latency_state: LatencyState,
    status_message: Option<String>,
}

impl TonismApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        params: TonismParams,
        streams: AudioStreams,
        host: cpal::Host,
        available_inputs: Vec<DeviceInfo>,
        available_outputs: Vec<DeviceInfo>,
        initial_input_idx: usize,
        initial_output_idx: usize,
        initial_sr: u32,
        initial_buf_size: Option<u32>,
        config: TonismConfig,
    ) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        // Compute initial SR and buffer size lists.
        let available_sample_rates =
            if !available_inputs.is_empty() && !available_outputs.is_empty() {
                compute_common_sample_rates(
                    &available_inputs[initial_input_idx],
                    &available_outputs[initial_output_idx],
                )
            } else {
                vec![initial_sr]
            };
        let selected_sr_idx = available_sample_rates
            .iter()
            .position(|&r| r == initial_sr)
            .unwrap_or(0);

        let available_buffer_sizes =
            if !available_inputs.is_empty() && !available_outputs.is_empty() {
                compute_available_buffer_sizes(
                    &available_inputs[initial_input_idx],
                    &available_outputs[initial_output_idx],
                )
            } else {
                vec![None]
            };
        let selected_buf_idx = available_buffer_sizes
            .iter()
            .position(|&b| b == initial_buf_size)
            .unwrap_or(0);

        Self {
            params,
            streams: Some(streams),
            host,
            available_inputs,
            available_outputs,
            selected_input_idx: initial_input_idx,
            selected_output_idx: initial_output_idx,
            available_sample_rates,
            selected_sr_idx,
            available_buffer_sizes,
            selected_buf_idx,
            config,
            latency_state: LatencyState::default(),
            status_message: None,
        }
    }

    /// Recompute the available SR and buffer size lists after an input/output
    /// device selection change, resetting the selected indices if needed.
    fn recompute_device_lists(&mut self) {
        if self.available_inputs.is_empty() || self.available_outputs.is_empty() {
            return;
        }
        let input_info = &self.available_inputs[self.selected_input_idx];
        let output_info = &self.available_outputs[self.selected_output_idx];

        let new_rates = compute_common_sample_rates(input_info, output_info);
        // Try to keep the currently selected rate; fall back to index 0.
        let current_sr = self
            .available_sample_rates
            .get(self.selected_sr_idx)
            .copied();
        self.available_sample_rates = new_rates;
        self.selected_sr_idx = current_sr
            .and_then(|r| self.available_sample_rates.iter().position(|&x| x == r))
            .unwrap_or(0);

        let new_bufs = compute_available_buffer_sizes(input_info, output_info);
        let current_buf = self
            .available_buffer_sizes
            .get(self.selected_buf_idx)
            .copied();
        self.available_buffer_sizes = new_bufs;
        self.selected_buf_idx = current_buf
            .and_then(|b| self.available_buffer_sizes.iter().position(|&x| x == b))
            .unwrap_or(0);
    }

    /// Apply the current picker selection: stop old streams, build new ones.
    fn apply_device_selection(&mut self) {
        let input_device = self.available_inputs[self.selected_input_idx]
            .device
            .clone();
        let output_device = self.available_outputs[self.selected_output_idx]
            .device
            .clone();

        let sample_rate = self
            .available_sample_rates
            .get(self.selected_sr_idx)
            .copied()
            .unwrap_or(48_000);
        let buffer_size = self
            .available_buffer_sizes
            .get(self.selected_buf_idx)
            .copied()
            .unwrap_or(None);

        let channels = output_device
            .default_output_config()
            .map(|c| c.channels())
            .unwrap_or(2);

        // Stop old streams first.
        self.streams = None;

        match build_streams(
            &input_device,
            &output_device,
            sample_rate,
            buffer_size,
            channels,
            &self.params,
            false,
        ) {
            Ok(new_streams) => {
                self.streams = Some(new_streams);

                // Persist the new selection.
                self.config.input_device_id = input_device.id().ok().map(|id| id.to_string());
                self.config.output_device_id = output_device.id().ok().map(|id| id.to_string());
                self.config.sample_rate = Some(sample_rate);
                self.config.buffer_size = buffer_size;
                if let Err(e) = save_config(&self.config) {
                    eprintln!("[config] save failed: {e}");
                }

                self.status_message = Some("Audio restarted".into());
                self.latency_state = LatencyState::default();
            }
            Err(e) => {
                self.status_message = Some(format!("Error: {e}"));
            }
        }
    }

    /// Re-enumerate devices from the host, preserving selections by id_string.
    fn refresh_devices(&mut self) {
        let prev_input_id = self
            .available_inputs
            .get(self.selected_input_idx)
            .map(|d| d.id_string.clone());
        let prev_output_id = self
            .available_outputs
            .get(self.selected_output_idx)
            .map(|d| d.id_string.clone());

        let (new_inputs, new_outputs) = enumerate_devices(&self.host);

        self.selected_input_idx = prev_input_id
            .as_deref()
            .and_then(|id| new_inputs.iter().position(|d| d.id_string == id))
            .unwrap_or(0);
        self.selected_output_idx = prev_output_id
            .as_deref()
            .and_then(|id| new_outputs.iter().position(|d| d.id_string == id))
            .unwrap_or(0);

        self.available_inputs = new_inputs;
        self.available_outputs = new_outputs;
        self.recompute_device_lists();
        self.status_message = Some("Devices refreshed".into());
    }
}

impl eframe::App for TonismApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // ---- Device picker section ----
        ui.label("Audio Devices");

        // Input combo.
        let mut input_changed = false;
        if !self.available_inputs.is_empty() {
            let selected_input_name = self.available_inputs[self.selected_input_idx].name.clone();
            egui::ComboBox::from_label("Input")
                .selected_text(&selected_input_name)
                .show_ui(ui, |ui| {
                    for (idx, info) in self.available_inputs.iter().enumerate() {
                        if ui
                            .selectable_value(&mut self.selected_input_idx, idx, &info.name)
                            .changed()
                        {
                            input_changed = true;
                        }
                    }
                });
        }

        // Output combo.
        let mut output_changed = false;
        if !self.available_outputs.is_empty() {
            let selected_output_name = self.available_outputs[self.selected_output_idx]
                .name
                .clone();
            egui::ComboBox::from_label("Output")
                .selected_text(&selected_output_name)
                .show_ui(ui, |ui| {
                    for (idx, info) in self.available_outputs.iter().enumerate() {
                        if ui
                            .selectable_value(&mut self.selected_output_idx, idx, &info.name)
                            .changed()
                        {
                            output_changed = true;
                        }
                    }
                });
        }

        if input_changed || output_changed {
            self.recompute_device_lists();
        }

        // Sample rate combo.
        if !self.available_sample_rates.is_empty() {
            let sr_label = self
                .available_sample_rates
                .get(self.selected_sr_idx)
                .map(|r| r.to_string())
                .unwrap_or_else(|| "—".into());
            egui::ComboBox::from_label("Sample Rate")
                .selected_text(sr_label)
                .show_ui(ui, |ui| {
                    for (idx, &rate) in self.available_sample_rates.iter().enumerate() {
                        ui.selectable_value(&mut self.selected_sr_idx, idx, rate.to_string());
                    }
                });
        }

        // Buffer size combo.
        {
            let buf_label = self
                .available_buffer_sizes
                .get(self.selected_buf_idx)
                .map(|b| buffer_size_label(*b))
                .unwrap_or_else(|| "Default".into());
            egui::ComboBox::from_label("Buffer Size")
                .selected_text(buf_label)
                .show_ui(ui, |ui| {
                    let n = self.available_buffer_sizes.len();
                    for idx in 0..n {
                        let label = buffer_size_label(self.available_buffer_sizes[idx]);
                        ui.selectable_value(&mut self.selected_buf_idx, idx, label);
                    }
                });
        }

        // Apply / Refresh buttons.
        ui.horizontal(|ui| {
            if ui.button("Apply").clicked() {
                self.apply_device_selection();
            }
            if ui.button("Refresh").clicked() {
                self.refresh_devices();
            }
        });

        if let Some(msg) = &self.status_message {
            ui.label(format!("status: {msg}"));
        }

        ui.separator();

        // ---- Gain / bypass controls ----
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

        // ---- Latency / xrun section ----
        let xrun_val = self
            .streams
            .as_ref()
            .map(|s| s.xrun_counter.read())
            .unwrap_or(0);
        ui.label(format!("xrun: {xrun_val}"));

        // Per-frame latency state machine — only active when streams are up.
        if let Some(streams) = self.streams.as_ref() {
            match streams.latency_handle.state() {
                CaptureState::Idle => {}
                CaptureState::Capturing => {
                    self.latency_state.display = LatencyDisplay::Measuring;
                }
                CaptureState::Done => {
                    // Read into the capture buf using the handle we own in streams.
                    streams
                        .latency_handle
                        .read_capture_into(&mut self.latency_state.capture_buf);

                    let sr_bits = streams.sample_rate.load(Ordering::Relaxed);
                    let sr = SampleRate::new(f32::from_bits(sr_bits));
                    let ring_latency_frames = streams.ring_latency_frames;

                    log_capture_diagnostics(
                        &self.latency_state.capture_buf,
                        sr.value(),
                        ring_latency_frames,
                    );

                    let result = measure_latency(
                        &self.latency_state.capture_buf,
                        N_IMPULSES,
                        DEFAULT_MIN_LAG_SAMPLES,
                        ring_latency_frames,
                        sr,
                    );
                    self.latency_state.display = match result {
                        Ok(ms) => LatencyDisplay::Measured(ms.value()),
                        Err(_) => LatencyDisplay::NoSignal,
                    };
                    streams.latency_handle.reset_to_idle();
                }
                CaptureState::Cancelled => {
                    self.latency_state.display = LatencyDisplay::Cancelled;
                    streams.latency_handle.reset_to_idle();
                }
            }
        }

        if ui.button("Measure latency").clicked()
            && let Some(streams) = self.streams.as_ref()
        {
            streams.latency_handle.request_measurement();
        }

        ui.label(self.latency_state.display.format());

        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }
}

fn buffer_size_label(bs: Option<u32>) -> String {
    match bs {
        None => "Default".into(),
        Some(n) => n.to_string(),
    }
}

fn log_capture_diagnostics(capture: &[f32], sample_rate: f32, ring_latency_frames: usize) {
    if capture.len() < N_IMPULSES {
        return;
    }
    let chunk_len = capture.len() / N_IMPULSES;
    for k in 0..N_IMPULSES {
        let start = k * chunk_len;
        let end = start + chunk_len;
        let chunk = &capture[start..end];
        let (best_lag, best_amp) = chunk.iter().enumerate().skip(DEFAULT_MIN_LAG_SAMPLES).fold(
            (DEFAULT_MIN_LAG_SAMPLES, 0.0_f32),
            |acc, (i, &v)| {
                let a = v.abs();
                if a > acc.1 { (i, a) } else { acc }
            },
        );
        let adjusted = best_lag.saturating_sub(ring_latency_frames);
        eprintln!(
            "[latency] chunk {k}: peak {best_amp:.4} at raw_lag {best_lag} adjusted {adjusted} ({:.2} ms)",
            adjusted as f32 / sample_rate * 1000.0,
        );
    }
}

pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT]),
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

    #[test]
    fn buffer_size_label_none_is_default() {
        assert_eq!(buffer_size_label(None), "Default");
    }

    #[test]
    fn buffer_size_label_some_is_number() {
        assert_eq!(buffer_size_label(Some(256)), "256");
        assert_eq!(buffer_size_label(Some(1024)), "1024");
    }
}
