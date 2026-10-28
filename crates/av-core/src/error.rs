//! Recoverable engine errors; malformed user input must never cause a panic.

/// Failure from input validation or pipeline execution.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CoreError {
    /// Dimensions exceed the configured pixel budget.
    #[error("image {w}×{h} exceeds the {limit} pixel limit")]
    ImageTooLarge { w: u32, h: u32, limit: u64 },
    /// At least one dimension is zero.
    #[error("image is empty")]
    EmptyImage,
    /// The caller requested cooperative cancellation.
    #[error("operation cancelled")]
    Cancelled,
    /// Parameters, dimensions or pixel buffer do not satisfy the contract.
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    /// Engine invariant failed; indicates an implementation defect.
    #[error("internal error: {0}")]
    Internal(String),
}
