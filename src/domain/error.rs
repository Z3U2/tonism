/// Errors that can be produced by the domain layer.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    /// The supplied sample rate is not supported or out of valid range.
    #[error("invalid sample rate: {0}")]
    InvalidSampleRate(u32),
    /// The loopback contains no detectable correlation peak (e.g. silence).
    #[error("loopback contains no detectable correlation peak")]
    LatencyNoPeak,
    /// The loopback is shorter than the reference impulse.
    #[error("loopback is shorter than the reference impulse")]
    LoopbackTooShort,
}
