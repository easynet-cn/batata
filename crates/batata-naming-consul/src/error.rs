//! Consul naming error types

/// Errors specific to Consul naming operations
#[derive(Debug, thiserror::Error)]
pub enum ConsulNamingError {
    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Check not found: {0}")]
    CheckNotFound(String),

    #[error("Service ID not found: {0}")]
    ServiceIdNotFound(String),

    #[error("Invalid service registration: {0}")]
    InvalidRegistration(String),

    #[error("Invalid check definition: {0}")]
    InvalidCheck(String),

    #[error("Datacenter not found: {0}")]
    DatacenterNotFound(String),

    #[error("ACL denied: {0}")]
    AclDenied(String),

    #[error("CAS conflict: index {expected} != {actual}")]
    CasConflict { expected: u64, actual: u64 },

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ConsulNamingError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::ServiceNotFound(_) => 404,
            Self::NodeNotFound(_) => 404,
            Self::CheckNotFound(_) => 404,
            Self::ServiceIdNotFound(_) => 404,
            Self::InvalidRegistration(_) => 400,
            Self::InvalidCheck(_) => 400,
            Self::DatacenterNotFound(_) => 404,
            Self::AclDenied(_) => 403,
            Self::CasConflict { .. } => 409,
            Self::Storage(_) => 500,
            Self::Internal(_) => 500,
        }
    }
}

impl From<batata_foundation::FoundationError> for ConsulNamingError {
    fn from(e: batata_foundation::FoundationError) -> Self {
        ConsulNamingError::Storage(e.to_string())
    }
}
