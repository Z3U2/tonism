//! Phase B — domain chain in the cpal callback.
//!
//! Evolves the Phase A walking skeleton by inserting Tonism's domain
//! [`Gain`] block into the output callback. The signal path is now:
//!
//! ```text
//! input stream  →  rtrb ring  →  output stream
//!                                       ↓
//!                                 Gain::process(buffer)  ← (0 dB / identity)
//! ```
//!
//! Per `docs/specs/cpal-direct-standalone/spec.md` Phase B, parameters
//! are hardcoded: gain at 0 dB, bypass off, test-signal off. There is
//! no GUI, no live params, no smoothing — those land in Phase C+.
//! Because the gain is unity, output is bit-identical to input; the
//! point isn't that the audio sounds different, it's that **the
//! callback path now goes through the domain chain**. A clean 5-minute
//! session proves the C2 callback shape can host the domain seam (rule
//! A4) without breaking A2 (no alloc / lock / syscall on the audio
//! thread).
//!
//! # Adaptations from the upstream `cpal/examples/feedback.rs`
//!
//! Carried over from Phase A:
//!
//! 1. Uses `rtrb` instead of `ringbuf` for the SPSC ring (already in
//!    Tonism's tree via [`tonism::audio::log_bridge`]).
//! 2. Runs until `<Enter>` is pressed instead of the example's fixed
//!    3-second sleep, so the spec's 5-minute session fits in one run.
//!
//! New in Phase B:
//!
//! 3. Constructs a domain [`Gain`] block and calls its `prepare` /
//!    `process` lifecycle methods. `prepare` runs on the main thread
//!    before the streams start (the trait permits allocation there);
//!    `process` runs inside the cpal output callback (A2-clean).
//!
//! # Channel-layout note
//!
//! cpal hands the output callback an **interleaved** `&mut [f32]`.
//! [`Gain::process`] multiplies every sample by a single scalar, so
//! interleaved-or-not is irrelevant for unity gain — every sample gets
//! the same multiplier. A future per-channel DSP block (e.g. a stereo
//! effect, channel-0-only latency meter) will require either
//! de-interleaving or evolving the [`Process`] trait. Not Phase B's
//! problem.
//!
//! # Purpose
//!
//! Phase B's exit gate is a manual 5-minute clean-audio session through
//! the domain chain on mac + Windows. On pass, Phase C (parameter
//! system + smoothing) begins. The C10 decision (always-compile vs
//! feature-gate the dormant `Plugin` impl) is recorded at this phase's
//! exit per the spec.
//!
//! See:
//! - `docs/adr/005-standalone-audio-cpal-direct.md`
//! - `docs/specs/cpal-direct-standalone/spec.md` (Phase B)
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
use tonism::domain::blocks::gain::Gain;
use tonism::domain::process::Process;
use tonism::domain::types::{Decibels, SampleRate};

/// Delay between input and output, in milliseconds. Absorbs clock drift
/// between input and output devices that are not running on the same
/// hardware clock (the common case on consumer hardware). Matches the
/// upstream `feedback.rs` default.
const LATENCY_MS: f32 = 150.0;

/// Upper bound on samples per cpal callback, passed to [`Process::prepare`]
/// so any future stateful domain block can pre-allocate. Generous so it
/// covers worst-case interleaved buffers (8192 frames × 8 channels).
/// [`Gain::process`] is stateless and ignores the value; the call is
/// here to exercise the prepare→process lifecycle per the trait
/// contract. Phase C+ will plumb the actual cpal buffer size through.
const MAX_BLOCK_SIZE: usize = 8192 * 8;

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

    // Construct + prepare the domain Gain block. Default is 0 dB → unity,
    // so output is bit-identical to input; the point is exercising the
    // C2 callback ↔ domain seam, not the audio's audible result.
    // `prepare` may allocate per the trait contract, so it runs on the
    // main thread before the audio threads start. Inside the callback
    // we only call `process`, which is A2-clean.
    let mut gain_block = Gain {
        db: Decibels::default(),
    };
    gain_block.prepare(SampleRate::new(config.sample_rate as f32), MAX_BLOCK_SIZE);
    gain_block.reset();

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
        // Drain the ring into the output slice. `data.iter_mut()` is
        // explicit (vs `for sample in data`, which would consume the
        // slice and prevent the `gain_block.process(data)` call below).
        for sample in data.iter_mut() {
            *sample = match consumer.pop() {
                Ok(s) => s,
                Err(_) => {
                    fell_behind = true;
                    0.0
                }
            };
        }
        // Phase B: the domain chain. Currently just Gain at 0 dB → a
        // no-op multiplication, but a real call into the domain seam.
        // A2-clean: `Gain::process` is a tight numeric loop, no alloc /
        // lock / syscall.
        gain_block.process(data);
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
