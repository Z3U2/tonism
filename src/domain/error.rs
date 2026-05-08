/// Errors that can be produced by the domain layer.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    /// The supplied sample rate is not supported or out of valid range.
    #[error("invalid sample rate: {0}")]
    InvalidSampleRate(u32),
}
