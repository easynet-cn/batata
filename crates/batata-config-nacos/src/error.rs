//! Nacos config error types

/// Errors specific to Nacos configuration operations
#[derive(Debug, thiserror::Error)]
pub enum NacosConfigError {
    #[error("Config not found: {namespace}:{group}:{data_id}")]
    NotFound {
        namespace: String,
        group: String,
        data_id: String,
    },

    #[error("Config already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid config: {0}")]
    InvalidConfig(String),

    #[error("Namespace not found: {0}")]
    NamespaceNotFound(String),

    #[error("Version conflict: expected {expected}, got {actual}")]
    VersionConflict { expected: u64, actual: u64 },

    #[error("Gray rule invalid: {0}")]
    InvalidGrayRule(String),

    #[error("Import error: {0}")]
    ImportError(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Content too large: {size} bytes (max: {max})")]
    ContentTooLarge { size: u64, max: u64 },

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl NacosConfigError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::NotFound { .. } => 404,
            Self::AlreadyExists(_) => 409,
            Self::InvalidConfig(_) => 400,
            Self::NamespaceNotFound(_) => 404,
            Self::VersionConflict { .. } => 409,
            Self::InvalidGrayRule(_) => 400,
            Self::ImportError(_) => 400,
            Self::EncryptionError(_) => 500,
            Self::PermissionDenied(_) => 403,
            Self::RateLimitExceeded => 429,
            Self::ContentTooLarge { .. } => 413,
            Self::Storage(_) => 500,
            Self::Internal(_) => 500,
        }
    }
}

impl From<batata_foundation::FoundationError> for NacosConfigError {
    fn from(e: batata_foundation::FoundationError) -> Self {
        NacosConfigError::Storage(e.to_string())
    }
}
