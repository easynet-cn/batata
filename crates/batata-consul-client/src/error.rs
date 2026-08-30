//! Error types and the `Result` alias.
use thiserror::Error;

/// Errors returned by every `batata-consul-client` operation.
#[derive(Error, Debug)]
pub enum ConsulError {
    /// Transport-level HTTP error from `reqwest`.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Consul returned a non-2xx API response.
    #[error("API error (status {status}): {message}")]
    Api {
        /// The HTTP status code returned by Consul.
        status: u16,
        /// The error message returned by Consul.
        message: String,
    },

    /// The requested resource does not exist.
    #[error("Not found")]
    NotFound,

    /// JSON (de)serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// An arbitrary error carrying a message.
    #[error("{0}")]
    Other(String),
}

impl ConsulError {
    /// Returns true when the error is a 404 (explicit or Api-status 404).
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound) || matches!(self, Self::Api { status, .. } if *status == 404)
    }

    /// HTTP status code if this is an API-level error.
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } => Some(*status),
            Self::NotFound => Some(404),
            _ => None,
        }
    }
}

/// Convenience alias for `std::result::Result<T, ConsulError>`.
pub type Result<T> = std::result::Result<T, ConsulError>;
