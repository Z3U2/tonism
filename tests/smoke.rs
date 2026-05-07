//! End-to-end smoke test: input vector → BufferBackend → Process impl → output.
//!
//! Per docs/standards/testing.md G2/G4: real BufferBackend (no mocks),
//! real Process impl (the domain Gain block), assertions on processed audio
//! (G3 — behaviour, not internal call counts).

mod common;

use common::fixtures::{kronecker_impulse, BUFFER_SIZES, SAMPLE_RATES};
use tonism::audio::backend::{AudioBackend, BufferBackend};
use tonism::domain::blocks::gain::Gain;
use tonism::domain::types::{Decibels, SampleRate};

#[test]
fn unity_gain_passes_kronecker_impulse_unchanged() {
    let input = kronecker_impulse(1024);
    let mut backend = BufferBackend::new(input.clone(), 128);
    let mut gain = Gain { db: Decibels(0.0) };
    backend.run(&mut gain, SampleRate(48_000));
    let out = backend.into_output();
    assert_eq!(out.len(), 1024);
    assert!(
        (out[0] - 1.0).abs() < 1e-6,
        "impulse peak should pass through unity gain"
    );
    for &s in &out[1..] {
        assert!(s.abs() < 1e-6, "non-peak samples should remain zero");
    }
}

#[test]
fn boundary_buffer_sizes_all_pass_unity_gain() {
    let input = kronecker_impulse(2048);
    let gain_db = Decibels(0.0);
    for &bs in BUFFER_SIZES {
        let mut backend = BufferBackend::new(input.clone(), bs as usize);
        let mut gain = Gain { db: gain_db };
        backend.run(&mut gain, SampleRate(48_000));
        let out = backend.into_output();
        assert_eq!(
            out.len(),
            input.len(),
            "buffer_size {bs}: output length mismatch"
        );
        assert!(
            (out[0] - 1.0).abs() < 1e-6,
            "buffer_size {bs}: impulse peak missing"
        );
    }
}

#[test]
fn boundary_sample_rates_all_drive_gain_block() {
    // Gain is sample-rate independent, but this verifies the reset() is called
    // for every supported rate without panicking.
    let input = kronecker_impulse(256);
    for &sr in SAMPLE_RATES {
        let mut backend = BufferBackend::new(input.clone(), 64);
        let mut gain = Gain { db: Decibels(-6.0) };
        backend.run(&mut gain, SampleRate(sr));
        let out = backend.into_output();
        assert_eq!(
            out.len(),
            input.len(),
            "sample_rate {sr}: output length mismatch"
        );
        // -6 dB ≈ 0.501; check the impulse peak.
        let expected = 10f32.powf(-6.0 / 20.0);
        assert!(
            (out[0] - expected).abs() < 1e-3,
            "sample_rate {sr}: peak {} != expected {}",
            out[0],
            expected
        );
    }
}
