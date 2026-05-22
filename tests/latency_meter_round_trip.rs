//! Integration: LatencyMeter driven through BufferBackend recovers a synthetic delay.
//!
//! Per docs/standards/testing.md G2/G4: real BufferBackend (no mocks),
//! real LatencyMeter (the audio-shell block), assertions on captured behaviour
//! (G3 — behaviour, not internal call counts).

#[allow(dead_code)]
mod common;

use common::fixtures::BUFFER_SIZES;
use tonism::audio::backend::{AudioBackend, BufferBackend};
use tonism::audio::latency::{CAPTURE_LEN, IMPULSE_INTERVAL, LatencyMeter, N_IMPULSES};
use tonism::domain::error::DomainError;
use tonism::domain::latency::{DEFAULT_MIN_LAG_SAMPLES, measure_latency};
use tonism::domain::process::Process;
use tonism::domain::types::SampleRate;

/// Drive a LatencyMeter with a synthetic input that places a unit impulse at
/// `delay` samples into each chunk of the capture window, then verify that
/// `measure_latency` recovers the expected round-trip delay.
///
/// The meter captures the INBOUND buffer (the original input value, before the
/// impulse overwrite in the output).  For chunk k, the capture holds the
/// original input[k * IMPULSE_INTERVAL + delay], so placing 1.0 there ensures
/// captured[k * IMPULSE_INTERVAL + delay] == 1.0 and the algorithm finds lag
/// `delay` in each chunk.
///
/// Note: input at each `k * IMPULSE_INTERVAL` is left as 0.0 so the capture
/// records 0.0 at those positions (the meter overwrites the OUTPUT at those
/// positions with the emitted impulse, but the capture stores the original 0.0).
#[test]
fn round_trip_with_synthetic_delay_via_buffer_backend() {
    let delay: usize = 256;
    let sr = SampleRate::new(48_000.0);

    for &bs in BUFFER_SIZES {
        // Build input: for each chunk k, place 1.0 at k*IMPULSE_INTERVAL + delay.
        // Pad beyond CAPTURE_LEN so the backend drives the meter to Done.
        let total = CAPTURE_LEN + 1024;
        let mut input = vec![0.0_f32; total];
        for k in 0..N_IMPULSES {
            input[k * IMPULSE_INTERVAL + delay] = 1.0;
        }

        let mut meter = LatencyMeter::default();
        let handle = meter.handle();
        handle.request_measurement();

        let mut backend = BufferBackend::new(input, bs as usize);
        backend.run(&mut meter, sr);

        assert_eq!(
            handle.state() as u8,
            2, // CaptureState::Done
            "buffer_size {bs}: expected Done (2), got {}",
            handle.state() as u8
        );

        let mut captured = Vec::with_capacity(CAPTURE_LEN);
        handle.read_capture_into(&mut captured);
        assert_eq!(
            captured.len(),
            CAPTURE_LEN,
            "buffer_size {bs}: captured len mismatch"
        );

        let result = measure_latency(&captured, N_IMPULSES, 0, 0, sr);
        assert!(
            result.is_ok(),
            "buffer_size {bs}: measure_latency failed: {result:?}"
        );

        let ms = result.unwrap().value();
        let expected = ((delay as f32 / 48_000.0) * 1000.0 * 10.0).round() / 10.0;
        assert_eq!(
            ms, expected,
            "buffer_size {bs}: recovered {ms} ms, expected {expected} ms"
        );
    }
}

/// Validate the channel-0 deinterleave pattern used by the cpal-direct output
/// callback.  A stereo interleaved buffer with a known delay on channel 0
/// is deinterleaved, processed by LatencyMeter, and written back.  Channel 1
/// must be untouched; the meter must reach Done with a valid capture.
#[test]
fn deinterleave_round_trip_stereo() {
    let channels: usize = 2;
    let delay: usize = 256;
    let sr = SampleRate::new(48_000.0);

    for &bs in BUFFER_SIZES {
        let bs = bs as usize;
        // Build a mono ch0 signal with impulses at the expected positions.
        let total_frames = CAPTURE_LEN + 1024;
        let mut ch0_input = vec![0.0_f32; total_frames];
        for k in 0..N_IMPULSES {
            ch0_input[k * IMPULSE_INTERVAL + delay] = 1.0;
        }

        // Interleave: ch0 carries the signal, ch1 is 0.5 (sentinel).
        let total_samples = total_frames * channels;
        let mut interleaved = vec![0.0_f32; total_samples];
        for i in 0..total_frames {
            interleaved[i * channels] = ch0_input[i];
            interleaved[i * channels + 1] = 0.5;
        }

        let mut meter = LatencyMeter::default();
        let handle = meter.handle();
        meter.prepare(sr, bs);
        meter.reset();
        handle.request_measurement();

        // Simulate the cpal output callback's deinterleave pattern.
        let mut scratch = vec![0.0_f32; total_frames];
        for chunk in interleaved.chunks_mut(bs * channels) {
            let n_frames = chunk.len() / channels;
            for i in 0..n_frames {
                scratch[i] = chunk[i * channels];
            }
            meter.process(&mut scratch[..n_frames]);
            for i in 0..n_frames {
                chunk[i * channels] = scratch[i];
            }
        }

        // Channel 1 must be untouched.
        for i in 0..total_frames {
            assert!(
                (interleaved[i * channels + 1] - 0.5).abs() < 1e-9,
                "buffer_size {bs}: ch1 at frame {i} was modified"
            );
        }

        assert_eq!(
            handle.state() as u8,
            2, // CaptureState::Done
            "buffer_size {bs}: expected Done (2), got {}",
            handle.state() as u8
        );

        let mut captured = Vec::with_capacity(CAPTURE_LEN);
        handle.read_capture_into(&mut captured);

        let result = measure_latency(&captured, N_IMPULSES, 0, 0, sr);
        assert!(
            result.is_ok(),
            "buffer_size {bs}: measure_latency failed: {result:?}"
        );

        let ms = result.unwrap().value();
        let expected = ((delay as f32 / 48_000.0) * 1000.0 * 10.0).round() / 10.0;
        assert_eq!(
            ms, expected,
            "buffer_size {bs}: recovered {ms} ms, expected {expected} ms"
        );
    }
}

/// Simulate the real cpal-direct signal path: the meter emits impulses into
/// the output buffer, those impulses travel through a delay line (representing
/// ring pre-fill + hardware round-trip), then arrive back in the meter's input
/// on a later process() call.
///
/// This catches the cross-chunk bug: when the total round-trip exceeds
/// IMPULSE_INTERVAL, the echo lands in a different chunk than the one that
/// emitted it.
#[test]
fn ring_loopback_simulation() {
    let sr = SampleRate::new(48_000.0);
    let ring_delay: usize = 7200;   // 150 ms ring pre-fill
    let hw_delay: usize = 2048;     // simulated hardware round-trip
    let total_delay = ring_delay + hw_delay; // 9248 frames
    let buf_size: usize = 512;

    // Delay line: the output of the meter feeds into a FIFO, and after
    // `total_delay` samples the signal comes back as input to the meter.
    let mut delay_line = std::collections::VecDeque::new();
    // Pre-fill the delay line with silence (simulates ring pre-fill + hw).
    for _ in 0..total_delay {
        delay_line.push_back(0.0_f32);
    }

    let mut meter = LatencyMeter::default();
    let handle = meter.handle();
    meter.prepare(sr, buf_size);
    meter.reset();
    handle.request_measurement();

    let mut buf = vec![0.0_f32; buf_size];

    // Drive the meter until it reaches Done. Each iteration:
    // 1. Fill buf from the delay line output (what arrives back as "input")
    // 2. meter.process() captures that input & may emit impulses
    // 3. Push the processed output back into the delay line
    let max_iters = (CAPTURE_LEN / buf_size) + (total_delay / buf_size) + 10;
    for _ in 0..max_iters {
        // Step 1: drain delay line into buf
        for s in buf.iter_mut() {
            *s = delay_line.pop_front().unwrap_or(0.0);
        }

        // Step 2: meter processes (captures input, emits impulses)
        meter.process(&mut buf);

        // Step 3: push processed output into delay line (loopback)
        for &s in buf.iter() {
            delay_line.push_back(s);
        }

        if handle.state() as u8 == 2 {
            break;
        }
    }

    assert_eq!(
        handle.state() as u8, 2,
        "meter did not reach Done"
    );

    let mut captured = Vec::with_capacity(CAPTURE_LEN);
    handle.read_capture_into(&mut captured);

    let result = measure_latency(
        &captured,
        N_IMPULSES,
        DEFAULT_MIN_LAG_SAMPLES,
        ring_delay,
        sr,
    );
    assert!(
        result.is_ok(),
        "measure_latency failed: {result:?}"
    );

    let ms = result.unwrap().value();
    let expected_frames = hw_delay;
    let expected_ms = ((expected_frames as f32 / 48_000.0) * 1000.0 * 10.0).round() / 10.0;
    assert_eq!(
        ms, expected_ms,
        "expected {expected_ms} ms (hw_delay={hw_delay}), got {ms} ms"
    );
}

/// Same as ring_loopback_simulation but with a round-trip that fits within
/// one IMPULSE_INTERVAL (no cross-chunk wrap). Sanity check that the
/// subtraction path works for the easy case.
#[test]
fn ring_loopback_within_chunk() {
    let sr = SampleRate::new(48_000.0);
    let ring_delay: usize = 2000;
    let hw_delay: usize = 500;
    let total_delay = ring_delay + hw_delay;
    let buf_size: usize = 256;

    let mut delay_line = std::collections::VecDeque::new();
    for _ in 0..total_delay {
        delay_line.push_back(0.0_f32);
    }

    let mut meter = LatencyMeter::default();
    let handle = meter.handle();
    meter.prepare(sr, buf_size);
    meter.reset();
    handle.request_measurement();

    let mut buf = vec![0.0_f32; buf_size];
    let max_iters = (CAPTURE_LEN / buf_size) + (total_delay / buf_size) + 10;
    for _ in 0..max_iters {
        for s in buf.iter_mut() {
            *s = delay_line.pop_front().unwrap_or(0.0);
        }
        meter.process(&mut buf);
        for &s in buf.iter() {
            delay_line.push_back(s);
        }
        if handle.state() as u8 == 2 {
            break;
        }
    }

    assert_eq!(handle.state() as u8, 2, "meter did not reach Done");

    let mut captured = Vec::with_capacity(CAPTURE_LEN);
    handle.read_capture_into(&mut captured);

    let result = measure_latency(
        &captured,
        N_IMPULSES,
        DEFAULT_MIN_LAG_SAMPLES,
        ring_delay,
        sr,
    );
    assert!(result.is_ok(), "measure_latency failed: {result:?}");

    let ms = result.unwrap().value();
    let expected_ms = ((hw_delay as f32 / 48_000.0) * 1000.0 * 10.0).round() / 10.0;
    assert_eq!(ms, expected_ms, "expected {expected_ms} ms, got {ms} ms");
}

/// A completely silent loopback should yield `DomainError::LatencyNoPeak` —
/// the "no signal" sentinel path.
#[test]
fn silent_loopback_yields_no_signal() {
    let sr = SampleRate::new(48_000.0);
    // Silent input: the meter's emitted impulses are written to the OUTPUT path;
    // the capture stores the original 0.0 input — so the capture buffer is all zeros.
    let input = vec![0.0_f32; CAPTURE_LEN + 256];

    let mut meter = LatencyMeter::default();
    let handle = meter.handle();
    handle.request_measurement();

    let mut backend = BufferBackend::new(input, 128);
    backend.run(&mut meter, sr);

    let mut captured = Vec::with_capacity(CAPTURE_LEN);
    handle.read_capture_into(&mut captured);

    let result = measure_latency(&captured, N_IMPULSES, DEFAULT_MIN_LAG_SAMPLES, 0, sr);
    assert!(
        matches!(result, Err(DomainError::LatencyNoPeak)),
        "expected LatencyNoPeak on silent loopback, got {result:?}"
    );
}
