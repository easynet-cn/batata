//! Error types for MaintainerClient

/// Errors that can occur during maintainer client operations
#[derive(Debug, thiserror::Error)]
pub enum MaintainerError {
    #[error("Authentication failed: {0}")]
    /// The `AuthFailed` variant.
    AuthFailed(String),

    #[error("All servers failed")]
    /// The `AllServersFailed` variant.
    AllServersFailed,

    #[error("Request failed with status {status}: {body}")]
    /// The `RequestFailed` variant.
    RequestFailed {
        /// The HTTP status code returned by the server.
        status: u16,
        /// The response body returned by the server.
        body: String,
    },

    #[error("Token expired")]
    /// The `TokenExpired` variant.
    TokenExpired,

    #[error("HTTP error: {0}")]
    /// The `Http` variant.
    Http(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    /// The `Serialization` variant.
    Serialization(#[from] serde_json::Error),

    #[error("{0}")]
    /// The `Other` variant.
    Other(#[from] anyhow::Error),
}
