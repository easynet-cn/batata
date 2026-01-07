//! Context traits for dependency injection
//!
//! These traits abstract away concrete implementations, allowing
//! different crates to depend only on the traits they need.

use std::sync::Arc;

/// Database access context trait
///
/// Implementations provide access to the database connection.
/// This allows services to work with any type that can provide
/// database access without depending on concrete types.
pub trait DbContext: Send + Sync {
    /// Get the database URL or identifier
    fn db_url(&self) -> &str;
}

/// Configuration access context trait
///
/// Provides access to application configuration values
/// that are needed by various services.
pub trait ConfigContext: Send + Sync {
    /// Maximum allowed content size for configurations
    fn max_content(&self) -> u64;

    /// Whether authentication is enabled
    fn auth_enabled(&self) -> bool;

    /// JWT token expiration time in seconds
    fn token_expire_seconds(&self) -> i64;

    /// Secret key for JWT signing
    fn secret_key(&self) -> &str;

    /// Get the server's main port
    fn main_port(&self) -> u16;

    /// Get the console port
    fn console_port(&self) -> u16;
}

/// Cluster member information
#[derive(Debug, Clone)]
pub struct MemberInfo {
    pub ip: String,
    pub port: u16,
    pub address: String,
    pub state: MemberState,
}

/// Member state in the cluster
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberState {
    Up,
    Down,
    Suspicious,
}

impl std::fmt::Display for MemberState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemberState::Up => write!(f, "UP"),
            MemberState::Down => write!(f, "DOWN"),
            MemberState::Suspicious => write!(f, "SUSPICIOUS"),
        }
    }
}

/// Cluster management context trait
///
/// Provides access to cluster state and member management.
pub trait ClusterContext: Send + Sync {
    /// Check if running in standalone mode
    fn is_standalone(&self) -> bool;

    /// Check if this node is the leader
    fn is_leader(&self) -> bool;

    /// Get the leader's address if known
    fn leader_address(&self) -> Option<String>;

    /// Get all cluster members
    fn all_members(&self) -> Vec<MemberInfo>;

    /// Get only healthy cluster members
    fn healthy_members(&self) -> Vec<MemberInfo>;

    /// Get the current member count
    fn member_count(&self) -> usize;
}

/// Console data source trait
///
/// Abstracts console operations that can be performed either
/// locally (direct database access) or remotely (via HTTP to leader).
#[async_trait::async_trait]
pub trait ConsoleDataSource: Send + Sync {
    /// Check if this is a remote data source
    fn is_remote(&self) -> bool;

    // Namespace operations

    /// Find all namespaces
    async fn namespace_find_all(&self) -> Vec<NamespaceInfo>;

    /// Get namespace by ID
    async fn namespace_get_by_id(
        &self,
        namespace_id: &str,
        tenant_id: &str,
    ) -> anyhow::Result<NamespaceInfo>;

    /// Create a new namespace
    async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<()>;

    /// Update a namespace
    async fn namespace_update(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool>;

    /// Delete a namespace
    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool>;

    /// Check if a namespace exists
    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool>;
}

/// Namespace information
#[derive(Debug, Clone, Default)]
pub struct NamespaceInfo {
    pub namespace_id: String,
    pub namespace_name: String,
    pub namespace_desc: String,
    pub quota: i32,
    pub config_count: i32,
    pub namespace_type: i32,
}

/// Auth service trait for authentication operations
#[async_trait::async_trait]
pub trait AuthService: Send + Sync {
    /// Validate a JWT token and return the username
    async fn validate_token(&self, token: &str) -> anyhow::Result<String>;

    /// Check if a user has a specific permission
    async fn check_permission(
        &self,
        username: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<bool>;
}

/// Payload handler registry trait
///
/// Used to register and dispatch gRPC payload handlers.
pub trait PayloadHandlerRegistry: Send + Sync {
    /// Register a handler for a specific message type
    fn register(&self, handler: Arc<dyn PayloadHandler>);

    /// Get a handler by type name
    fn get_handler(&self, type_name: &str) -> Option<Arc<dyn PayloadHandler>>;
}

/// Payload handler trait for gRPC message handling
#[async_trait::async_trait]
pub trait PayloadHandler: Send + Sync {
    /// Get the type name this handler processes
    fn type_name(&self) -> &'static str;

    /// Handle the payload and return a response
    async fn handle(
        &self,
        request_id: &str,
        payload: &[u8],
        metadata: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_member_state_display() {
        assert_eq!(format!("{}", MemberState::Up), "UP");
        assert_eq!(format!("{}", MemberState::Down), "DOWN");
        assert_eq!(format!("{}", MemberState::Suspicious), "SUSPICIOUS");
    }

    #[test]
    fn test_namespace_info_default() {
        let ns = NamespaceInfo::default();
        assert!(ns.namespace_id.is_empty());
        assert!(ns.namespace_name.is_empty());
        assert_eq!(ns.quota, 0);
    }
}
