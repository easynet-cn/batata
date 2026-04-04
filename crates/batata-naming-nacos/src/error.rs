//! Nacos naming error types

/// Errors specific to Nacos naming operations
#[derive(Debug, thiserror::Error)]
pub enum NacosNamingError {
    #[error("Service not found: {namespace}@@{group}@@{service}")]
    ServiceNotFound {
        namespace: String,
        group: String,
        service: String,
    },

    #[error("Instance not found: {0}")]
    InstanceNotFound(String),

    #[error("Instance already exists: {0}")]
    InstanceAlreadyExists(String),

    #[error("Namespace not found: {0}")]
    NamespaceNotFound(String),

    #[error("Invalid service name: {0}")]
    InvalidServiceName(String),

    #[error("Invalid instance: {0}")]
    InvalidInstance(String),

    #[error("Cluster not found: {0}")]
    ClusterNotFound(String),

    #[error("Heartbeat timeout for instance: {0}")]
    HeartbeatTimeout(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl NacosNamingError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::ServiceNotFound { .. } => 404,
            Self::InstanceNotFound(_) => 404,
            Self::InstanceAlreadyExists(_) => 409,
            Self::NamespaceNotFound(_) => 404,
            Self::InvalidServiceName(_) => 400,
            Self::InvalidInstance(_) => 400,
            Self::ClusterNotFound(_) => 404,
            Self::HeartbeatTimeout(_) => 408,
            Self::PermissionDenied(_) => 403,
            Self::RateLimitExceeded => 429,
            Self::Storage(_) => 500,
            Self::Internal(_) => 500,
        }
    }
}

impl From<batata_foundation::FoundationError> for NacosNamingError {
    fn from(e: batata_foundation::FoundationError) -> Self {
        NacosNamingError::Storage(e.to_string())
    }
}
