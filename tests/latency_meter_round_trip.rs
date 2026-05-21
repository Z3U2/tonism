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

        let result = measure_latency(&captured, N_IMPULSES, 0, sr);
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

        let result = measure_latency(&captured, N_IMPULSES, 0, sr);
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

    let result = measure_latency(&captured, N_IMPULSES, DEFAULT_MIN_LAG_SAMPLES, sr);
    assert!(
        matches!(result, Err(DomainError::LatencyNoPeak)),
        "expected LatencyNoPeak on silent loopback, got {result:?}"
    );
}
