use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConsulError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("Not found")]
    NotFound,

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

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

pub type Result<T> = std::result::Result<T, ConsulError>;
