use crate::domain::error::DomainError;
use crate::domain::types::SampleRate;

/// Round-trip latency in milliseconds, rounded to one decimal place.
///
/// The inner value is stored pre-rounded via [`LatencyMs::new`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatencyMs(f32);

impl LatencyMs {
    /// Construct a [`LatencyMs`], rounding `value` to one decimal place.
    ///
    /// Note: cannot be `const fn` because `f32::round` is not const-stable.
    pub fn new(value: f32) -> Self {
        Self((value * 10.0).round() / 10.0)
    }

    /// Return the rounded millisecond value.
    pub fn value(self) -> f32 {
        self.0
    }
}

/// Minimum lag (in samples) to skip at the start of each chunk when searching
/// for the peak.  Skipping the first few samples avoids DC/click artifacts that
/// could be mistaken for an impulse echo.
pub const DEFAULT_MIN_LAG_SAMPLES: usize = 16;

/// Maximum number of impulses (chunks) supported on the stack inside
/// [`measure_latency`].  The caller passes `n_impulses ≤ MAX_IMPULSES`.
const MAX_IMPULSES: usize = 8;

/// Compute round-trip latency from a multi-impulse loopback capture.
///
/// The audio meter emits `n_impulses` unit impulses spaced evenly across the
/// capture window (one at the start of each equal-length chunk).  This function
/// splits the loopback into `n_impulses` equal chunks, finds the strongest peak
/// in each chunk (ignoring the first `min_lag_samples` to avoid DC artifacts),
/// takes the median lag across all chunks, and accepts the measurement when a
/// strict majority of chunks agree with the median within
/// `max(median / 10, 2)` samples.  A single noise-dominated chunk does not
/// corrupt the result; only when fewer than `n_impulses / 2 + 1` chunks
/// converge does the function reject the capture as noise-dominated.
///
/// `ring_latency_samples` is the known internal ring-buffer pre-fill in frames.
/// It is subtracted from the median lag so the result reflects hardware +
/// processing latency only.
///
/// # Arguments
///
/// * `loopback`              — the captured loopback signal; must satisfy
///   `loopback.len() / n_impulses > min_lag_samples`.
/// * `n_impulses`            — number of impulses emitted (1 …= [`MAX_IMPULSES`]).
/// * `min_lag_samples`       — samples to skip at the start of each chunk (typically
///   [`DEFAULT_MIN_LAG_SAMPLES`]).
/// * `ring_latency_samples`  — known ring-buffer pre-fill to subtract (frames).
/// * `sr`                    — the session sample rate; must be positive.
///
/// # Returns
///
/// `Ok(LatencyMs)` — the median per-chunk peak lag minus the ring latency,
/// expressed in milliseconds, rounded to one decimal place.
///
/// # Errors
///
/// * [`DomainError::InvalidSampleRate`] — `sr` is zero or negative.
/// * [`DomainError::LoopbackTooShort`]  — `n_impulses` is out of range, or the
///   chunk length does not exceed `min_lag_samples`.
/// * [`DomainError::LatencyNoPeak`]     — any chunk is silent (peak < 1e-6), or
///   the per-chunk lags disagree by more than the tolerance (indicates a
///   noise-dominated capture).
pub fn measure_latency(
    loopback: &[f32],
    n_impulses: usize,
    min_lag_samples: usize,
    ring_latency_samples: usize,
    sr: SampleRate,
) -> Result<LatencyMs, DomainError> {
    // --- Validation ---
    if sr.value() <= 0.0 {
        return Err(DomainError::InvalidSampleRate(sr.value().max(0.0) as u32));
    }
    if n_impulses == 0 || n_impulses > MAX_IMPULSES {
        return Err(DomainError::LoopbackTooShort);
    }
    let chunk_len = loopback.len() / n_impulses;
    if chunk_len <= min_lag_samples {
        return Err(DomainError::LoopbackTooShort);
    }

    // --- Per-chunk argmax ---
    let mut lags = [0usize; MAX_IMPULSES];

    for (k, lag_slot) in lags.iter_mut().enumerate().take(n_impulses) {
        let start = k * chunk_len;
        let mut best_lag = min_lag_samples;
        let mut best_amp = 0.0_f32;

        for lag in min_lag_samples..chunk_len {
            let a = loopback[start + lag].abs();
            if a > best_amp {
                best_amp = a;
                best_lag = lag;
            }
        }

        if best_amp < 1e-6 {
            return Err(DomainError::LatencyNoPeak);
        }

        *lag_slot = best_lag;
    }

    // --- Median (in-place sort of the first n_impulses entries) ---
    lags[..n_impulses].sort_unstable();

    let median_lag = if n_impulses % 2 == 1 {
        lags[n_impulses / 2]
    } else {
        (lags[n_impulses / 2 - 1] + lags[n_impulses / 2]) / 2
    };

    // --- Agreement check (strict majority rule) ---
    //
    // Real loopback captures routinely have one noise-dominated chunk per
    // measurement (random transient louder than the impulse echo).  Requiring
    // every chunk to agree throws away the whole measurement on a single
    // outlier.  Strict majority — `n / 2 + 1` chunks within tolerance —
    // tolerates one outlier per 4 chunks while still rejecting captures where
    // no consistent peak exists.
    let tolerance = (median_lag / 10).max(2);
    let agreeing = lags[..n_impulses]
        .iter()
        .filter(|&&lag| lag.abs_diff(median_lag) <= tolerance)
        .count();
    let required = (n_impulses / 2) + 1;
    if agreeing < required {
        return Err(DomainError::LatencyNoPeak);
    }

    let adjusted_lag = median_lag.saturating_sub(ring_latency_samples);
    let ms_raw = (adjusted_lag as f32 / sr.value()) * 1000.0;
    Ok(LatencyMs::new(ms_raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::error::DomainError;

    fn round_to_1dp(x: f32) -> f32 {
        (x * 10.0).round() / 10.0
    }

    /// Build a loopback of `n_impulses` chunks of `chunk_len` each, with a
    /// unit impulse at `delay` within every chunk.
    fn build_multi_impulse_loopback(n_impulses: usize, chunk_len: usize, delay: usize) -> Vec<f32> {
        let mut v = vec![0.0_f32; n_impulses * chunk_len];
        for k in 0..n_impulses {
            v[k * chunk_len + delay] = 1.0;
        }
        v
    }

    #[test]
    fn measure_latency_recovers_synthetic_delay_at_boundary_rates() {
        // delay = 0 excluded: new algorithm skips lag < min_lag_samples.
        // min_lag_samples = 0 here so the smallest useful delay is 32.
        let delays: &[usize] = &[32, 256, 512];
        let sample_rates: &[f32] = &[44_100.0, 48_000.0, 88_200.0, 96_000.0];

        for &delay in delays {
            for &sr in sample_rates {
                let loopback = build_multi_impulse_loopback(4, 2048, delay);

                let result = measure_latency(&loopback, 4, 0, 0, SampleRate::new(sr));
                assert!(
                    result.is_ok(),
                    "expected Ok for delay={delay} sr={sr}, got {result:?}"
                );
                let expected = round_to_1dp((delay as f32 / sr) * 1000.0);
                let got = result.unwrap().value();
                assert_eq!(
                    got, expected,
                    "delay={delay} sr={sr}: expected {expected} ms, got {got} ms"
                );
            }
        }
    }

    #[test]
    fn silent_loopback_returns_no_peak() {
        let loopback = vec![0.0_f32; 8192];
        let result = measure_latency(&loopback, 4, 16, 0, SampleRate::new(48_000.0));
        assert!(
            matches!(result, Err(DomainError::LatencyNoPeak)),
            "expected LatencyNoPeak, got {result:?}"
        );
    }

    #[test]
    fn loopback_shorter_than_min_lag_returns_too_short() {
        let loopback = vec![0.0_f32; 64];
        let result = measure_latency(&loopback, 4, 100, 0, SampleRate::new(48_000.0));
        assert!(
            matches!(result, Err(DomainError::LoopbackTooShort)),
            "expected LoopbackTooShort, got {result:?}"
        );
    }

    #[test]
    fn sample_rate_zero_returns_invalid_sample_rate() {
        let loopback = build_multi_impulse_loopback(4, 2048, 32);
        let result = measure_latency(&loopback, 4, 16, 0, SampleRate::new(0.0));
        assert!(
            matches!(result, Err(DomainError::InvalidSampleRate(0))),
            "expected InvalidSampleRate(0), got {result:?}"
        );
    }

    #[test]
    fn latency_ms_new_rounds_to_one_decimal() {
        assert_eq!(LatencyMs::new(7.36).value(), 7.4_f32);
        assert_eq!(LatencyMs::new(0.04).value(), 0.0_f32);
    }

    #[test]
    fn three_of_four_chunks_agreeing_accepts_majority() {
        // Strict-majority rule: with n=4 we need >= 3 chunks within tolerance.
        // Chunk 0: noise outlier at lag 100.
        // Chunks 1-3: real echo at lag 800.
        // Sorted lags: [100, 800, 800, 800] → median = 800, tolerance = 80.
        // Agreeing count = 3 ≥ required (3).  Returns Ok(median 800).
        let chunk_len = 1024;
        let n_impulses = 4;
        let mut loopback = vec![0.0_f32; n_impulses * chunk_len];
        loopback[100] = 1.0;
        loopback[chunk_len + 800] = 1.0;
        loopback[2 * chunk_len + 800] = 1.0;
        loopback[3 * chunk_len + 800] = 1.0;

        let result = measure_latency(&loopback, n_impulses, 0, 0, SampleRate::new(48_000.0));
        assert!(
            result.is_ok(),
            "expected Ok with 3-of-4 majority, got {result:?}"
        );
        let expected = round_to_1dp((800.0_f32 / 48_000.0) * 1000.0);
        assert_eq!(result.unwrap().value(), expected);
    }

    #[test]
    fn split_chunk_measurements_return_no_peak() {
        // No majority: 2 chunks at lag 100, 2 chunks at lag 800.
        // Sorted: [100, 100, 800, 800] → median = (100 + 800)/2 = 450,
        // tolerance = 45.  Each lag is 350 away from median → agreeing = 0.
        // Below required (3) → LatencyNoPeak.
        let chunk_len = 1024;
        let n_impulses = 4;
        let mut loopback = vec![0.0_f32; n_impulses * chunk_len];
        loopback[100] = 1.0;
        loopback[chunk_len + 100] = 1.0;
        loopback[2 * chunk_len + 800] = 1.0;
        loopback[3 * chunk_len + 800] = 1.0;

        let result = measure_latency(&loopback, n_impulses, 0, 0, SampleRate::new(48_000.0));
        assert!(
            matches!(result, Err(DomainError::LatencyNoPeak)),
            "expected LatencyNoPeak for 2/2 split, got {result:?}"
        );
    }

    #[test]
    fn ring_latency_subtracted_from_result() {
        let delay = 7200usize;
        let ring = 7000usize;
        let loopback = build_multi_impulse_loopback(4, 8192, delay);
        let result = measure_latency(&loopback, 4, 0, ring, SampleRate::new(48_000.0));
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        let expected = round_to_1dp(((delay - ring) as f32 / 48_000.0) * 1000.0);
        assert_eq!(result.unwrap().value(), expected);
    }

    #[test]
    fn ring_latency_saturates_to_zero() {
        let delay = 100usize;
        let ring = 7000usize;
        let loopback = build_multi_impulse_loopback(4, 8192, delay);
        let result = measure_latency(&loopback, 4, 0, ring, SampleRate::new(48_000.0));
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        assert_eq!(result.unwrap().value(), 0.0);
    }
}
