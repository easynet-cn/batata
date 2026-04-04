//! Foundation-level error types

/// Foundation-level errors for infrastructure operations
#[derive(Debug, thiserror::Error)]
pub enum FoundationError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Consistency error: {0}")]
    Consistency(String),

    #[error("Cluster error: {0}")]
    Cluster(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Timeout after {0}ms")]
    Timeout(u64),

    #[error("Not leader, current leader: {0:?}")]
    NotLeader(Option<String>),

    #[error("Node not ready")]
    NotReady,

    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for FoundationError {
    fn from(e: std::io::Error) -> Self {
        FoundationError::Storage(e.to_string())
    }
}
