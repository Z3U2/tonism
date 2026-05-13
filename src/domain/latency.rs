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

/// Compute round-trip latency by O(N·M) cross-correlation.
///
/// # Arguments
///
/// * `reference` — the emitted reference impulse (e.g. a Kronecker delta).
/// * `loopback`  — the captured loopback signal; must be at least as long as `reference`.
/// * `sr`        — the session sample rate; must be positive.
///
/// # Returns
///
/// `Ok(LatencyMs)` — the delay between `reference` and its echo in `loopback`,
/// expressed in milliseconds rounded to one decimal place.
///
/// # Errors
///
/// * [`DomainError::InvalidSampleRate`] — `sr` is zero or negative.
/// * [`DomainError::LoopbackTooShort`]  — `loopback` is shorter than `reference`
///   (includes the degenerate case where `reference` is empty).
/// * [`DomainError::LatencyNoPeak`]     — the loopback is silent (peak < 1e-9) or
///   the best correlation coefficient falls below 10 % of the expected amplitude
///   for a clean impulse echo (indicates no usable signal).
pub fn measure_latency(
    reference: &[f32],
    loopback: &[f32],
    sr: SampleRate,
) -> Result<LatencyMs, DomainError> {
    if sr.value() <= 0.0 {
        return Err(DomainError::InvalidSampleRate(sr.value().max(0.0) as u32));
    }
    if reference.is_empty() || loopback.len() < reference.len() {
        return Err(DomainError::LoopbackTooShort);
    }

    // Silence check: if there is no energy in the loopback, cross-correlation
    // would yield 0.0 and the threshold formula would trivially pass (0 < 0).
    let loopback_peak = loopback.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
    if loopback_peak < 1e-9 {
        return Err(DomainError::LatencyNoPeak);
    }

    let max_lag = loopback.len() - reference.len();
    let mut best_lag: usize = 0;
    let mut best_corr: f32 = 0.0;

    for lag in 0..=max_lag {
        let mut corr: f32 = 0.0;
        for (i, &r) in reference.iter().enumerate() {
            corr += r * loopback[lag + i];
        }
        if corr.abs() > best_corr.abs() {
            best_corr = corr;
            best_lag = lag;
        }
    }

    let reference_energy: f32 = reference.iter().map(|x| x.abs()).sum();
    let threshold = 0.1 * loopback_peak * reference_energy;
    if best_corr.abs() < threshold {
        return Err(DomainError::LatencyNoPeak);
    }

    let ms_raw = (best_lag as f32 / sr.value()) * 1000.0;
    Ok(LatencyMs::new(ms_raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::error::DomainError;

    fn kronecker_impulse(n: usize) -> Vec<f32> {
        let mut v = vec![0.0; n];
        if n > 0 {
            v[0] = 1.0;
        }
        v
    }

    fn round_to_1dp(x: f32) -> f32 {
        (x * 10.0).round() / 10.0
    }

    #[test]
    fn measure_latency_recovers_synthetic_delay_at_boundary_rates() {
        let delays: &[usize] = &[0, 32, 256, 2048];
        let sample_rates: &[f32] = &[44_100.0, 48_000.0, 88_200.0, 96_000.0];

        let reference = kronecker_impulse(1024);

        for &delay in delays {
            for &sr in sample_rates {
                let mut loopback = vec![0.0_f32; 8192];
                for (i, &v) in reference.iter().enumerate() {
                    loopback[delay + i] = v;
                }

                let result = measure_latency(&reference, &loopback, SampleRate::new(sr));
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
        let reference = kronecker_impulse(1024);
        let loopback = vec![0.0_f32; 8192];
        let result = measure_latency(&reference, &loopback, SampleRate::new(48_000.0));
        assert!(
            matches!(result, Err(DomainError::LatencyNoPeak)),
            "expected LatencyNoPeak, got {result:?}"
        );
    }

    #[test]
    fn loopback_shorter_than_reference_returns_too_short() {
        let reference = kronecker_impulse(1024);
        let loopback = vec![0.0_f32; 512];
        let result = measure_latency(&reference, &loopback, SampleRate::new(48_000.0));
        assert!(
            matches!(result, Err(DomainError::LoopbackTooShort)),
            "expected LoopbackTooShort, got {result:?}"
        );
    }

    #[test]
    fn sample_rate_zero_returns_invalid_sample_rate() {
        let reference = kronecker_impulse(1024);
        let mut loopback = vec![0.0_f32; 8192];
        for (i, &v) in reference.iter().enumerate() {
            loopback[i] = v;
        }
        let result = measure_latency(&reference, &loopback, SampleRate::new(0.0));
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
}
