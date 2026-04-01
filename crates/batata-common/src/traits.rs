//! Context traits for dependency injection
//!
//! These traits abstract away concrete implementations, allowing
//! different crates to depend only on the traits they need.

use std::sync::Arc;

use crate::crypto::CryptoResult;

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
    fn secret_key(&self) -> String;

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

// ============================================================================
// Auth Plugin Trait
// ============================================================================

/// Raw token extracted by middleware, stored in request extensions.
/// The middleware only extracts the token string — all validation
/// (JWT decode, expiry check) is handled by the AuthPlugin.
#[derive(Debug, Clone, Default)]
pub struct RequestToken(pub Option<String>);

/// Identity context built from request token, enriched by AuthPlugin.
///
/// The `secured!` macro creates this from the `RequestToken` and passes
/// it to `AuthPlugin::validate_identity()` which fills in the remaining fields.
#[derive(Debug, Clone, Default)]
pub struct IdentityContext {
    /// Raw token from the request (populated by secured! macro from RequestToken)
    pub token: Option<String>,
    /// Username extracted from the token (set by AuthPlugin)
    pub username: String,
    /// Whether the identity has been successfully authenticated (set by AuthPlugin)
    pub authenticated: bool,
    /// Whether the user is a global admin (set by AuthPlugin)
    pub is_global_admin: bool,
}

/// Permission for authorization checking
#[derive(Debug, Clone)]
pub struct AuthPermission {
    /// Resource being accessed (format: "namespace:group:type/name")
    pub resource: String,
    /// Action being performed: "r" (read) or "w" (write)
    pub action: String,
}

/// Result of an authentication or authorization check
#[derive(Debug, Clone)]
pub struct AuthCheckResult {
    pub success: bool,
    pub message: Option<String>,
}

impl AuthCheckResult {
    pub fn success() -> Self {
        Self {
            success: true,
            message: None,
        }
    }

    pub fn fail(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(msg.into()),
        }
    }
}

/// Result returned by a successful login
#[derive(Debug, Clone)]
pub struct LoginResult {
    /// JWT access token
    pub token: String,
    /// Token time-to-live in seconds
    pub token_ttl: i64,
    /// Authenticated username
    pub username: String,
    /// Whether the user is a global admin
    pub is_global_admin: bool,
}

/// Auth plugin trait — the main SPI interface for pluggable authentication.
///
/// Each auth backend (nacos, ldap, oauth2) implements this trait.
/// The active plugin is selected via config key `batata.core.auth.system.type`.
///
/// Flow:
/// 1. Middleware extracts raw token → stores as `RequestToken`
/// 2. `secured!` macro builds `IdentityContext` from `RequestToken`
/// 3. `secured!` calls `validate_identity()` — plugin decodes token, checks validity,
///    sets username/is_global_admin. Handles expired/invalid/missing token errors.
/// 4. For non-admin users, `secured!` calls `validate_authority()` — plugin checks
///    if user has permission for the requested resource+action.
#[async_trait::async_trait]
pub trait AuthPlugin: Send + Sync {
    /// Plugin identifier (e.g., "nacos", "ldap", "oauth2")
    fn plugin_name(&self) -> &str;

    /// Whether this plugin supports username/password login
    fn is_login_enabled(&self) -> bool {
        true
    }

    /// Validate identity: decode token, verify validity, load user info/roles.
    ///
    /// Handles all token errors (missing, expired, invalid) internally.
    /// On success, sets `identity.authenticated`, `identity.username`,
    /// `identity.is_global_admin`.
    async fn validate_identity(&self, identity: &mut IdentityContext) -> AuthCheckResult;

    /// Authorize: check if authenticated user has permission for resource+action.
    ///
    /// Only called for non-admin users (admin bypass is handled by the caller).
    async fn validate_authority(
        &self,
        identity: &IdentityContext,
        permission: &AuthPermission,
    ) -> AuthCheckResult;

    /// Login with username/password credentials.
    ///
    /// Returns a JWT token on success, or an error message on failure.
    async fn login(&self, username: &str, password: &str) -> Result<LoginResult, String>;
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

/// Heartbeat service trait for health check tracking
///
/// Abstracts heartbeat recording and removal operations,
/// allowing AppState to hold a trait object instead of `Arc<dyn Any>`.
pub trait HeartbeatService: Send + Sync {
    /// Record a heartbeat for an instance
    #[allow(clippy::too_many_arguments)]
    fn record_heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
        heartbeat_timeout: i64,
        ip_delete_timeout: i64,
    );

    /// Remove heartbeat tracking for an instance
    fn remove_heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    );
}

// ============================================================================
// Cluster Manager Trait
// ============================================================================

/// Cluster health summary
#[derive(Clone, Debug, Default)]
pub struct ClusterHealthSummary {
    pub total: usize,
    pub up: usize,
    pub down: usize,
    pub suspicious: usize,
    pub starting: usize,
    pub isolation: usize,
}

impl ClusterHealthSummary {
    pub fn is_healthy(&self) -> bool {
        self.up > self.total / 2
    }
}

/// Extended member information with metadata
///
/// Provides richer member data than `MemberInfo`, including
/// extend_info metadata used by Consul and other plugins.
#[derive(Debug, Clone)]
pub struct ExtendedMemberInfo {
    pub ip: String,
    pub port: u16,
    pub address: String,
    pub state: MemberState,
    pub extend_info: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Cluster manager trait
///
/// Abstracts cluster membership management operations.
/// This allows plugins (Consul, Console) to depend on the trait
/// rather than the concrete `ServerMemberManager` type.
pub trait ClusterManager: Send + Sync {
    /// Check if running in standalone mode
    fn is_standalone(&self) -> bool;

    /// Check if this node is the leader
    fn is_leader(&self) -> bool;

    /// Check if the cluster is healthy (majority of nodes are up)
    fn is_cluster_healthy(&self) -> bool;

    /// Get the leader's address if known
    fn leader_address(&self) -> Option<String>;

    /// Get the local node's address
    fn local_address(&self) -> &str;

    /// Get the current member count
    fn member_count(&self) -> usize;

    /// Get all cluster members
    fn all_members_extended(&self) -> Vec<ExtendedMemberInfo>;

    /// Get only healthy cluster members
    fn healthy_members_extended(&self) -> Vec<ExtendedMemberInfo>;

    /// Get a member by address
    fn get_member(&self, address: &str) -> Option<ExtendedMemberInfo>;

    /// Get self member info
    fn get_self_member(&self) -> ExtendedMemberInfo;

    /// Get cluster health summary
    fn health_summary(&self) -> ClusterHealthSummary;

    /// Refresh self member's last refresh timestamp
    fn refresh_self(&self);

    /// Check if a given address is the local node
    fn is_self(&self, address: &str) -> bool;
}

// Note: ClientConnectionManager trait has been moved to batata-core
// to allow using the Payload type from batata-api.

/// Connection metadata for trait-based access
#[derive(Debug, Clone, Default)]
pub struct ConnectionInfo {
    pub connection_id: String,
    pub client_ip: String,
    pub client_port: u16,
    pub app_name: String,
    pub sdk: String,
    pub version: String,
    pub labels: std::collections::HashMap<String, String>,
    pub create_time: u64,
    pub last_active_time: u64,
}

// ============================================================================
// Config Subscription Service Trait
// ============================================================================

/// Key for a configuration subscription
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConfigSubscriptionKey {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
}

impl ConfigSubscriptionKey {
    pub fn new(data_id: &str, group: &str, tenant: &str) -> Self {
        Self {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
        }
    }
}

/// Information about a config subscriber
#[derive(Clone, Debug)]
pub struct ConfigSubscriberInfo {
    pub connection_id: String,
    pub client_ip: String,
    pub md5: String,
    pub client_tenant: String,
}

/// Config subscription service trait
///
/// Abstracts config subscription tracking operations.
/// This allows handlers and console to manage subscriptions
/// without depending on the concrete `ConfigSubscriberManager` type.
pub trait ConfigSubscriptionService: Send + Sync {
    /// Register a subscription for a configuration
    fn subscribe(
        &self,
        connection_id: &str,
        client_ip: &str,
        key: &ConfigSubscriptionKey,
        md5: &str,
        client_tenant: &str,
    );

    /// Unsubscribe from a specific configuration
    fn unsubscribe(&self, connection_id: &str, key: &ConfigSubscriptionKey);

    /// Unsubscribe from all configurations for a connection
    fn unsubscribe_all(&self, connection_id: &str);

    /// Get all subscribers for a specific configuration
    fn get_subscribers(&self, key: &ConfigSubscriptionKey) -> Vec<ConfigSubscriberInfo>;

    /// Get all subscribers by client IP
    fn get_subscribers_by_ip(
        &self,
        client_ip: &str,
    ) -> Vec<(ConfigSubscriptionKey, ConfigSubscriberInfo)>;

    /// Get all subscriptions
    fn get_all_subscriptions(&self) -> Vec<(ConfigSubscriptionKey, Vec<ConfigSubscriberInfo>)>;

    /// Update the MD5 for a subscriber
    fn update_md5(&self, connection_id: &str, key: &ConfigSubscriptionKey, md5: &str);

    /// Get total number of subscriptions
    fn subscription_count(&self) -> usize;

    /// Get number of unique configs being watched
    fn config_count(&self) -> usize;

    /// Get number of connections with active subscriptions
    fn subscriber_connection_count(&self) -> usize;
}

// ============================================================================
// Config Encryption Provider Trait
// ============================================================================

/// Configuration encryption provider trait
///
/// Abstracts config encryption/decryption operations,
/// allowing AppState to hold a trait object instead of `Arc<dyn Any>`.
#[async_trait::async_trait]
pub trait ConfigEncryptionProvider: Send + Sync {
    /// Check if encryption is enabled
    fn is_enabled(&self) -> bool;

    /// Check if a data_id should be encrypted based on configured patterns
    fn should_encrypt(&self, data_id: &str) -> bool;

    /// Encrypt content if the data_id matches encryption patterns
    ///
    /// Returns (possibly encrypted content, encrypted_data_key).
    /// If encryption is disabled or data_id doesn't match, returns original content
    /// with an empty data key.
    async fn encrypt_if_needed(&self, data_id: &str, content: &str) -> (String, String);

    /// Decrypt content if it has an encrypted data key
    ///
    /// If the encrypted_data_key is empty, returns the content as-is.
    async fn decrypt_if_needed(
        &self,
        data_id: &str,
        content: &str,
        encrypted_data_key: &str,
    ) -> String;

    /// Encrypt content directly
    async fn encrypt(&self, content: &str) -> CryptoResult<(String, String)>;

    /// Decrypt content directly
    async fn decrypt(&self, content: &str, encrypted_data_key: &str) -> CryptoResult<String>;
}

// ============================================================================
// OAuth Provider Trait
// ============================================================================

/// OAuth/OIDC provider trait for pluggable external authentication.
///
/// Abstracts the OAuth2/OIDC service so that AppState can hold a trait object
/// instead of a concrete `OAuthService` type. This decouples `batata-server-common`
/// from `batata-auth`'s OAuth implementation.
#[async_trait::async_trait]
pub trait OAuthProvider: Send + Sync {
    /// Check if OAuth is enabled
    fn is_enabled(&self) -> bool;

    /// Get all enabled provider names
    fn get_enabled_providers(&self) -> Vec<String>;

    /// Generate authorization URL for a provider
    async fn get_authorization_url(
        &self,
        provider_name: &str,
        redirect_uri: &str,
    ) -> anyhow::Result<(String, String)>;

    /// Exchange authorization code for tokens
    async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        redirect_uri: &str,
        state: &str,
    ) -> anyhow::Result<OAuthTokenResponse>;

    /// Get user info from provider using access token
    async fn get_user_info(
        &self,
        provider_name: &str,
        access_token: &str,
    ) -> anyhow::Result<OAuthUserProfile>;
}

/// OAuth token response (provider-agnostic)
#[derive(Debug, Clone)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub scope: Option<String>,
}

/// OAuth user profile (provider-agnostic)
#[derive(Debug, Clone)]
pub struct OAuthUserProfile {
    pub provider_user_id: String,
    pub username: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub groups: Vec<String>,
}

// ============================================================================
// gRPC Connection Manager Trait
// ============================================================================

/// Trait for managing gRPC client connections.
///
/// Abstracts connection registration, lookup, and lifecycle management,
/// allowing naming handlers and other components to depend on the trait
/// rather than the concrete `ConnectionManager` type.
#[async_trait::async_trait]
pub trait GrpcConnectionManager: Send + Sync {
    /// Register a new gRPC client connection.
    /// Returns true if successfully registered, false if rejected.
    async fn register_connection(&self, connection_id: &str, client: GrpcClientInfo) -> bool;

    /// Unregister a gRPC client connection
    async fn unregister_connection(&self, connection_id: &str);

    /// Get a connection's client IP by connection ID
    fn get_client_ip(&self, connection_id: &str) -> Option<String>;

    /// Get all connection IDs
    fn get_all_connection_ids(&self) -> Vec<String>;

    /// Get the number of active connections
    fn connection_count(&self) -> usize;
}

/// Minimal gRPC client info for the trait boundary
#[derive(Debug, Clone)]
pub struct GrpcClientInfo {
    pub client_ip: String,
    pub client_port: u16,
    pub app_name: String,
    pub sdk: String,
    pub labels: std::collections::HashMap<String, String>,
}

// ============================================================================
// Plugin Base Trait
// ============================================================================

/// Plugin trait for extensibility
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// Get the plugin name
    fn name(&self) -> &str;

    /// Initialize the plugin
    async fn init(&self) -> anyhow::Result<()>;

    /// Shutdown the plugin
    async fn shutdown(&self) -> anyhow::Result<()>;
}

// ============================================================================
// Control Plugin Trait
// ============================================================================

use crate::model::plugin::control::{
    ConnectionLimitResult, ControlContext, ControlStats, RateLimitResult,
};

/// Control Plugin SPI for rate limiting and connection control
#[async_trait::async_trait]
pub trait ControlPlugin: Plugin {
    /// Check rate limit for a request
    async fn check_rate_limit(&self, ctx: &ControlContext) -> RateLimitResult;

    /// Check connection limit
    async fn check_connection_limit(&self, ctx: &ControlContext) -> ConnectionLimitResult;

    /// Release a connection (call when connection closes)
    async fn release_connection(&self, ctx: &ControlContext);

    /// Get control statistics
    async fn get_stats(&self) -> ControlStats;

    /// Reload rules from storage
    async fn reload_rules(&self) -> anyhow::Result<()>;
}

// ============================================================================
// CMDB Plugin Trait
// ============================================================================

use crate::model::plugin::cmdb::{
    CmdbEntity, CmdbEntityType, CmdbStats, CmdbSyncResult, LabelMapping,
};

/// CMDB Plugin SPI for label sync and entity mapping
#[async_trait::async_trait]
pub trait CmdbPlugin: Plugin {
    async fn register_entity(&self, entity: CmdbEntity) -> anyhow::Result<String>;
    async fn update_entity(&self, entity: CmdbEntity) -> anyhow::Result<()>;
    async fn delete_entity(&self, id: &str) -> anyhow::Result<bool>;
    async fn get_entity(&self, id: &str) -> anyhow::Result<Option<CmdbEntity>>;
    async fn list_entities(
        &self,
        entity_type: Option<CmdbEntityType>,
    ) -> anyhow::Result<Vec<CmdbEntity>>;
    async fn search_by_labels(
        &self,
        labels: &std::collections::HashMap<String, String>,
    ) -> anyhow::Result<Vec<CmdbEntity>>;
    async fn sync_labels(
        &self,
        entity_id: &str,
        labels: &std::collections::HashMap<String, String>,
    ) -> anyhow::Result<std::collections::HashMap<String, String>>;
    async fn map_entity(&self, entity: &CmdbEntity) -> anyhow::Result<serde_json::Value>;
    async fn full_sync(&self) -> anyhow::Result<CmdbSyncResult>;
    async fn get_label_mappings(&self) -> anyhow::Result<Vec<LabelMapping>>;
    async fn add_label_mapping(&self, mapping: LabelMapping) -> anyhow::Result<String>;
    async fn remove_label_mapping(&self, id: &str) -> anyhow::Result<bool>;
    async fn get_stats(&self) -> CmdbStats;
}

// ============================================================================
// AI Service Traits
// ============================================================================

use crate::model::Page;
use crate::model::ai::VersionDetail;
use crate::model::ai::a2a::{
    AgentCard, AgentCardVersionInfo, AgentRegistryStats, BatchAgentRegistrationRequest,
    BatchRegistrationResponse, RegisteredAgent,
};
use crate::model::ai::agentspec::{AgentSpec, AgentSpecBasicInfo, AgentSpecMeta, AgentSpecSummary};
use crate::model::ai::mcp::McpRegistryStats;
use crate::model::ai::mcp::{McpServer, McpServerBasicInfo, McpServerRegistration};
use crate::model::ai::pipeline::PipelineExecution;
use crate::model::ai::skill::{Skill, SkillBasicInfo, SkillMeta, SkillSummary};

/// Trait for skill lifecycle operations (CRUD, draft, publish, etc.)
#[async_trait::async_trait]
pub trait SkillService: Send + Sync {
    async fn get_skill_detail(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Option<SkillMeta>>;

    async fn get_skill_version_detail(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<Skill>>;

    async fn download_skill_version(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<Skill>>;

    async fn delete_skill(&self, namespace_id: &str, name: &str) -> anyhow::Result<()>;

    async fn list_skills(
        &self,
        namespace_id: &str,
        skill_name: Option<&str>,
        search: Option<&str>,
        order_by: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<SkillSummary>>;

    async fn upload_skill(
        &self,
        namespace_id: &str,
        name: &str,
        skill: &Skill,
        author: &str,
        overwrite: bool,
    ) -> anyhow::Result<String>;

    async fn create_draft(
        &self,
        namespace_id: &str,
        name: &str,
        based_on_version: Option<&str>,
        target_version: Option<&str>,
        initial_content: Option<&Skill>,
        author: &str,
    ) -> anyhow::Result<String>;

    async fn update_draft(
        &self,
        namespace_id: &str,
        name: &str,
        skill: &Skill,
    ) -> anyhow::Result<()>;

    async fn delete_draft(&self, namespace_id: &str, name: &str) -> anyhow::Result<()>;

    async fn submit(&self, namespace_id: &str, name: &str, version: &str)
    -> anyhow::Result<String>;

    async fn publish(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        update_latest_label: bool,
    ) -> anyhow::Result<()>;

    async fn update_labels(
        &self,
        namespace_id: &str,
        name: &str,
        labels: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<()>;

    async fn update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        biz_tags: &str,
    ) -> anyhow::Result<()>;

    async fn change_online_status(
        &self,
        namespace_id: &str,
        name: &str,
        scope: Option<&str>,
        version: Option<&str>,
        online: bool,
    ) -> anyhow::Result<()>;

    async fn update_scope(&self, namespace_id: &str, name: &str, scope: &str)
    -> anyhow::Result<()>;

    async fn query_skill(
        &self,
        namespace_id: &str,
        name: &str,
        version: Option<&str>,
        label: Option<&str>,
    ) -> anyhow::Result<Option<Skill>>;

    async fn search_skills(
        &self,
        namespace_id: &str,
        keyword: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<SkillBasicInfo>>;
}

/// Trait for agentspec lifecycle operations (CRUD, draft, publish, etc.)
#[async_trait::async_trait]
pub trait AgentSpecService: Send + Sync {
    async fn get_detail(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Option<AgentSpecMeta>>;

    async fn get_version_detail(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<AgentSpec>>;

    async fn delete(&self, namespace_id: &str, name: &str) -> anyhow::Result<()>;

    async fn list(
        &self,
        namespace_id: &str,
        name_filter: Option<&str>,
        search: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentSpecSummary>>;

    async fn upload(
        &self,
        namespace_id: &str,
        name: &str,
        spec: &AgentSpec,
        author: &str,
        overwrite: bool,
    ) -> anyhow::Result<String>;

    async fn create_draft(
        &self,
        namespace_id: &str,
        name: &str,
        based_on_version: Option<&str>,
        target_version: Option<&str>,
        initial_content: Option<&AgentSpec>,
        author: &str,
    ) -> anyhow::Result<String>;

    async fn update_draft(
        &self,
        namespace_id: &str,
        name: &str,
        spec: &AgentSpec,
    ) -> anyhow::Result<()>;

    async fn delete_draft(&self, namespace_id: &str, name: &str) -> anyhow::Result<()>;

    async fn submit(&self, namespace_id: &str, name: &str, version: &str)
    -> anyhow::Result<String>;

    async fn publish(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        update_latest_label: bool,
    ) -> anyhow::Result<()>;

    async fn update_labels(
        &self,
        namespace_id: &str,
        name: &str,
        labels: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<()>;

    async fn update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        biz_tags: &str,
    ) -> anyhow::Result<()>;

    async fn change_online_status(
        &self,
        namespace_id: &str,
        name: &str,
        scope: Option<&str>,
        version: Option<&str>,
        online: bool,
    ) -> anyhow::Result<()>;

    async fn update_scope(&self, namespace_id: &str, name: &str, scope: &str)
    -> anyhow::Result<()>;

    async fn query(
        &self,
        namespace_id: &str,
        name: &str,
        version: Option<&str>,
        label: Option<&str>,
    ) -> anyhow::Result<Option<AgentSpec>>;

    async fn search(
        &self,
        namespace_id: &str,
        keyword: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentSpecBasicInfo>>;
}

/// Trait for MCP server CRUD operations (config-backed persistence)
#[async_trait::async_trait]
pub trait McpServerService: Send + Sync {
    async fn create_mcp_server(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<String>;

    async fn get_mcp_server_detail(
        &self,
        namespace: &str,
        id: Option<&str>,
        name: Option<&str>,
        version: Option<&str>,
    ) -> anyhow::Result<Option<McpServer>>;

    async fn update_mcp_server(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<()>;

    async fn delete_mcp_server(
        &self,
        namespace: &str,
        name: Option<&str>,
        id: Option<&str>,
        version: Option<&str>,
    ) -> anyhow::Result<()>;

    fn list_mcp_servers(
        &self,
        namespace: &str,
        name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
    ) -> Page<McpServerBasicInfo>;

    /// Import tools from a running MCP server via SSE transport
    async fn import_tools_from_mcp(
        &self,
        base_url: &str,
        endpoint: &str,
        auth_token: Option<&str>,
        timeout: std::time::Duration,
    ) -> anyhow::Result<Vec<crate::model::ai::mcp::McpTool>>;

    /// Import MCP servers from a config (e.g., claude_desktop_config.json)
    async fn import_mcp_servers(
        &self,
        request: crate::model::ai::mcp::McpServerImportRequest,
    ) -> anyhow::Result<crate::model::ai::a2a::BatchRegistrationResponse>;

    /// Get registry statistics
    async fn mcp_stats(&self) -> anyhow::Result<McpRegistryStats>;
}

/// Trait for A2A agent CRUD operations (config-backed persistence)
#[async_trait::async_trait]
pub trait A2aAgentService: Send + Sync {
    async fn register_agent(
        &self,
        card: &AgentCard,
        namespace: &str,
        registration_type: &str,
    ) -> anyhow::Result<String>;

    async fn get_agent_card(
        &self,
        namespace: &str,
        agent_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<Option<RegisteredAgent>>;

    async fn update_agent_card(
        &self,
        card: &AgentCard,
        namespace: &str,
        registration_type: &str,
    ) -> anyhow::Result<()>;

    async fn delete_agent(
        &self,
        namespace: &str,
        agent_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()>;

    async fn list_agents(
        &self,
        namespace: &str,
        agent_name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<Page<AgentCardVersionInfo>>;

    async fn list_versions(
        &self,
        namespace: &str,
        agent_name: &str,
    ) -> anyhow::Result<Vec<VersionDetail>>;

    /// Find agents that provide a specific skill
    async fn find_by_skill(&self, skill: &str) -> anyhow::Result<Vec<RegisteredAgent>>;

    /// Batch register multiple agents
    async fn batch_register(
        &self,
        request: BatchAgentRegistrationRequest,
    ) -> anyhow::Result<BatchRegistrationResponse>;

    /// Get registry statistics
    async fn stats(&self) -> anyhow::Result<AgentRegistryStats>;
}

/// Trait for pipeline query operations
#[async_trait::async_trait]
pub trait PipelineService: Send + Sync {
    async fn get_pipeline(&self, execution_id: &str) -> anyhow::Result<Option<PipelineExecution>>;

    async fn list_pipelines(
        &self,
        resource_type: &str,
        resource_name: Option<&str>,
        namespace_id: Option<&str>,
        version: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PipelineExecution>>;
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
        assert_eq!(ns.config_count, 0);
        assert_eq!(ns.quota, 0);
    }

    #[test]
    fn test_member_info_creation() {
        let member = MemberInfo {
            ip: "192.168.1.1".to_string(),
            port: 8848,
            address: "192.168.1.1:8848".to_string(),
            state: MemberState::Up,
        };
        assert_eq!(member.ip, "192.168.1.1");
        assert_eq!(member.port, 8848);
    }

    #[test]
    fn test_cluster_health_summary_is_healthy() {
        let healthy = ClusterHealthSummary {
            total: 3,
            up: 2,
            down: 1,
            ..Default::default()
        };
        assert!(healthy.is_healthy());

        let unhealthy = ClusterHealthSummary {
            total: 3,
            up: 1,
            down: 2,
            ..Default::default()
        };
        assert!(!unhealthy.is_healthy());
    }

    #[test]
    fn test_extended_member_info() {
        let member = ExtendedMemberInfo {
            ip: "10.0.0.1".to_string(),
            port: 8848,
            address: "10.0.0.1:8848".to_string(),
            state: MemberState::Up,
            extend_info: std::collections::BTreeMap::new(),
        };
        assert_eq!(member.ip, "10.0.0.1");
        assert_eq!(member.state, MemberState::Up);
    }

    #[test]
    fn test_config_subscription_key() {
        let key = ConfigSubscriptionKey::new("app.yaml", "DEFAULT_GROUP", "public");
        assert_eq!(key.data_id, "app.yaml");
        assert_eq!(key.group, "DEFAULT_GROUP");
        assert_eq!(key.tenant, "public");
    }

    #[test]
    fn test_connection_info_default() {
        let info = ConnectionInfo::default();
        assert!(info.connection_id.is_empty());
        assert!(info.client_ip.is_empty());
        assert_eq!(info.create_time, 0);
    }

    #[test]
    fn test_auth_check_result() {
        let ok = AuthCheckResult::success();
        assert!(ok.success);
        assert!(ok.message.is_none());

        let fail = AuthCheckResult::fail("token expired");
        assert!(!fail.success);
        assert_eq!(fail.message.as_deref(), Some("token expired"));
    }

    #[test]
    fn test_identity_context_default() {
        let ctx = IdentityContext::default();
        assert!(ctx.token.is_none());
        assert!(ctx.username.is_empty());
        assert!(!ctx.authenticated);
        assert!(!ctx.is_global_admin);
    }

    #[test]
    fn test_request_token_default() {
        let token = RequestToken::default();
        assert!(token.0.is_none());

        let token = RequestToken(Some("abc123".to_string()));
        assert_eq!(token.0.as_deref(), Some("abc123"));
    }

    #[test]
    fn test_auth_permission() {
        let perm = AuthPermission {
            resource: "public:DEFAULT_GROUP:config/app.yaml".to_string(),
            action: "r".to_string(),
        };
        assert!(perm.resource.contains("config"));
        assert_eq!(perm.action, "r");
    }

    #[test]
    fn test_login_result() {
        let result = LoginResult {
            token: "jwt-token".to_string(),
            token_ttl: 18000,
            username: "nacos".to_string(),
            is_global_admin: true,
        };
        assert_eq!(result.username, "nacos");
        assert!(result.is_global_admin);
        assert_eq!(result.token_ttl, 18000);
    }
}
