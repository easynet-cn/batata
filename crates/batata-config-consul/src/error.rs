//! Consul KV error types

/// Errors specific to Consul KV operations
#[derive(Debug, thiserror::Error)]
pub enum ConsulKvError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("CAS conflict: expected index {expected}, current index {actual}")]
    CasConflict { expected: u64, actual: u64 },

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Session expired: {0}")]
    SessionExpired(String),

    #[error("Key locked by session: {0}")]
    KeyLocked(String),

    #[error("Transaction failed at operation {index}: {reason}")]
    TransactionFailed { index: u32, reason: String },

    #[error("Invalid key: {0}")]
    InvalidKey(String),

    #[error("Value too large: {size} bytes (max: {max})")]
    ValueTooLarge { size: u64, max: u64 },

    #[error("ACL denied: {0}")]
    AclDenied(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ConsulKvError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::KeyNotFound(_) => 404,
            Self::CasConflict { .. } => 409,
            Self::SessionNotFound(_) => 404,
            Self::SessionExpired(_) => 404,
            Self::KeyLocked(_) => 409,
            Self::TransactionFailed { .. } => 409,
            Self::InvalidKey(_) => 400,
            Self::ValueTooLarge { .. } => 413,
            Self::AclDenied(_) => 403,
            Self::Storage(_) => 500,
            Self::Internal(_) => 500,
        }
    }
}

impl From<batata_foundation::FoundationError> for ConsulKvError {
    fn from(e: batata_foundation::FoundationError) -> Self {
        ConsulKvError::Storage(e.to_string())
    }
}
