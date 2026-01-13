//! Batata Common - Shared types, traits, and utilities
//!
//! This crate provides the foundational types used across all Batata components:
//! - Error types and error codes
//! - Context traits for dependency injection
//! - Utility functions
//! - Common constants
//! - Configuration encryption

pub mod crypto;
pub mod error;
pub mod traits;
pub mod utils;

// Re-exports for convenience
pub use error::{AppError, BatataError, ErrorCode};
pub use traits::*;
pub use utils::{is_valid, local_ip};

/// Default namespace ID used when no namespace is specified
pub const DEFAULT_NAMESPACE_ID: &str = "public";

/// Default group name
pub const DEFAULT_GROUP: &str = "DEFAULT_GROUP";

/// Query parameter names
pub const TENANT: &str = "tenant";
pub const NAMESPACE_ID: &str = "namespaceId";
pub const GROUP: &str = "group";
pub const GROUP_NAME: &str = "groupName";
pub const DATA_ID: &str = "dataId";
pub const SERVICE_NAME: &str = "serviceName";

/// Action types for permission control
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionTypes {
    #[default]
    Read,
    Write,
}

impl ActionTypes {
    pub fn as_str(self) -> &'static str {
        match self {
            ActionTypes::Read => "r",
            ActionTypes::Write => "w",
        }
    }
}

impl std::fmt::Display for ActionTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ActionTypes {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "r" => Ok(ActionTypes::Read),
            "w" => Ok(ActionTypes::Write),
            _ => Err(format!("Invalid action: {}", s)),
        }
    }
}

/// Signature types for different service modules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignType {
    #[default]
    Naming,
    Config,
    Lock,
    Ai,
    Console,
    Specified,
}

impl SignType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SignType::Naming => "naming",
            SignType::Config => "config",
            SignType::Lock => "lock",
            SignType::Ai => "ai",
            SignType::Console => "console",
            SignType::Specified => "specified",
        }
    }
}

impl std::fmt::Display for SignType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for SignType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "naming" => Ok(SignType::Naming),
            "config" => Ok(SignType::Config),
            "lock" => Ok(SignType::Lock),
            "ai" => Ok(SignType::Ai),
            "console" => Ok(SignType::Console),
            "specified" => Ok(SignType::Specified),
            _ => Err(format!("Invalid sign type: {}", s)),
        }
    }
}

/// API access types with different permission levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ApiType {
    AdminApi,
    ConsoleApi,
    #[default]
    OpenApi,
    InnerApi,
}

impl ApiType {
    pub fn description(&self) -> &'static str {
        match self {
            ApiType::AdminApi => "ADMIN_API",
            ApiType::ConsoleApi => "CONSOLE_API",
            ApiType::OpenApi => "OPEN_API",
            ApiType::InnerApi => "INNER_API",
        }
    }
}

impl std::fmt::Display for ApiType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl std::str::FromStr for ApiType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ADMIN_API" => Ok(ApiType::AdminApi),
            "CONSOLE_API" => Ok(ApiType::ConsoleApi),
            "OPEN_API" => Ok(ApiType::OpenApi),
            "INNER_API" => Ok(ApiType::InnerApi),
            _ => Err(format!("Invalid API type: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_types() {
        assert_eq!(ActionTypes::default(), ActionTypes::Read);
        assert_eq!(ActionTypes::Read.as_str(), "r");
        assert_eq!(ActionTypes::Write.as_str(), "w");
        assert_eq!("r".parse::<ActionTypes>().unwrap(), ActionTypes::Read);
        assert_eq!("w".parse::<ActionTypes>().unwrap(), ActionTypes::Write);
    }

    #[test]
    fn test_sign_type() {
        assert_eq!(SignType::default(), SignType::Naming);
        assert_eq!(SignType::Config.as_str(), "config");
        assert_eq!("config".parse::<SignType>().unwrap(), SignType::Config);
    }

    #[test]
    fn test_api_type() {
        assert_eq!(ApiType::default(), ApiType::OpenApi);
        assert_eq!(ApiType::AdminApi.description(), "ADMIN_API");
        assert_eq!("ADMIN_API".parse::<ApiType>().unwrap(), ApiType::AdminApi);
    }
}
