//! Plugin SPI (Service Provider Interface) definitions
//!
//! Standard plugin interfaces for extending Batata functionality.
//! Each plugin type has a trait that implementations must satisfy.

use std::any::Any;
use std::collections::HashMap;
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
    pub success: bool,
    pub username: String,
    pub message: Option<String>,
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
    pub allowed: bool,
    pub message: Option<String>,
}

impl ConfigChangeResult {
    pub fn allow() -> Self {
        Self {
            allowed: true,
            message: None,
        }
    }

    pub fn deny(message: &str) -> Self {
        Self {
            allowed: false,
            message: Some(message.to_string()),
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
    pub allowed: bool,
    pub remaining: u64,
    pub limit: u64,
}

/// Context provided to protocol adapter plugins during initialization.
///
/// Carries server resources as type-erased `Any` trait objects to avoid
/// coupling the plugin SPI to specific infrastructure crates.
///
/// Well-known keys:
/// - `"raft_node"`: `Arc<batata_consistency::RaftNode>` (only in cluster mode)
/// - `"configuration"`: `Arc<Configuration>`
/// - `"is_cluster"`: `Arc<bool>`
pub struct PluginContext {
    extensions: HashMap<String, Arc<dyn Any + Send + Sync>>,
}

impl PluginContext {
    pub fn new() -> Self {
        Self {
            extensions: HashMap::new(),
        }
    }

    /// Insert a typed value into the context.
    pub fn insert<T: Send + Sync + 'static>(&mut self, key: &str, value: Arc<T>) {
        self.extensions.insert(key.to_string(), value);
    }

    /// Get a typed reference from the context. Returns None if key missing or wrong type.
    pub fn get<T: Send + Sync + 'static>(&self, key: &str) -> Option<Arc<T>> {
        self.extensions
            .get(key)
            .and_then(|v| v.clone().downcast::<T>().ok())
    }

    /// Check if a key exists.
    pub fn contains(&self, key: &str) -> bool {
        self.extensions.contains_key(key)
    }
}

impl Default for PluginContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Protocol adapter plugin for compatibility layers (e.g., Consul, Eureka, Apollo).
///
/// Protocol adapters translate external protocols into Batata-native (Nacos) operations.
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

    /// Called when an instance should be deregistered (critical too long).
    ///
    /// Default: no-op. Override for auto-deregister behavior.
    fn on_deregister(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        ip: &str,
        port: i32,
        cluster: &str,
    ) {
        let _ = (namespace, group, service, ip, port, cluster);
    }
}

/// Plugin naming store for protocol-specific service registration.
///
/// Each protocol adapter plugin can register its own naming store to manage
/// service data independently from the core Nacos naming service.
///
/// Key design:
/// - **Core (Nacos)** data lives in `NamingService` — the batata core.
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
    #[error("Key already exists: {0}")]
    AlreadyExists(String),

    #[error("Key not found: {0}")]
    NotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Storage error: {0}")]
    Storage(String),
}

/// Plugin manager for registering and invoking plugins
pub struct PluginManager {
    auth_plugins: Vec<Arc<dyn AuthPlugin>>,
    config_change_plugins: Vec<Arc<dyn ConfigChangePlugin>>,
    control_plugins: Vec<Arc<dyn ControlPlugin>>,
    protocol_adapters: Vec<Arc<dyn ProtocolAdapterPlugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            auth_plugins: Vec::new(),
            config_change_plugins: Vec::new(),
            control_plugins: Vec::new(),
            protocol_adapters: Vec::new(),
        }
    }

    pub fn register_auth(&mut self, plugin: Arc<dyn AuthPlugin>) {
        self.auth_plugins.push(plugin);
        self.auth_plugins.sort_by_key(|p| p.priority());
    }

    pub fn register_config_change(&mut self, plugin: Arc<dyn ConfigChangePlugin>) {
        self.config_change_plugins.push(plugin);
        self.config_change_plugins.sort_by_key(|p| p.priority());
    }

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

    pub fn auth_plugins(&self) -> &[Arc<dyn AuthPlugin>] {
        &self.auth_plugins
    }

    pub fn config_change_plugins(&self) -> &[Arc<dyn ConfigChangePlugin>] {
        &self.config_change_plugins
    }

    pub fn control_plugins(&self) -> &[Arc<dyn ControlPlugin>] {
        &self.control_plugins
    }

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
}
