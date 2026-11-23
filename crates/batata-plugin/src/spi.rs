//! Plugin SPI (Service Provider Interface) definitions
//!
//! Standard plugin interfaces for extending Batata functionality.
//! Each plugin type has a trait that implementations must satisfy.

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Plugin lifecycle and metadata
pub trait Plugin: Send + Sync {
    /// Plugin name
    fn name(&self) -> &str;

    /// Plugin type identifier
    fn plugin_type(&self) -> &str;

    /// Plugin priority (lower = higher priority)
    fn priority(&self) -> i32 {
        0
    }

    /// Whether this plugin is enabled
    fn is_enabled(&self) -> bool {
        true
    }
}

/// Authentication plugin for custom auth providers (SPI interface).
///
/// NOTE: This is the plugin SPI interface for potential external auth providers.
/// The **active** auth trait used by the application is `batata_common::AuthPlugin`,
/// which has a different API shape suited for the middleware pipeline.
#[async_trait::async_trait]
pub trait AuthPlugin: Plugin {
    /// Authenticate a user with credentials
    async fn authenticate(
        &self,
        username: &str,
        password: &str,
        properties: &HashMap<String, String>,
    ) -> AuthPluginResult;

    /// Authorize an action on a resource
    async fn authorize(&self, username: &str, resource: &str, action: &str) -> bool;
}

/// Result of authentication
#[derive(Debug, Clone)]
pub struct AuthPluginResult {
    /// Whether authentication succeeded.
    pub success: bool,
    /// Authenticated username.
    pub username: String,
    /// Optional human-readable message (e.g. failure reason).
    pub message: Option<String>,
    /// Properties/attributes attached to the authenticated principal.
    pub properties: HashMap<String, String>,
}

/// Config change plugin for intercepting config operations
#[async_trait::async_trait]
pub trait ConfigChangePlugin: Plugin {
    /// Called before a config is published/updated
    /// Return false to abort the operation
    async fn before_publish(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
    ) -> ConfigChangeResult;

    /// Called after a config is published/updated
    async fn after_publish(&self, data_id: &str, group: &str, tenant: &str, content: &str);

    /// Called before a config is deleted
    async fn before_delete(&self, data_id: &str, group: &str, tenant: &str) -> ConfigChangeResult;

    /// Called after a config is deleted
    async fn after_delete(&self, data_id: &str, group: &str, tenant: &str);
}

/// Result of config change interception
#[derive(Debug, Clone)]
pub struct ConfigChangeResult {
    /// Whether the operation is allowed to proceed.
    pub allowed: bool,
    /// Optional message describing the decision (e.g. denial reason).
    pub message: Option<String>,
}

impl ConfigChangeResult {
    /// Create an allow result.
    pub fn allow() -> Self {
        Self {
            allowed: true,
            message: None,
        }
    }

    /// Create a deny result with the given message.
    pub fn deny(message: &str) -> Self {
        Self {
            allowed: false,
            message: Some(message.to_string()),
        }
    }
}

// =============================================================================
// Enhanced Config Change Plugin V2 — Pointcut, ExecuteType, parameter replacement
// =============================================================================

/// Interception point for config operations.
/// Mirrors Nacos `ConfigChangePointCutTypes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigPointcut {
    /// Configuration publish (create/update) operation.
    Publish,
    /// Configuration remove (delete) operation.
    Remove,
    /// Configuration get (read) operation.
    Get,
    /// Batch configuration import operation.
    Import,
    /// Configuration export operation.
    Export,
    /// Configuration history query operation.
    History,
}

impl ConfigPointcut {
    /// Return the string form of this pointcut.
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfigPointcut::Publish => "publish",
            ConfigPointcut::Remove => "remove",
            ConfigPointcut::Get => "get",
            ConfigPointcut::Import => "import",
            ConfigPointcut::Export => "export",
            ConfigPointcut::History => "history",
        }
    }
}

/// Execution timing for plugin hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteType {
    /// Hook runs before the operation.
    Before,
    /// Hook runs after the operation.
    After,
}

impl ExecuteType {
    /// Return the string form of this execute type.
    pub fn as_str(&self) -> &'static str {
        match self {
            ExecuteType::Before => "before",
            ExecuteType::After => "after",
        }
    }
}

/// Context for a config change operation, passed through the plugin chain.
/// Plugins can read and modify fields (parameter replacement).
#[derive(Debug, Clone)]
pub struct ConfigChangeRequest {
    /// Config data ID the operation targets.
    pub data_id: String,
    /// Config group the operation targets.
    pub group: String,
    /// Tenant (namespace) the operation targets.
    pub tenant: String,
    /// Config content.
    pub content: String,
    /// Optional content type of the config.
    pub content_type: Option<String>,
    /// Arbitrary key-value metadata attached to the operation.
    pub metadata: HashMap<String, String>,
    /// Interception pointcut of the operation.
    pub pointcut: ConfigPointcut,
    /// Execution timing (before/after) of the operation.
    pub execute_type: ExecuteType,
    /// Operator that initiated the operation.
    pub operator: String,
    /// Client IP address that initiated the operation, if known.
    pub client_ip: Option<String>,
    /// Whether the chain should continue; set to `false` to deny.
    pub proceed: bool,
    /// Reason for denial, populated when `proceed` is `false`.
    pub deny_reason: Option<String>,
}

impl ConfigChangeRequest {
    /// Create a new config change request with default metadata and `proceed = true`.
    pub fn new(
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        pointcut: ConfigPointcut,
        execute_type: ExecuteType,
        operator: &str,
    ) -> Self {
        Self {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
            content: content.to_string(),
            content_type: None,
            metadata: HashMap::new(),
            pointcut,
            execute_type,
            operator: operator.to_string(),
            client_ip: None,
            proceed: true,
            deny_reason: None,
        }
    }

    /// Mark the request as denied with the given reason.
    pub fn deny(&mut self, reason: &str) {
        self.proceed = false;
        self.deny_reason = Some(reason.to_string());
    }
}

/// Result after running the plugin chain.
#[derive(Debug, Clone)]
pub struct ConfigChangeResponse {
    /// Whether the operation is allowed to proceed.
    pub allowed: bool,
    /// Optional message describing the decision.
    pub message: Option<String>,
    /// The (possibly mutated) request when allowed, `None` otherwise.
    pub request: Option<ConfigChangeRequest>,
}

impl ConfigChangeResponse {
    /// Create an allow result with no attached request.
    pub fn allow() -> Self {
        Self {
            allowed: true,
            message: None,
            request: None,
        }
    }

    /// Create an allow result carrying the (possibly mutated) request.
    pub fn allow_with_request(req: ConfigChangeRequest) -> Self {
        Self {
            allowed: true,
            message: None,
            request: Some(req),
        }
    }

    /// Create a deny result with no attached request.
    pub fn deny(message: &str) -> Self {
        Self {
            allowed: false,
            message: Some(message.to_string()),
            request: None,
        }
    }
}

/// Enhanced config change plugin supporting Pointcut, ExecuteType, and parameter replacement.
#[async_trait::async_trait]
pub trait ConfigChangePluginV2: Send + Sync {
    /// Plugin name.
    fn name(&self) -> &str;
    /// Plugin order; lower values run earlier in the chain.
    fn order(&self) -> i32 {
        0
    }
    /// Whether this plugin is enabled.
    fn is_enabled(&self) -> bool {
        true
    }
    /// Pointcuts this plugin wants to intercept.
    fn interested_pointcuts(&self) -> Vec<ConfigPointcut> {
        vec![
            ConfigPointcut::Publish,
            ConfigPointcut::Remove,
            ConfigPointcut::Get,
            ConfigPointcut::Import,
            ConfigPointcut::Export,
            ConfigPointcut::History,
        ]
    }
    /// Execute types (before/after) this plugin wants to handle.
    fn interested_execute_types(&self) -> Vec<ExecuteType> {
        vec![ExecuteType::Before, ExecuteType::After]
    }
    /// Run the plugin against the mutable request, returning an allow/deny result.
    async fn execute(&self, ctx: &mut ConfigChangeRequest) -> ConfigChangeResult;
}

/// Wraps the old ConfigChangePlugin to work with the new V2 chain.
pub struct ConfigChangePluginAdapter {
    /// The wrapped legacy `ConfigChangePlugin`.
    pub inner: Arc<dyn ConfigChangePlugin>,
}

#[async_trait::async_trait]
impl ConfigChangePluginV2 for ConfigChangePluginAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn order(&self) -> i32 {
        self.inner.priority()
    }
    fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }
    fn interested_pointcuts(&self) -> Vec<ConfigPointcut> {
        vec![ConfigPointcut::Publish, ConfigPointcut::Remove]
    }
    async fn execute(&self, ctx: &mut ConfigChangeRequest) -> ConfigChangeResult {
        match (ctx.pointcut, ctx.execute_type) {
            (ConfigPointcut::Publish, ExecuteType::Before) => {
                self.inner
                    .before_publish(&ctx.data_id, &ctx.group, &ctx.tenant, &ctx.content)
                    .await
            }
            (ConfigPointcut::Publish, ExecuteType::After) => {
                self.inner
                    .after_publish(&ctx.data_id, &ctx.group, &ctx.tenant, &ctx.content)
                    .await;
                ConfigChangeResult::allow()
            }
            (ConfigPointcut::Remove, ExecuteType::Before) => {
                self.inner
                    .before_delete(&ctx.data_id, &ctx.group, &ctx.tenant)
                    .await
            }
            (ConfigPointcut::Remove, ExecuteType::After) => {
                self.inner
                    .after_delete(&ctx.data_id, &ctx.group, &ctx.tenant)
                    .await;
                ConfigChangeResult::allow()
            }
            _ => ConfigChangeResult::allow(),
        }
    }
}

/// Traffic control plugin for rate limiting
#[async_trait::async_trait]
pub trait ControlPlugin: Plugin {
    /// Check if a request should be allowed based on TPS rules
    async fn check_tps(&self, point_name: &str, client_ip: &str) -> TpsCheckResult;
}

/// Result of TPS check
#[derive(Debug, Clone)]
pub struct TpsCheckResult {
    /// Whether the request is allowed under the TPS limit.
    pub allowed: bool,
    /// Remaining requests permitted in the current window.
    pub remaining: u64,
    /// Configured TPS limit.
    pub limit: u64,
}

/// Canonical names for the infrastructure dependencies passed to protocol
/// adapters. Dynamic extensions may still use arbitrary keys, but built-in
/// producers and consumers must use these constants.
pub mod context_keys {
    /// `Arc<bool>`; whether the server is in cluster mode.
    pub const IS_CLUSTER: &str = "is_cluster";
    /// `Arc<batata_persistence::model::StorageMode>`.
    pub const STORAGE_MODE: &str = "storage_mode";
    /// `Arc<sea_orm::DatabaseConnection>`; present for ExternalDb mode.
    pub const DATABASE: &str = "db";
    /// `Arc<rocksdb::DB>`; present for embedded mode.
    pub const ROCKS_DB: &str = "rocks_db";
    /// `Arc<batata_consistency::RaftNode>`; present in cluster mode.
    pub const RAFT_NODE: &str = "raft_node";
    /// Legacy `Arc<Arc<dyn ClusterManager>>` kept for older readers.
    pub const CLUSTER_MANAGER: &str = "cluster_manager";
    /// Normalized `Arc<dyn ClusterManager>` for typed readers.
    pub const CLUSTER_MANAGER_TYPED: &str = "cluster_manager_typed";
    /// `Arc<batata_core::service::distro::DistroProtocol>`; cluster mode only.
    pub const DISTRO_PROTOCOL: &str = "distro_protocol";
}

/// A missing or wrong-typed required plugin context dependency.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PluginContextError {
    /// The key was not inserted at all.
    #[error("required plugin context key '{key}' is missing")]
    Missing {
        /// Canonical context key.
        key: String,
    },
    /// The key exists but holds a different concrete type.
    #[error("plugin context key '{key}' has type '{actual}', expected '{expected}'")]
    WrongType {
        /// Canonical context key.
        key: String,
        /// Type requested by the caller.
        expected: &'static str,
        /// Type actually inserted by the producer.
        actual: &'static str,
    },
}

struct ContextValue {
    value: Arc<dyn Any + Send + Sync>,
    type_name: &'static str,
}

/// A sized wrapper for injecting a possibly unsized shared dependency
/// (typically `Arc<dyn Trait>`) through `PluginContext`.
///
/// `Any` downcasts require a sized concrete type, so a bare trait object
/// cannot be stored directly. This wrapper keeps the SPI generic while letting
/// producers expose a single `Arc<dyn Trait>` instead of a double `Arc`.
#[derive(Clone)]
pub struct SharedDependency<T: ?Sized + Send + Sync + 'static>(pub Arc<T>);

impl<T: ?Sized + Send + Sync + 'static> SharedDependency<T> {
    /// Wrap a shared dependency for insertion into a [`PluginContext`].
    pub fn new(value: Arc<T>) -> Self {
        Self(value)
    }
}

/// Context provided to protocol adapter plugins during initialization.
///
/// Carries server resources as type-erased `Any` trait objects to avoid
/// coupling the plugin SPI to specific infrastructure crates. Known keys are
/// named in [`context_keys`]; [`PluginContext::get_required`] distinguishes a
/// missing key from a wrong concrete type instead of collapsing both into
/// `None`.
pub struct PluginContext {
    extensions: HashMap<String, ContextValue>,
}

impl PluginContext {
    /// Create a new, empty plugin context.
    pub fn new() -> Self {
        Self {
            extensions: HashMap::new(),
        }
    }

    /// Insert a typed value into the context.
    pub fn insert<T: Send + Sync + 'static>(&mut self, key: &str, value: Arc<T>) {
        self.extensions.insert(
            key.to_string(),
            ContextValue {
                value,
                type_name: std::any::type_name::<T>(),
            },
        );
    }

    /// Get a typed reference from the context. Returns None if key missing or wrong type.
    pub fn get<T: Send + Sync + 'static>(&self, key: &str) -> Option<Arc<T>> {
        self.extensions
            .get(key)
            .and_then(|entry| entry.value.clone().downcast::<T>().ok())
    }

    /// Get a required typed dependency, distinguishing missing from wrong-type.
    pub fn get_required<T: Send + Sync + 'static>(
        &self,
        key: &str,
    ) -> Result<Arc<T>, PluginContextError> {
        match self.extensions.get(key) {
            None => Err(PluginContextError::Missing {
                key: key.to_string(),
            }),
            Some(entry) => {
                entry
                    .value
                    .clone()
                    .downcast::<T>()
                    .map_err(|_| PluginContextError::WrongType {
                        key: key.to_string(),
                        expected: std::any::type_name::<T>(),
                        actual: entry.type_name,
                    })
            }
        }
    }

    /// Insert a shared possibly-unsized dependency (for example
    /// `Arc<dyn ClusterManager>`) without forcing producers into a double Arc.
    pub fn insert_shared<T: ?Sized + Send + Sync + 'static>(&mut self, key: &str, value: Arc<T>) {
        self.insert(key, Arc::new(SharedDependency::new(value)));
    }

    /// Get a shared dependency inserted with [`PluginContext::insert_shared`].
    pub fn get_shared<T: ?Sized + Send + Sync + 'static>(&self, key: &str) -> Option<Arc<T>> {
        self.get::<SharedDependency<T>>(key)
            .map(|wrapper| wrapper.0.clone())
    }

    /// Required variant of [`PluginContext::get_shared`].
    pub fn get_shared_required<T: ?Sized + Send + Sync + 'static>(
        &self,
        key: &str,
    ) -> Result<Arc<T>, PluginContextError> {
        self.get_required::<SharedDependency<T>>(key)
            .map(|wrapper| wrapper.0.clone())
    }

    /// Check if a key exists.
    pub fn contains(&self, key: &str) -> bool {
        self.extensions.contains_key(key)
    }

    /// Describe all inserted keys and their concrete type names for startup
    /// validation logs. The result is key-sorted so log output is stable.
    pub fn describe(&self) -> BTreeMap<String, &'static str> {
        self.extensions
            .iter()
            .map(|(key, entry)| (key.clone(), entry.type_name))
            .collect()
    }
}

impl Default for PluginContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Protocol adapter plugin for compatibility layers (e.g., Consul, Eureka, Apollo).
///
/// Protocol adapters translate external protocols into Batata-native operations.
/// Batata's core is a Nacos-compatible service discovery and configuration platform.
/// Protocol adapter plugins enable clients of other systems to interact with Batata
/// using their native protocols.
///
/// # Lifecycle
///
/// 1. Plugin is constructed by a factory function with its dependencies
/// 2. `init()` is called during server startup
/// 3. `configure()` is called to register HTTP routes and services
/// 4. `start_background_tasks()` is called to spawn monitors and sync tasks
/// 5. Plugin serves requests until shutdown
/// 6. `shutdown()` is called during graceful server shutdown
///
/// # Example
///
/// ```rust,ignore
/// impl ProtocolAdapterPlugin for ConsulPlugin {
///     fn name(&self) -> &str { "consul-compatibility" }
///     fn protocol(&self) -> &str { "consul" }
///     fn is_enabled(&self) -> bool { true }
///     fn default_port(&self) -> u16 { 8500 }
///
///     fn configure(&self, cfg: &mut actix_web::web::ServiceConfig) {
///         cfg.app_data(web::Data::new(self.agent.clone()))
///            .service(routes());
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait ProtocolAdapterPlugin: Send + Sync {
    // ========================================================================
    // Metadata
    // ========================================================================

    /// Plugin name (e.g., "consul-compatibility")
    fn name(&self) -> &str;

    /// Protocol identifier (e.g., "consul", "eureka", "apollo")
    fn protocol(&self) -> &str;

    /// Whether this adapter is enabled
    fn is_enabled(&self) -> bool;

    /// Plugin priority (lower = higher priority, for ordering)
    fn priority(&self) -> i32 {
        0
    }

    /// Default HTTP port for this protocol's server (e.g., 8500 for Consul)
    fn default_port(&self) -> u16;

    /// Number of HTTP workers for this protocol's server.
    /// Default: min(4, cpu_cores/2), at least 2.
    fn http_workers(&self) -> usize {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        std::cmp::min(4, cpus / 2).max(2)
    }

    // ========================================================================
    // Lifecycle
    // ========================================================================

    /// Initialize the plugin after construction.
    ///
    /// Called once during server startup with server context (RaftNode, Configuration, etc.).
    /// Use this for Raft registration, store creation, and service initialization.
    async fn init(&self, _ctx: &PluginContext) -> anyhow::Result<()> {
        Ok(())
    }

    /// Shut down the plugin gracefully.
    ///
    /// Called during server shutdown. Use this to flush buffers,
    /// close connections, and release resources.
    async fn shutdown(&self) -> anyhow::Result<()> {
        Ok(())
    }

    // ========================================================================
    // HTTP Route Registration
    // ========================================================================

    /// Register HTTP routes and services with the actix-web app.
    ///
    /// This is the primary integration point. The plugin should:
    /// 1. Register its service instances as `web::Data` (for handler injection)
    /// 2. Mount its HTTP routes (e.g., `/v1/agent/*` for Consul)
    ///
    /// Called once during HTTP server construction.
    fn configure(&self, cfg: &mut actix_web::web::ServiceConfig);

    // ========================================================================
    // Background Tasks
    // ========================================================================

    /// Column families required by this plugin for RocksDB storage.
    ///
    /// Called BEFORE `init()` — during server startup, the Raft state machine
    /// creates these CFs when opening RocksDB. Return an empty vec if the
    /// plugin doesn't use RocksDB.
    fn required_column_families(&self) -> Vec<String> {
        Vec::new()
    }

    /// Start background tasks (monitors, sync jobs, cleanup tasks).
    ///
    /// Called after the HTTP server is running. Plugins should spawn their
    /// own tokio tasks for periodic work like:
    /// - TTL health check monitoring
    /// - Instance deregistration cleanup
    /// - Session expiration
    ///
    /// The default implementation does nothing (no background tasks).
    async fn start_background_tasks(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Health check result handler — called when a check status changes.
///
/// The health check engine (InstanceCheckRegistry) is protocol-agnostic.
/// It executes probes and tracks status, but doesn't know how to apply
/// the result to a specific service registry. This trait bridges that gap.
///
/// - **CoreResultHandler** (built-in): updates `Instance.healthy` in NamingService
/// - **ConsulResultHandler** (plugin): updates check status in ConsulNamingStore,
///   increments blocking query index, triggers auto-deregister
pub trait HealthCheckResultHandler: Send + Sync {
    /// Called when the aggregated health of an instance changes.
    ///
    /// `healthy`: true if all checks are passing/warning, false if any is critical.
    fn on_health_changed(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        ip: &str,
        port: i32,
        cluster: &str,
        healthy: bool,
    );

    /// Called when an individual check transitions to Critical.
    ///
    /// This is called per-check, before the aggregated `on_health_changed`.
    /// Used by Consul to invalidate sessions linked to the failed check.
    ///
    /// Default: no-op. Override to react to individual check failures.
    fn on_check_critical(&self, check_id: &str) {
        let _ = check_id;
    }

    /// Called when an instance should be deregistered (critical too long).
    ///
    /// `check_id` is the ID of the check that triggered the deregistration.
    /// Consul uses this to look up the ServiceID and deregister by service key.
    ///
    /// Default: no-op. Override for auto-deregister behavior.
    fn on_deregister(
        &self,
        check_id: &str,
        namespace: &str,
        group: &str,
        service: &str,
        ip: &str,
        port: i32,
        cluster: &str,
    ) {
        let _ = (check_id, namespace, group, service, ip, port, cluster);
    }
}

/// Plugin naming store for protocol-specific service registration.
///
/// Each protocol adapter plugin can register its own naming store to manage
/// service data independently from the core Batata naming service.
///
/// Key design:
/// - **Core (Batata)** data lives in `NamingService` — the batata core.
/// - **Plugin** data lives in plugin-owned stores — completely isolated.
/// - No data conversion between core and plugins.
/// - Each plugin defines its own key format and data model.
///
/// # Key Format
///
/// Plugins define their own key hierarchy. Examples:
/// - Consul: `"dc1/web-api/instance-1"`
/// - Eureka: `"app-name/instance-id"`
///
/// # Data Format
///
/// Data is stored as `bytes::Bytes` — plugins serialize/deserialize
/// their own types. The store doesn't interpret the content.
#[async_trait::async_trait]
pub trait PluginNamingStore: Send + Sync {
    /// Unique plugin identifier (e.g., "consul", "eureka")
    fn plugin_id(&self) -> &str;

    /// Register a service entry
    fn register(&self, key: &str, data: bytes::Bytes) -> Result<(), PluginNamingStoreError>;

    /// Deregister a service entry
    fn deregister(&self, key: &str) -> Result<(), PluginNamingStoreError>;

    /// Get a single entry by key
    fn get(&self, key: &str) -> Option<bytes::Bytes>;

    /// Scan entries by key prefix
    fn scan(&self, prefix: &str) -> Vec<(String, bytes::Bytes)>;

    /// Get all keys
    fn keys(&self) -> Vec<String>;

    /// Total entry count
    fn len(&self) -> usize;

    /// Whether the store is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // ================================================================
    // Cluster Sync — for Raft snapshot and Distro verification
    // ================================================================

    /// Create a snapshot of all data (for Raft snapshot / new node join)
    async fn snapshot(&self) -> bytes::Bytes;

    /// Restore from a snapshot
    async fn restore(&self, data: &[u8]) -> Result<(), PluginNamingStoreError>;

    /// Current revision number (monotonically increasing on writes)
    fn revision(&self) -> u64;
}

/// Errors from plugin naming store operations
#[derive(Debug, thiserror::Error)]
pub enum PluginNamingStoreError {
    /// The key being registered already exists.
    #[error("Key already exists: {0}")]
    AlreadyExists(String),

    /// The requested key was not found.
    #[error("Key not found: {0}")]
    NotFound(String),

    /// Serialization or deserialization of stored data failed.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Underlying storage backend reported an error.
    #[error("Storage error: {0}")]
    Storage(String),
}

/// Plugin state provider for exposing plugin configuration to server state APIs.
///
/// Protocol adapter plugins (Consul, Eureka, etc.) implement this trait to
/// contribute their configuration state (enabled, port, version, etc.) to
/// the server state endpoint without the core needing to know about specific plugins.
///
/// # Example
///
/// ```rust,ignore
/// impl PluginStateProvider for ConsulPlugin {
///     fn plugin_state(&self) -> HashMap<String, Option<String>> {
///         let mut state = HashMap::new();
///         state.insert("consul_enabled".into(), Some("true".into()));
///         state.insert("consul_port".into(), Some("8500".into()));
///         state
///     }
/// }
/// ```
pub trait PluginStateProvider: Send + Sync {
    /// Return plugin state as key-value pairs for server state API responses.
    fn plugin_state(&self) -> HashMap<String, Option<String>>;
}

/// Plugin manager for registering and invoking plugins
pub struct PluginManager {
    auth_plugins: Vec<Arc<dyn AuthPlugin>>,
    config_change_plugins: Vec<Arc<dyn ConfigChangePlugin>>,
    config_change_plugins_v2: Vec<Arc<dyn ConfigChangePluginV2>>,
    control_plugins: Vec<Arc<dyn ControlPlugin>>,
    protocol_adapters: Vec<Arc<dyn ProtocolAdapterPlugin>>,
}

impl PluginManager {
    /// Create a new, empty plugin manager.
    pub fn new() -> Self {
        Self {
            auth_plugins: Vec::new(),
            config_change_plugins: Vec::new(),
            config_change_plugins_v2: Vec::new(),
            control_plugins: Vec::new(),
            protocol_adapters: Vec::new(),
        }
    }

    /// Register an auth plugin (sorted by priority, ascending).
    pub fn register_auth(&mut self, plugin: Arc<dyn AuthPlugin>) {
        self.auth_plugins.push(plugin);
        self.auth_plugins.sort_by_key(|p| p.priority());
    }

    /// Registers a config change plugin, sorted by priority (ascending).
    pub fn register_config_change(&mut self, plugin: Arc<dyn ConfigChangePlugin>) {
        self.config_change_plugins.push(plugin);
        self.config_change_plugins.sort_by_key(|p| p.priority());
    }

    /// Registers a V2 config change plugin, sorted by order (ascending).
    pub fn register_config_change_v2(&mut self, plugin: Arc<dyn ConfigChangePluginV2>) {
        self.config_change_plugins_v2.push(plugin);
        self.config_change_plugins_v2.sort_by_key(|p| p.order());
    }

    /// Execute V2 plugin chain for a config change operation.
    /// Supports Before/After, Pointcut filtering, and parameter replacement.
    pub async fn execute_config_change_v2(
        &self,
        mut request: ConfigChangeRequest,
    ) -> ConfigChangeResponse {
        for plugin in &self.config_change_plugins_v2 {
            if !plugin.is_enabled() {
                continue;
            }
            if !plugin.interested_pointcuts().contains(&request.pointcut) {
                continue;
            }
            if !plugin
                .interested_execute_types()
                .contains(&request.execute_type)
            {
                continue;
            }
            let result = plugin.execute(&mut request).await;
            if !result.allowed {
                return ConfigChangeResponse::deny(
                    result.message.as_deref().unwrap_or("plugin denied"),
                );
            }
            if !request.proceed {
                return ConfigChangeResponse::deny(
                    request.deny_reason.as_deref().unwrap_or("plugin denied"),
                );
            }
        }
        ConfigChangeResponse::allow_with_request(request)
    }

    /// Registers a control plugin, sorted by priority (ascending).
    pub fn register_control(&mut self, plugin: Arc<dyn ControlPlugin>) {
        self.control_plugins.push(plugin);
        self.control_plugins.sort_by_key(|p| p.priority());
    }

    /// Execute all config change plugins before publish
    pub async fn before_config_publish(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
    ) -> ConfigChangeResult {
        for plugin in &self.config_change_plugins {
            if !plugin.is_enabled() {
                continue;
            }
            let result = plugin.before_publish(data_id, group, tenant, content).await;
            if !result.allowed {
                return result;
            }
        }
        ConfigChangeResult::allow()
    }

    /// Execute all config change plugins after publish
    pub async fn after_config_publish(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
    ) {
        for plugin in &self.config_change_plugins {
            if plugin.is_enabled() {
                plugin.after_publish(data_id, group, tenant, content).await;
            }
        }
    }

    /// Check TPS across all control plugins
    pub async fn check_tps(&self, point_name: &str, client_ip: &str) -> TpsCheckResult {
        for plugin in &self.control_plugins {
            if !plugin.is_enabled() {
                continue;
            }
            let result = plugin.check_tps(point_name, client_ip).await;
            if !result.allowed {
                return result;
            }
        }
        TpsCheckResult {
            allowed: true,
            remaining: u64::MAX,
            limit: u64::MAX,
        }
    }

    /// Registers a protocol adapter plugin, sorted by priority (ascending).
    pub fn register_protocol_adapter(&mut self, plugin: Arc<dyn ProtocolAdapterPlugin>) {
        tracing::info!(
            "Registered protocol adapter: {} (protocol={}, port={})",
            plugin.name(),
            plugin.protocol(),
            plugin.default_port()
        );
        self.protocol_adapters.push(plugin);
        self.protocol_adapters.sort_by_key(|p| p.priority());
    }

    /// Initialize all registered protocol adapters with the given context.
    pub async fn init_protocol_adapters(&self, ctx: &PluginContext) -> anyhow::Result<()> {
        for plugin in &self.protocol_adapters {
            if plugin.is_enabled() {
                plugin.init(ctx).await?;
            }
        }
        Ok(())
    }

    /// Start background tasks for all registered protocol adapters
    pub async fn start_protocol_adapter_tasks(&self) -> anyhow::Result<()> {
        for plugin in &self.protocol_adapters {
            if plugin.is_enabled() {
                plugin.start_background_tasks().await?;
            }
        }
        Ok(())
    }

    /// Shut down all registered protocol adapters
    pub async fn shutdown_protocol_adapters(&self) -> anyhow::Result<()> {
        for plugin in self.protocol_adapters.iter().rev() {
            if plugin.is_enabled() {
                plugin.shutdown().await?;
            }
        }
        Ok(())
    }

    /// Returns all registered auth plugins in priority order.
    pub fn auth_plugins(&self) -> &[Arc<dyn AuthPlugin>] {
        &self.auth_plugins
    }

    /// Returns all registered config change plugins in priority order.
    pub fn config_change_plugins(&self) -> &[Arc<dyn ConfigChangePlugin>] {
        &self.config_change_plugins
    }

    /// Returns all registered V2 config change plugins in order.
    pub fn config_change_plugins_v2(&self) -> &[Arc<dyn ConfigChangePluginV2>] {
        &self.config_change_plugins_v2
    }

    /// Returns all registered control plugins in priority order.
    pub fn control_plugins(&self) -> &[Arc<dyn ControlPlugin>] {
        &self.control_plugins
    }

    /// Returns all registered protocol adapter plugins in priority order.
    pub fn protocol_adapters(&self) -> &[Arc<dyn ProtocolAdapterPlugin>] {
        &self.protocol_adapters
    }

    /// Collect all column family names required by enabled protocol adapters.
    ///
    /// Call this BEFORE creating the Raft state machine so that RocksDB can
    /// create these CFs at open time.
    pub fn collect_plugin_column_families(&self) -> Vec<String> {
        self.protocol_adapters
            .iter()
            .filter(|p| p.is_enabled())
            .flat_map(|p| p.required_column_families())
            .collect()
    }

    /// Get a protocol adapter by protocol name
    pub fn get_protocol_adapter(&self, protocol: &str) -> Option<&Arc<dyn ProtocolAdapterPlugin>> {
        self.protocol_adapters
            .iter()
            .find(|p| p.protocol() == protocol && p.is_enabled())
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestAuthPlugin;

    impl Plugin for TestAuthPlugin {
        fn name(&self) -> &str {
            "test-auth"
        }
        fn plugin_type(&self) -> &str {
            "auth"
        }
        fn priority(&self) -> i32 {
            10
        }
    }

    #[async_trait::async_trait]
    impl AuthPlugin for TestAuthPlugin {
        async fn authenticate(
            &self,
            username: &str,
            _password: &str,
            _props: &HashMap<String, String>,
        ) -> AuthPluginResult {
            AuthPluginResult {
                success: username == "admin",
                username: username.to_string(),
                message: None,
                properties: HashMap::new(),
            }
        }
        async fn authorize(&self, _username: &str, _resource: &str, _action: &str) -> bool {
            true
        }
    }

    struct TestConfigPlugin;

    impl Plugin for TestConfigPlugin {
        fn name(&self) -> &str {
            "test-config"
        }
        fn plugin_type(&self) -> &str {
            "config-change"
        }
    }

    #[async_trait::async_trait]
    impl ConfigChangePlugin for TestConfigPlugin {
        async fn before_publish(
            &self,
            _data_id: &str,
            _group: &str,
            _tenant: &str,
            content: &str,
        ) -> ConfigChangeResult {
            if content.contains("FORBIDDEN") {
                ConfigChangeResult::deny("Content contains forbidden keyword")
            } else {
                ConfigChangeResult::allow()
            }
        }
        async fn after_publish(&self, _data_id: &str, _group: &str, _tenant: &str, _content: &str) {
        }
        async fn before_delete(
            &self,
            _data_id: &str,
            _group: &str,
            _tenant: &str,
        ) -> ConfigChangeResult {
            ConfigChangeResult::allow()
        }
        async fn after_delete(&self, _data_id: &str, _group: &str, _tenant: &str) {}
    }

    #[tokio::test]
    async fn test_plugin_manager_config_change() {
        let mut manager = PluginManager::new();
        manager.register_config_change(Arc::new(TestConfigPlugin));

        let result = manager
            .before_config_publish("app.yaml", "DEFAULT_GROUP", "public", "key: value")
            .await;
        assert!(result.allowed);

        let result = manager
            .before_config_publish("app.yaml", "DEFAULT_GROUP", "public", "FORBIDDEN content")
            .await;
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_plugin_manager_auth() {
        let mut manager = PluginManager::new();
        manager.register_auth(Arc::new(TestAuthPlugin));

        let result = manager.auth_plugins()[0]
            .authenticate("admin", "pass", &HashMap::new())
            .await;
        assert!(result.success);

        let result = manager.auth_plugins()[0]
            .authenticate("user", "pass", &HashMap::new())
            .await;
        assert!(!result.success);
    }

    #[test]
    fn test_config_change_result() {
        let allow = ConfigChangeResult::allow();
        assert!(allow.allowed);
        assert!(allow.message.is_none());

        let deny = ConfigChangeResult::deny("not allowed");
        assert!(!deny.allowed);
        assert_eq!(deny.message.as_deref(), Some("not allowed"));
    }

    #[test]
    fn test_plugin_priority_ordering() {
        let mut manager = PluginManager::new();

        struct LowPriority;
        impl Plugin for LowPriority {
            fn name(&self) -> &str {
                "low"
            }
            fn plugin_type(&self) -> &str {
                "auth"
            }
            fn priority(&self) -> i32 {
                100
            }
        }
        #[async_trait::async_trait]
        impl AuthPlugin for LowPriority {
            async fn authenticate(
                &self,
                _: &str,
                _: &str,
                _: &HashMap<String, String>,
            ) -> AuthPluginResult {
                AuthPluginResult {
                    success: true,
                    username: String::new(),
                    message: None,
                    properties: HashMap::new(),
                }
            }
            async fn authorize(&self, _: &str, _: &str, _: &str) -> bool {
                true
            }
        }

        struct HighPriority;
        impl Plugin for HighPriority {
            fn name(&self) -> &str {
                "high"
            }
            fn plugin_type(&self) -> &str {
                "auth"
            }
            fn priority(&self) -> i32 {
                1
            }
        }
        #[async_trait::async_trait]
        impl AuthPlugin for HighPriority {
            async fn authenticate(
                &self,
                _: &str,
                _: &str,
                _: &HashMap<String, String>,
            ) -> AuthPluginResult {
                AuthPluginResult {
                    success: true,
                    username: String::new(),
                    message: None,
                    properties: HashMap::new(),
                }
            }
            async fn authorize(&self, _: &str, _: &str, _: &str) -> bool {
                true
            }
        }

        // Register in reverse order
        manager.register_auth(Arc::new(LowPriority));
        manager.register_auth(Arc::new(HighPriority));

        // High priority should be first
        assert_eq!(manager.auth_plugins()[0].name(), "high");
        assert_eq!(manager.auth_plugins()[1].name(), "low");
    }

    #[test]
    fn test_protocol_adapter_registration() {
        struct TestAdapter {
            enabled: bool,
        }

        #[async_trait::async_trait]
        impl ProtocolAdapterPlugin for TestAdapter {
            fn name(&self) -> &str {
                "test-consul"
            }
            fn protocol(&self) -> &str {
                "consul"
            }
            fn is_enabled(&self) -> bool {
                self.enabled
            }
            fn default_port(&self) -> u16 {
                8500
            }
            fn configure(&self, _cfg: &mut actix_web::web::ServiceConfig) {
                // No routes in test
            }
        }

        let mut manager = PluginManager::new();
        manager.register_protocol_adapter(Arc::new(TestAdapter { enabled: true }));

        assert_eq!(manager.protocol_adapters().len(), 1);
        assert_eq!(manager.protocol_adapters()[0].name(), "test-consul");
        assert_eq!(manager.protocol_adapters()[0].protocol(), "consul");
        assert_eq!(manager.protocol_adapters()[0].default_port(), 8500);

        let adapter = manager.get_protocol_adapter("consul");
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), "test-consul");

        // Disabled adapter should not be found
        let mut manager2 = PluginManager::new();
        manager2.register_protocol_adapter(Arc::new(TestAdapter { enabled: false }));
        assert!(manager2.get_protocol_adapter("consul").is_none());

        // Non-existent protocol should return None
        assert!(manager.get_protocol_adapter("eureka").is_none());
    }

    struct ShutdownOrderAdapter {
        id: &'static str,
        priority: i32,
        order: Arc<std::sync::Mutex<Vec<&'static str>>>,
    }

    #[async_trait::async_trait]
    impl ProtocolAdapterPlugin for ShutdownOrderAdapter {
        fn name(&self) -> &str {
            self.id
        }

        fn protocol(&self) -> &str {
            self.id
        }

        fn is_enabled(&self) -> bool {
            true
        }

        fn priority(&self) -> i32 {
            self.priority
        }

        fn default_port(&self) -> u16 {
            0
        }

        fn configure(&self, _cfg: &mut actix_web::web::ServiceConfig) {}

        async fn shutdown(&self) -> anyhow::Result<()> {
            self.order
                .lock()
                .expect("shutdown order mutex poisoned")
                .push(self.id);
            Ok(())
        }
    }

    #[tokio::test]
    async fn protocol_adapters_shutdown_in_reverse_priority_order() {
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut manager = PluginManager::new();
        manager.register_protocol_adapter(Arc::new(ShutdownOrderAdapter {
            id: "late",
            priority: 20,
            order: order.clone(),
        }));
        manager.register_protocol_adapter(Arc::new(ShutdownOrderAdapter {
            id: "early",
            priority: 10,
            order: order.clone(),
        }));

        manager
            .shutdown_protocol_adapters()
            .await
            .expect("shutdown must succeed");

        assert_eq!(
            *order.lock().expect("shutdown order mutex poisoned"),
            vec!["late", "early"],
            "teardown must be the reverse of startup priority"
        );
    }

    #[test]
    fn plugin_context_required_value_distinguishes_missing_and_wrong_type() {
        let mut ctx = PluginContext::new();

        assert_eq!(
            ctx.get_required::<bool>(context_keys::IS_CLUSTER),
            Err(PluginContextError::Missing {
                key: context_keys::IS_CLUSTER.to_string(),
            }),
            "an absent required dependency must be named explicitly"
        );

        ctx.insert(context_keys::IS_CLUSTER, Arc::new(1_u32));
        assert!(
            ctx.contains(context_keys::IS_CLUSTER),
            "wrong-type values must still be visible as present"
        );
        assert!(
            ctx.get::<bool>(context_keys::IS_CLUSTER).is_none(),
            "legacy getter must preserve its previous None-on-mismatch behavior"
        );
        assert_eq!(
            ctx.get_required::<bool>(context_keys::IS_CLUSTER),
            Err(PluginContextError::WrongType {
                key: context_keys::IS_CLUSTER.to_string(),
                expected: "bool",
                actual: "u32",
            }),
            "a wrong concrete type must not be reported as missing"
        );
    }

    #[test]
    fn plugin_context_describe_reports_stable_key_type_map() {
        let mut ctx = PluginContext::new();
        ctx.insert(context_keys::STORAGE_MODE, Arc::new("embedded".to_string()));
        ctx.insert(context_keys::IS_CLUSTER, Arc::new(true));

        let described = ctx.describe();
        assert_eq!(
            described.keys().cloned().collect::<Vec<_>>(),
            vec![
                context_keys::IS_CLUSTER.to_string(),
                context_keys::STORAGE_MODE.to_string(),
            ],
            "descriptions must be lexicographically ordered for stable logs"
        );
        assert_eq!(described[context_keys::IS_CLUSTER], "bool");
        assert_eq!(
            described[context_keys::STORAGE_MODE],
            "alloc::string::String"
        );
    }

    #[test]
    fn plugin_context_shared_dependency_supports_trait_objects() {
        trait Greeter: Send + Sync {
            fn greet(&self) -> &'static str;
        }
        struct Hello;
        impl Greeter for Hello {
            fn greet(&self) -> &'static str {
                "hello"
            }
        }

        let mut ctx = PluginContext::new();
        ctx.insert_shared("greeter", Arc::new(Hello) as Arc<dyn Greeter>);

        let greeter = ctx
            .get_shared_required::<dyn Greeter>("greeter")
            .expect("a shared trait object must be retrievable");
        assert_eq!(greeter.greet(), "hello");
        assert!(
            ctx.get_shared::<dyn Greeter>("missing").is_none(),
            "optional shared lookup must stay None on a missing key"
        );
    }

    #[test]
    fn plugin_context_new_is_empty() {
        let ctx = PluginContext::new();
        assert!(ctx.describe().is_empty());
        assert!(!ctx.contains(context_keys::IS_CLUSTER));
        assert!(ctx.get::<bool>(context_keys::IS_CLUSTER).is_none());
    }

    #[test]
    fn plugin_context_insert_overwrites_same_key() {
        let mut ctx = PluginContext::new();
        ctx.insert(context_keys::IS_CLUSTER, Arc::new(true));
        ctx.insert(context_keys::IS_CLUSTER, Arc::new(false));
        assert!(
            !*ctx
                .get_required::<bool>(context_keys::IS_CLUSTER)
                .expect("latest value must win")
        );
    }

    #[test]
    fn plugin_context_supports_multiple_distinct_keys() {
        let mut ctx = PluginContext::new();
        ctx.insert(context_keys::IS_CLUSTER, Arc::new(true));
        ctx.insert("retry_count", Arc::new(3_u32));
        ctx.insert(context_keys::STORAGE_MODE, Arc::new("embedded".to_string()));

        assert!(*ctx.get_required::<bool>(context_keys::IS_CLUSTER).unwrap());
        assert_eq!(*ctx.get_required::<u32>("retry_count").unwrap(), 3);
        assert_eq!(ctx.describe().len(), 3);
    }

    #[test]
    fn plugin_context_optional_get_stays_none_on_wrong_type() {
        let mut ctx = PluginContext::new();
        ctx.insert("retry_count", Arc::new(3_u32));

        assert!(ctx.get::<bool>("retry_count").is_none());
        assert_eq!(*ctx.get::<u32>("retry_count").unwrap(), 3);
    }

    #[test]
    fn plugin_context_required_error_display_is_actionable() {
        let missing = PluginContextError::Missing {
            key: context_keys::RAFT_NODE.to_string(),
        };
        let message = missing.to_string();
        assert!(message.contains(context_keys::RAFT_NODE));
        assert!(message.contains("missing"));

        let wrong = PluginContextError::WrongType {
            key: context_keys::STORAGE_MODE.to_string(),
            expected: "bool",
            actual: "u32",
        };
        let message = wrong.to_string();
        assert!(message.contains(context_keys::STORAGE_MODE));
        assert!(message.contains("expected"));
        assert!(message.contains("has type"));
    }

    #[test]
    fn plugin_context_missing_shared_dependency_is_reported() {
        trait Marker: Send + Sync {}

        let ctx = PluginContext::new();
        assert!(ctx.get_shared::<dyn Marker>("missing").is_none());
        assert!(
            matches!(
                ctx.get_shared_required::<dyn Marker>("missing"),
                Err(PluginContextError::Missing { ref key }) if key == "missing"
            ),
            "a missing shared dependency must preserve the canonical key"
        );
    }

    #[test]
    fn plugin_context_shared_wrong_type_is_distinguished() {
        trait First: Send + Sync {}
        trait Second: Send + Sync {}
        struct OnlyFirst;
        impl First for OnlyFirst {}

        let mut ctx = PluginContext::new();
        ctx.insert_shared("dep", Arc::new(OnlyFirst) as Arc<dyn First>);

        assert!(
            matches!(
                ctx.get_shared_required::<dyn Second>("dep"),
                Err(PluginContextError::WrongType { .. })
            ),
            "a trait-object mismatch must not be reported as missing"
        );
    }

    #[test]
    fn plugin_context_describe_reports_inserted_concrete_types() {
        let mut ctx = PluginContext::new();
        ctx.insert("flag", Arc::new(true));
        ctx.insert("count", Arc::new(7_u64));

        let described = ctx.describe();
        assert_eq!(described["flag"], "bool");
        assert_eq!(described["count"], "u64");
    }
}
