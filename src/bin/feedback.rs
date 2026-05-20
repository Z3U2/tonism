//! Phase A walking skeleton — direct-cpal duplex passthrough.
//!
//! Models the upstream `cpal/examples/feedback.rs` example shape with two
//! deliberate adaptations:
//!
//! 1. Uses `rtrb` instead of `ringbuf` for the SPSC ring (already in
//!    Tonism's tree via [`crate::audio::log_bridge`]); avoids adding a
//!    second ring-buffer crate to the dep surface for the same job.
//! 2. Runs until `<Enter>` is pressed instead of the example's fixed
//!    3-second sleep, so the spec's 5-minute clean-audio session can
//!    complete without a re-run loop.
//!
//! Nothing else is changed from the example's shape: default input +
//! default output device, same `StreamConfig` on both sides, a small
//! latency ring to absorb clock drift between independently-clocked
//! devices, per-sample push/pop in each callback.
//!
//! # Purpose
//!
//! This binary is the falsifier for [ADR-005]'s central premise. If it
//! does NOT produce clean audio on the user's hardware, the
//! cpal-direct pivot is wrong and the ADR must be reopened. If it DOES
//! produce clean audio, Phase A's exit criterion is met and Phase B
//! (domain chain in the callback) can begin.
//!
//! See:
//! - `docs/adr/005-standalone-audio-cpal-direct.md`
//! - `docs/specs/cpal-direct-standalone/spec.md` (Phase A)
//!
//! # Usage
//!
//! ```text
//! cargo run --release --bin feedback
//! # → press <Enter> to stop
//! ```
//!
//! [ADR-005]: ../../docs/adr/005-standalone-audio-cpal-direct.md

use anyhow::Context;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::RingBuffer;

/// Delay between input and output, in milliseconds. Absorbs clock drift
/// between input and output devices that are not running on the same
/// hardware clock (the common case on consumer hardware). Matches the
/// upstream `feedback.rs` default.
const LATENCY_MS: f32 = 150.0;

fn main() -> anyhow::Result<()> {
    let host = cpal::default_host();

    let input_device = host
        .default_input_device()
        .context("no default input device available")?;
    let output_device = host
        .default_output_device()
        .context("no default output device available")?;

    println!("Using input device:  {:?}", device_label(&input_device));
    println!("Using output device: {:?}", device_label(&output_device));

    // Same configuration on both streams — the example's deliberate
    // simplification. If the input device's default config can't be
    // satisfied by the output device, cpal will error on
    // `build_output_stream` below.
    let config: cpal::StreamConfig = input_device.default_input_config()?.into();

    // Size the ring so the producer has a fixed latency-window head start
    // over the consumer. Capacity is 2× the latency window so the
    // producer side has headroom over the consumer.
    let latency_frames = (LATENCY_MS / 1_000.0) * config.sample_rate as f32;
    let latency_samples = latency_frames as usize * config.channels as usize;

    let (mut producer, mut consumer) = RingBuffer::<f32>::new(latency_samples * 2);

    // Pre-fill the ring with `latency_samples` zeros so the output
    // callback has something to play before the input callback's first
    // run. Without this the output drains an empty ring on its first
    // call and falls behind immediately.
    for _ in 0..latency_samples {
        producer
            .push(0.0)
            .expect("ring has 2× headroom for the pre-fill");
    }

    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        let mut fell_behind = false;
        for &sample in data {
            if producer.push(sample).is_err() {
                fell_behind = true;
            }
        }
        if fell_behind {
            eprintln!("output stream fell behind — consider increasing LATENCY_MS");
        }
    };

    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let mut fell_behind = false;
        for sample in data {
            *sample = match consumer.pop() {
                Ok(s) => s,
                Err(_) => {
                    fell_behind = true;
                    0.0
                }
            };
        }
        if fell_behind {
            eprintln!("input stream fell behind — consider increasing LATENCY_MS");
        }
    };

    println!("Building both streams with f32 samples at {config:?}");
    let input_stream = input_device.build_input_stream(&config, input_data_fn, err_fn, None)?;
    let output_stream = output_device.build_output_stream(&config, output_data_fn, err_fn, None)?;
    println!("Streams built. Starting playback.");

    input_stream.play()?;
    output_stream.play()?;

    println!("\nPlaying with {LATENCY_MS:.0} ms of buffered latency.");
    println!("Press <Enter> to stop.\n");

    // Block the main thread on stdin so the cpal-owned audio threads
    // can run. Pressing <Enter> unblocks, the streams drop, the
    // process exits cleanly.
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;

    drop(input_stream);
    drop(output_stream);
    println!("Stopped.");
    Ok(())
}

/// Stream-level error callback. cpal invokes this off the realtime
/// thread, so plain `eprintln!` is fine here.
fn err_fn(err: cpal::StreamError) {
    eprintln!("stream error: {err}");
}

/// Best-effort human-readable device label. Matches the
/// `description()?.name()` pattern used by `scripts/check_buffer_size.rs`
/// (the non-deprecated path in cpal 0.17). Falls back to "<unnamed>" if
/// the description lookup fails — the binary should not bail just
/// because device metadata is unavailable.
fn device_label(device: &cpal::Device) -> String {
    device
        .description()
        .map(|desc| desc.name().to_string())
        .unwrap_or_else(|_| "<unnamed>".into())
}
