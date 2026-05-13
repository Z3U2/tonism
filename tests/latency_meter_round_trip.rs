//! Integration: LatencyMeter driven through BufferBackend recovers a synthetic delay.
//!
//! Per docs/standards/testing.md G2/G4: real BufferBackend (no mocks),
//! real LatencyMeter (the audio-shell block), assertions on captured behaviour
//! (G3 — behaviour, not internal call counts).

#[allow(dead_code)]
mod common;

use common::fixtures::BUFFER_SIZES;
use tonism::audio::backend::{AudioBackend, BufferBackend};
use tonism::audio::latency::{CAPTURE_LEN, KRONECKER_REF, LatencyMeter};
use tonism::domain::error::DomainError;
use tonism::domain::latency::measure_latency;
use tonism::domain::types::SampleRate;

/// Drive a LatencyMeter with a synthetic input that places a Kronecker impulse
/// at `delay` samples into a CAPTURE_LEN-long capture window, then verify that
/// `measure_latency` recovers the expected round-trip delay.
///
/// The meter captures the INBOUND buffer (unchanged — before the impulse
/// overwrite on channel 0 of the output).  The impulse is placed in the INPUT
/// at position `delay`, so `captured[delay] == 1.0` and the correlation finds
/// the right lag.
#[test]
fn round_trip_with_synthetic_delay_via_buffer_backend() {
    let delay: usize = 256;
    let sr = SampleRate::new(48_000.0);

    for &bs in BUFFER_SIZES {
        // Input is long enough to fill the capture window past the impulse.
        // Layout: zeros(delay) | 1.0 | zeros(to fill CAPTURE_LEN + slack)
        let total = delay + CAPTURE_LEN + 1024;
        let mut input = vec![0.0_f32; total];
        // Place impulse at `delay` so the capture sees it at that index.
        input[delay] = 1.0;

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

        let result = measure_latency(&KRONECKER_REF, &captured, sr);
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
    // Silent input: the meter's own emitted impulse at sample 0 gets CAPTURED
    // as 0.0 (the original input value), so the capture buffer is all zeros.
    let input = vec![0.0_f32; CAPTURE_LEN + 256];

    let mut meter = LatencyMeter::default();
    let handle = meter.handle();
    handle.request_measurement();

    let mut backend = BufferBackend::new(input, 128);
    backend.run(&mut meter, sr);

    let mut captured = Vec::with_capacity(CAPTURE_LEN);
    handle.read_capture_into(&mut captured);

    let result = measure_latency(&KRONECKER_REF, &captured, sr);
    assert!(
        matches!(result, Err(DomainError::LatencyNoPeak)),
        "expected LatencyNoPeak on silent loopback, got {result:?}"
    );
}
