// Error types for MaintainerClient

/// Errors that can occur during maintainer client operations
#[derive(Debug, thiserror::Error)]
pub enum MaintainerError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("All servers failed")]
    AllServersFailed,

    #[error("Request failed with status {status}: {body}")]
    RequestFailed { status: u16, body: String },

    #[error("Token expired")]
    TokenExpired,

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
