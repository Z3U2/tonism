//! Shared test fixtures: reference signals + boundary parameter lists.
//!
//! Per docs/specs/mvp/dependencies.md "Test data" and docs/standards/testing.md G7.

use tonism::domain::types::SampleRate;

/// Boundary buffer sizes covering the cpal block-size spectrum (G7).
pub const BUFFER_SIZES: &[u32] = &[32, 64, 128, 256, 512, 1024, 2048];

/// Boundary sample rates for gigging-rig norms (44.1 / 48 / 88.2 / 96 kHz).
pub const SAMPLE_RATES: &[u32] = &[44_100, 48_000, 88_200, 96_000];

/// 1024-sample Kronecker impulse: one 1.0 followed by 1023 zeros.
/// Used as the AC1 latency reference signal.
pub fn kronecker_impulse(n: usize) -> Vec<f32> {
    let mut v = vec![0.0; n];
    if n > 0 {
        v[0] = 1.0;
    }
    v
}

/// `secs` seconds of silence at `sr`.  Used for the AC2 5-min idle-path fixture.
#[allow(dead_code)]
pub fn silent_buffer(secs: f32, sr: SampleRate) -> Vec<f32> {
    let n = (secs * sr.0 as f32) as usize;
    vec![0.0; n]
}
