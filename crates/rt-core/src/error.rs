// TODO: Implementation pending

use thiserror::Error;

/// Top-level error type for the rt-core crate and dependents.
#[derive(Debug, Error)]
pub enum RtError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("schema error: {0}")]
    Schema(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience Result alias used across the workspace.
pub type Result<T> = std::result::Result<T, RtError>;
