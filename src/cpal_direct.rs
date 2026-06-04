//! cpal-direct standalone entry point (C8 composition root).
//!
//! Two public entries:
//!
//! - [`run_gui`]: opens an eframe window (C6) alongside the running audio
//!   stream. Used by `src/main.rs` (the default `tonism` binary).
//! - [`run`]: headless, blocks on stdin. Used by `src/bin/feedback.rs`
//!   for iteration without a window.
//!
//! Signal path:
//!
//! ```text
//! input  →  ×input_gain (smoothed)  →  rtrb ring
//!                                          ↓
//!                                  Gain::process (0 dB)
//!                                          ↓
//!                                  ×output_gain (smoothed)
//!                                          ↓
//!                                        output
//! ```
//!
//! Phase F additions: `bypass` gates all processing (passthrough when
//! on), `test_signal` injects a 440 Hz sine in place of mic input,
//! `LatencyMeter` captures loopback on channel 0 of the output buffer
//! (deinterleaved via a pre-allocated scratch buffer).
//!
//! The xrun counter (C5) is shared between audio callbacks and the GUI;
//! ring over/underflows bump the counter, and the eframe app reads it
//! each frame for the live display.

use std::f32::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use cpal::traits::{DeviceTrait, StreamTrait};
use rtrb::RingBuffer;

use crate::audio::latency::{CAPTURE_LEN, LatencyHandle, LatencyMeter};
use crate::audio::rt_guard::assert_no_alloc_audio;
use crate::audio::xrun::XrunCounter;
use crate::domain::blocks::gain::Gain;
use crate::domain::process::Process;
use crate::domain::types::{Decibels, GainLinear, SampleRate};
use crate::device::{device_label, err_fn};
use crate::params::{FloatParamHandle, TonismParams};

/// Delay between input and output, in milliseconds. Absorbs clock drift
/// between input and output devices that are not running on the same
/// hardware clock. Matches the upstream `cpal/examples/feedback.rs`
/// default; will shrink in Phase G when device-pair / aggregate-device
/// assumptions land.
const LATENCY_MS: f32 = 150.0;

/// Upper bound on samples per cpal callback, passed to [`Process::prepare`]
/// so stateful domain blocks can pre-allocate. Generous so it covers
/// worst-case interleaved buffers (8192 frames × 8 channels).
const MAX_BLOCK_SIZE: usize = 8192 * 8;

/// Smoothing time used when `--ramp` is on. Production trims smooth in
/// ~20 ms (per [`TonismParams::PRODUCTION_SMOOTHING_SECS`]) which is too
/// fast to perceive as anything other than "instantaneous level
/// change." The ramp test exists to demonstrate the smoother audibly,
/// so it gets a duration that sounds like a real fade.
const RAMP_SMOOTHING_SECS: f32 = 1.0;

// ----------------------------------------------------------------------
// Entry point.
// ----------------------------------------------------------------------

/// A running pair of cpal streams. Holds the audio-side state only;
/// `TonismParams` (GUI handles) lives separately and is passed into
/// [`build_streams`] by reference. Dropping this struct stops playback.
pub struct AudioStreams {
    pub _input_stream: cpal::Stream,
    pub _output_stream: cpal::Stream,
    pub xrun_counter: XrunCounter,
    pub latency_handle: LatencyHandle,
    pub sample_rate: Arc<AtomicU32>,
    pub ring_latency_frames: usize,
}

/// CLI options parsed from `std::env::args()`.
struct CliOpts {
    ramp_test: bool,
    input_device_name: Option<String>,
    output_device_name: Option<String>,
}

fn parse_cli() -> CliOpts {
    let args: Vec<String> = std::env::args().collect();
    let ramp_test = args.iter().any(|a| a == "--ramp");
    let input_device_name = args
        .iter()
        .position(|a| a == "--input")
        .and_then(|i| args.get(i + 1).cloned());
    let output_device_name = args
        .iter()
        .position(|a| a == "--output")
        .and_then(|i| args.get(i + 1).cloned());
    CliOpts {
        ramp_test,
        input_device_name,
        output_device_name,
    }
}

/// Find the index of `device` in `list` by matching device ID strings.
/// Returns 0 if not found.
fn find_device_index(list: &[crate::device::DeviceInfo], device: &cpal::Device) -> usize {
    let target_id = device.id().ok().map(|id| id.to_string());
    list.iter()
        .position(|d| target_id.as_deref() == Some(&d.id_string))
        .unwrap_or(0)
}

/// Build the ring buffer, domain blocks, and cpal streams for the given
/// devices and configuration. The caller is responsible for creating
/// [`TonismParams`] and resolving the devices; this function borrows
/// the GUI-side handles to extract fresh audio-side readers.
///
/// Returns an [`AudioStreams`] whose lifetime keeps both streams alive.
/// Dropping it stops playback.
///
/// # Parameters
/// - `input_device` / `output_device` — cpal device handles.
/// - `sample_rate` — raw sample-rate value in Hz (e.g. `48_000`).
/// - `buffer_size` — `None` → `cpal::BufferSize::Default`; `Some(n)` →
///   `cpal::BufferSize::Fixed(n)`.
/// - `channels` — interleaved channel count used for both streams.
/// - `params` — GUI-side param handles; smoothed audio readers are
///   derived from these via [`FloatParamHandle::build_smoothed`].
/// - `ramp_test` — when `true` the phase accumulator and sine generator
///   are active (the test-signal gate is still controlled by
///   `params.test_signal` at runtime).
pub fn build_streams(
    input_device: &cpal::Device,
    output_device: &cpal::Device,
    sample_rate: u32,
    buffer_size: Option<u32>,
    channels: u16,
    params: &TonismParams,
    _ramp_test: bool,
) -> anyhow::Result<AudioStreams> {
    let config = cpal::StreamConfig {
        channels,
        sample_rate,
        buffer_size: match buffer_size {
            None => cpal::BufferSize::Default,
            Some(n) => cpal::BufferSize::Fixed(n),
        },
    };
    let channels_usize = channels as usize;
    let sr = SampleRate::new(sample_rate as f32);

    // Build fresh audio-side smoothed readers from the GUI handles.
    let mut input_gain_audio = params.input_gain.build_smoothed();
    let mut output_gain_audio = params.output_gain.build_smoothed();
    input_gain_audio.prepare(sr);
    output_gain_audio.prepare(sr);
    input_gain_audio.snap_to_target();
    output_gain_audio.snap_to_target();

    // Clone the bool params for the callbacks.
    let bypass = params.bypass.clone();
    let test_signal = params.test_signal.clone();

    // Ring buffer (same shape as Phase A/B).
    let latency_frames = (LATENCY_MS / 1_000.0) * sample_rate as f32;
    assert!(
        CAPTURE_LEN > latency_frames as usize,
        "CAPTURE_LEN ({CAPTURE_LEN}) must exceed ring pre-fill ({} frames) \
         so the latency meter can capture the echo",
        latency_frames as usize,
    );
    let latency_samples = latency_frames as usize * channels_usize;
    let (mut producer, mut consumer) = RingBuffer::<f32>::new(latency_samples * 2);
    for _ in 0..latency_samples {
        producer
            .push(0.0)
            .expect("ring has 2× headroom for the pre-fill");
    }

    // Domain Gain block.
    let mut gain_block = Gain {
        db: Decibels::default(),
    };
    gain_block.prepare(sr, MAX_BLOCK_SIZE);
    gain_block.reset();

    // LatencyMeter (C5) — captures loopback on channel 0, emits impulses.
    let mut latency_meter = LatencyMeter::default();
    let latency_handle = latency_meter.handle();
    latency_meter.prepare(sr, MAX_BLOCK_SIZE);
    latency_meter.reset();

    // Pre-allocated scratch buffer for channel-0 deinterleave. Sized to
    // the worst-case frame count (MAX_BLOCK_SIZE covers the interleaved
    // total; dividing by channels gives the frame count, but mono is the
    // worst case where frames == samples).
    let mut ch0_scratch: Vec<f32> = vec![0.0; MAX_BLOCK_SIZE];

    // Shared sample rate for the GUI's measure_latency() call.
    let sample_rate_shared = Arc::new(AtomicU32::new(sr.value().to_bits()));

    let xrun_counter = XrunCounter::default();
    let input_xrun = xrun_counter.clone();
    let output_xrun = xrun_counter.clone();

    // Clone bypass for the output closure; test_signal moves into input only.
    let input_bypass = bypass.clone();
    let output_bypass = bypass;

    // Phase accumulator for the 440 Hz test-signal sine generator.
    let phase_inc = TAU * 440.0 / sr.value();
    let mut phase: f32 = 0.0;

    // Input callback: optionally inject test signal, apply input gain
    // (skipped under bypass), push to ring.
    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        assert_no_alloc_audio(|| {
            let is_bypass = input_bypass.value();
            let is_test_signal = test_signal.value();
            let mut fell_behind = false;
            let mut frame_start = 0;
            while frame_start < data.len() {
                let sine = phase.sin();
                phase = (phase + phase_inc) % TAU;

                let mul = if is_bypass {
                    1.0
                } else {
                    let in_gain_db = input_gain_audio.next();
                    let in_gain: GainLinear = Decibels::new(in_gain_db).into();
                    in_gain.value()
                };

                for ch in 0..channels_usize {
                    let raw = if is_test_signal {
                        sine
                    } else {
                        data[frame_start + ch]
                    };
                    let scaled = raw * mul;
                    if producer.push(scaled).is_err() {
                        fell_behind = true;
                    }
                }
                frame_start += channels_usize;
            }
            if fell_behind {
                input_xrun.bump();
                eprintln!("output stream fell behind — consider increasing LATENCY_MS");
            }
        });
    };

    // Output callback: drain ring → latency meter (ch0) → domain gain
    // block → output gain. All processing skipped under bypass.
    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        assert_no_alloc_audio(|| {
            let mut fell_behind = false;
            for sample in data.iter_mut() {
                *sample = match consumer.pop() {
                    Ok(s) => s,
                    Err(_) => {
                        fell_behind = true;
                        0.0
                    }
                };
            }

            if output_bypass.value() {
                latency_meter.cancel();
                latency_meter.disarm();
            } else {
                // Deinterleave channel 0 → scratch, process, write back.
                let n_frames = data.len() / channels_usize;
                for i in 0..n_frames {
                    ch0_scratch[i] = data[i * channels_usize];
                }
                latency_meter.process(&mut ch0_scratch[..n_frames]);
                for i in 0..n_frames {
                    data[i * channels_usize] = ch0_scratch[i];
                }

                gain_block.process(data);

                let mut frame_start = 0;
                while frame_start < data.len() {
                    let out_gain_db = output_gain_audio.next();
                    let out_gain: GainLinear = Decibels::new(out_gain_db).into();
                    let mul = out_gain.value();
                    for ch in 0..channels_usize {
                        data[frame_start + ch] *= mul;
                    }
                    frame_start += channels_usize;
                }
            }

            if fell_behind {
                output_xrun.bump();
                eprintln!("input stream fell behind — consider increasing LATENCY_MS");
            }
        });
    };

    println!("Building both streams with f32 samples at {config:?}");
    let input_stream = input_device.build_input_stream(&config, input_data_fn, err_fn, None)?;
    let output_stream = output_device.build_output_stream(&config, output_data_fn, err_fn, None)?;
    println!("Streams built. Starting playback.");

    input_stream.play()?;
    output_stream.play()?;

    Ok(AudioStreams {
        _input_stream: input_stream,
        _output_stream: output_stream,
        xrun_counter,
        latency_handle,
        sample_rate: sample_rate_shared,
        ring_latency_frames: latency_frames as usize,
    })
}

/// Boot the cpal-direct standalone with an eframe GUI window (C6).
///
/// The audio stream runs on cpal's threads; the eframe event loop runs
/// on the main thread (required by macOS/winit). Closing the window
/// returns control here, dropping the streams.
///
/// CLI flags:
/// - `--ramp` — smoother audibility test (cycles output_gain)
/// - `--input <name>` — select input device by name substring
/// - `--output <name>` — select output device by name substring
pub fn run_gui() -> anyhow::Result<()> {
    let opts = parse_cli();

    // Load persisted config.
    let config = crate::config::load_config();

    // Create params once — they survive the app lifetime.
    let smoothing_time_secs = if opts.ramp_test {
        RAMP_SMOOTHING_SECS
    } else {
        TonismParams::PRODUCTION_SMOOTHING_SECS
    };
    let (gui_params, _initial_audio_params) = TonismParams::new(smoothing_time_secs);

    // Resolve initial device config using the device module.
    let host = cpal::default_host();
    let (resolved, inputs, outputs) = crate::device::resolve_initial_config(
        &host,
        &config,
        opts.input_device_name.as_deref(),
        opts.output_device_name.as_deref(),
    )?;

    // Find indices of the resolved devices in the lists for the GUI dropdowns.
    let input_idx = find_device_index(&inputs, &resolved.input_device);
    let output_idx = find_device_index(&outputs, &resolved.output_device);

    println!(
        "Using input device:  {:?}",
        device_label(&resolved.input_device)
    );
    println!(
        "Using output device: {:?}",
        device_label(&resolved.output_device)
    );

    // Build initial streams.
    let streams = build_streams(
        &resolved.input_device,
        &resolved.output_device,
        resolved.sample_rate,
        resolved.buffer_size,
        resolved.channels,
        &gui_params,
        opts.ramp_test,
    )?;

    if opts.ramp_test {
        spawn_ramp_thread(gui_params.output_gain.clone());
        println!("\n[ramp] mode ON — output_gain will cycle -60 → 0 → -60 dB.");
    }

    println!("\nPlaying. Close the window to stop.\n");

    eframe::run_native(
        "Tonism",
        crate::gui::standalone::native_options(),
        Box::new(move |cc| {
            Ok(Box::new(crate::gui::standalone::TonismApp::new(
                cc,
                gui_params,
                streams,
                host,
                inputs,
                outputs,
                input_idx,
                output_idx,
                resolved.sample_rate,
                resolved.buffer_size,
                config,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;
    Ok(())
}

/// Headless entry: blocks on stdin instead of opening a window. Used by
/// `src/bin/feedback.rs` for iteration without a GUI.
pub fn run() -> anyhow::Result<()> {
    let opts = parse_cli();

    // Load persisted config.
    let config = crate::config::load_config();

    // Create params once — they survive for the duration of the session.
    let smoothing_time_secs = if opts.ramp_test {
        RAMP_SMOOTHING_SECS
    } else {
        TonismParams::PRODUCTION_SMOOTHING_SECS
    };
    let (gui_params, _initial_audio_params) = TonismParams::new(smoothing_time_secs);

    // Resolve devices.
    let host = cpal::default_host();
    let (resolved, _, _) = crate::device::resolve_initial_config(
        &host,
        &config,
        opts.input_device_name.as_deref(),
        opts.output_device_name.as_deref(),
    )?;

    println!(
        "Using input device:  {:?}",
        device_label(&resolved.input_device)
    );
    println!(
        "Using output device: {:?}",
        device_label(&resolved.output_device)
    );

    let streams = build_streams(
        &resolved.input_device,
        &resolved.output_device,
        resolved.sample_rate,
        resolved.buffer_size,
        resolved.channels,
        &gui_params,
        opts.ramp_test,
    )?;

    if opts.ramp_test {
        spawn_ramp_thread(gui_params.output_gain.clone());
        println!("\n[ramp] mode ON — output_gain will cycle -60 → 0 → -60 dB.");
    }

    println!("\nPlaying with {LATENCY_MS:.0} ms of buffered latency.");
    println!("Press <Enter> to stop.\n");

    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;

    drop(streams);
    println!("Stopped.");
    Ok(())
}

// ----------------------------------------------------------------------
// Helpers.
// ----------------------------------------------------------------------

/// Spawn a detached worker thread that periodically retargets
/// `output_gain` so a listener can verify the smoother is click-free
/// across the full -60 dB → 0 dB range. The thread exits when the
/// process exits (no orderly shutdown signal — the main thread's
/// `<Enter>`-and-drop is the kill switch).
fn spawn_ramp_thread(handle: FloatParamHandle) {
    use std::thread;
    use std::time::Duration;
    thread::spawn(move || {
        // Audible-throughout cycle. -18 dB is ~12 % linear volume; you
        // still hear yourself clearly at all four steps. Earlier
        // versions dwelt at -40 dB and -60 dB, which is essentially
        // silent and made the test useless to listen to. The pattern
        // also includes a brief +0 dB peak so the contrast is obvious.
        let cycle: &[f32] = &[-18.0, -6.0, 0.0, -6.0];
        // Step dwell is `RAMP_SMOOTHING_SECS` + 1 s so the listener
        // hears the full fade, then ~1 s of hold at the new level
        // before the next transition starts.
        let step = Duration::from_secs_f32(RAMP_SMOOTHING_SECS + 1.0);
        loop {
            for &db in cycle {
                handle.set(db);
                eprintln!("[ramp] output_gain target = {db:>5.1} dB");
                thread::sleep(step);
            }
        }
    });
}

