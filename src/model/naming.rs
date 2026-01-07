// Namespace models for service discovery
// Re-exports types from batata_config for backward compatibility

// Re-export Namespace and related constants from batata_config
pub use batata_config::model::{
    Namespace, NamespaceForm, DEFAULT_NAMESPACE_DESCRIPTION, DEFAULT_NAMESPACE_ID,
    DEFAULT_NAMESPACE_QUOTA, DEFAULT_NAMESPACE_SHOW_NAME,
};

// Common parameters for service discovery.
pub const CODE: &str = "code";
pub const SERVICE_NAME: &str = "serviceName";
pub const CLUSTER_NAME: &str = "clusterName";
pub const NAMESPACE_ID: &str = "namespaceId";
pub const GROUP_NAME: &str = "groupName";
pub const LIGHT_BEAT_ENABLED: &str = "lightBeatEnabled";
pub const NAMING_REQUEST_TIMEOUT: &str = "namingRequestTimeout";
