//! ConsulPlugin - Protocol adapter plugin for Consul API compatibility.
//!
//! Implements the `Plugin` and `ProtocolAdapterPlugin` traits, encapsulating
//! all Consul services, routes, and app_data configuration.
//!
//! Supports two-phase initialization:
//! 1. Construction via `from_config()` — lightweight, no DB or Raft needed
//! 2. Initialization via `init(ctx)` — creates stores, registers Raft, builds services

use std::sync::Arc;
use std::time::Duration;

use rocksdb::DB;

use batata_naming::InstanceCheckRegistry;
use batata_naming::healthcheck::{deregister_monitor::DeregisterMonitor, ttl_monitor::TtlMonitor};
use batata_plugin::ProtocolAdapterPlugin;

use crate::acl::AclService;
use crate::agent::ConsulAgentService;
use crate::catalog::ConsulCatalogService;
use crate::config_entry::ConsulConfigEntryService;
use crate::connect::ConsulConnectService;
use crate::connect_ca::ConsulConnectCAService;
use crate::coordinate::ConsulCoordinateService;
use crate::event::ConsulEventService;
use crate::health::ConsulHealthService;
use crate::index_provider::{ConsulIndexProvider, ConsulTableIndex};
use crate::kv::ConsulKVService;
use crate::lock::{ConsulLockService, ConsulSemaphoreService};
use crate::model::ConsulDatacenterConfig;
use crate::operator::ConsulOperatorService;
use crate::peering::ConsulPeeringService;
use crate::query::ConsulQueryService;
use crate::raft::ConsulRaftWriter;
use crate::session::ConsulSessionService;
use crate::snapshot::ConsulSnapshotService;

/// Inner state holding all initialized Consul services.
///
/// Created during `init()` and stored in the `OnceLock`. Derives `Clone`
/// so that `configure()` can create `web::Data` instances for each HTTP worker.
#[derive(Clone)]
pub struct ConsulPluginInner {
    pub naming_store: Arc<crate::naming_store::ConsulNamingStore>,
    pub agent: ConsulAgentService,
    pub health: ConsulHealthService,
    pub kv: ConsulKVService,
    pub catalog: ConsulCatalogService,
    pub acl: AclService,
    pub session: ConsulSessionService,
    pub event: ConsulEventService,
    pub query: ConsulQueryService,
    pub lock: ConsulLockService,
    pub semaphore: ConsulSemaphoreService,
    pub peering: Arc<ConsulPeeringService>,
    pub config_entry: ConsulConfigEntryService,
    pub connect: ConsulConnectService,
    pub connect_ca: ConsulConnectCAService,
    pub coordinate: ConsulCoordinateService,
    pub snapshot: ConsulSnapshotService,
    pub operator: ConsulOperatorService,
    pub namespace_service: crate::namespace::ConsulNamespaceService,
    pub dc_config: ConsulDatacenterConfig,
    pub index_provider: ConsulIndexProvider,
    pub registry: Arc<InstanceCheckRegistry>,
}

/// Consul compatibility protocol adapter plugin.
///
/// Supports two-phase initialization:
/// - Phase 1: `from_config()` creates a lightweight plugin with only config
/// - Phase 2: `init(ctx)` reads Raft/cluster info from context, builds all services
///
/// Legacy constructors (`new()`, `with_consul_raft()`) are preserved for backward
/// compatibility but will be removed once `consul_init.rs` is deleted.
pub struct ConsulPlugin {
    /// Whether this plugin is enabled.
    enabled: bool,
    /// Whether ACL is enabled.
    acl_enabled: bool,
    /// Initial management token for ACL bootstrap (like Consul's `acl.tokens.initial_management`).
    initial_management_token: Option<String>,
    /// Datacenter configuration.
    pub dc_config: ConsulDatacenterConfig,
    /// Whether to auto-register the Consul service on startup.
    register_self: bool,
    /// Lazily initialized inner state (populated during `init()`).
    inner: std::sync::OnceLock<ConsulPluginInner>,
}

impl ConsulPlugin {
    // ========================================================================
    // New two-phase constructor
    // ========================================================================

    /// Creates a lightweight ConsulPlugin from configuration only.
    ///
    /// No database, Raft, or heavy services are created here. Call `init(ctx)`
    /// to complete initialization with server context.
    pub fn from_config(
        enabled: bool,
        acl_enabled: bool,
        initial_management_token: Option<String>,
        dc_config: ConsulDatacenterConfig,
        register_self: bool,
        _server_address: String,
        _server_port: u16,
    ) -> Self {
        Self {
            enabled,
            acl_enabled,
            initial_management_token,
            dc_config,
            register_self,
            inner: std::sync::OnceLock::new(),
        }
    }

    /// Get the initialized inner state. Panics if `init()` has not been called.
    fn inner(&self) -> &ConsulPluginInner {
        self.inner
            .get()
            .expect("ConsulPlugin::init() must be called before accessing services")
    }

    /// Build inner services with in-memory storage (standalone mode).
    fn build_inner_standalone(
        &self,
        naming_store: Arc<crate::naming_store::ConsulNamingStore>,
        registry: Arc<InstanceCheckRegistry>,
    ) -> ConsulPluginInner {
        let index_provider = ConsulIndexProvider::new();
        let session = ConsulSessionService::new().with_node_name(self.dc_config.node_name.clone());
        let kv = ConsulKVService::new();
        let kv_arc = Arc::new(kv.clone());
        let session_arc = Arc::new(session.clone());
        let lock = ConsulLockService::new(kv_arc.clone(), session_arc.clone());
        let semaphore = ConsulSemaphoreService::new(kv_arc, session_arc);
        let check_index = Arc::new(crate::check_index::ConsulCheckIndex::new());

        ConsulPluginInner {
            naming_store: naming_store.clone(),
            agent: ConsulAgentService::new(
                naming_store.clone(),
                registry.clone(),
                check_index.clone(),
            ),
            health: ConsulHealthService::new(registry.clone(), check_index)
                .with_node_name(self.dc_config.node_name.clone()),
            kv,
            catalog: ConsulCatalogService::with_dc_config(naming_store.clone(), &self.dc_config)
                .with_index_provider(index_provider.clone()),
            namespace_service: crate::namespace::ConsulNamespaceService::new(
                index_provider.clone(),
            ),
            event: ConsulEventService::new(index_provider.clone()),
            index_provider,
            acl: if self.acl_enabled {
                match &self.initial_management_token {
                    Some(token) => AclService::with_initial_management_token(token.clone()),
                    None => AclService::new(),
                }
            } else {
                AclService::disabled()
            },
            session,
            query: ConsulQueryService::new(),
            lock,
            semaphore,
            peering: Arc::new(ConsulPeeringService::new()),
            config_entry: ConsulConfigEntryService::new(),
            connect: ConsulConnectService::new(),
            connect_ca: ConsulConnectCAService::new(),
            coordinate: ConsulCoordinateService::new()
                .with_node_name(self.dc_config.node_name.clone()),
            snapshot: ConsulSnapshotService::new(),
            operator: ConsulOperatorService::with_dc_config(&self.dc_config),
            dc_config: self.dc_config.clone(),
            registry,
        }
    }

    /// Build inner services with Raft-replicated RocksDB storage (cluster mode).
    fn build_inner_cluster(
        &self,
        naming_store: Arc<crate::naming_store::ConsulNamingStore>,
        registry: Arc<InstanceCheckRegistry>,
        db: Arc<DB>,
        consul_raft: Arc<ConsulRaftWriter>,
        table_index: ConsulTableIndex,
    ) -> ConsulPluginInner {
        let index_provider: ConsulIndexProvider = table_index;
        let session = ConsulSessionService::with_raft(db.clone(), consul_raft.clone())
            .with_node_name(self.dc_config.node_name.clone());
        let kv = ConsulKVService::with_raft(db.clone(), consul_raft.clone());
        let kv_arc = Arc::new(kv.clone());
        let session_arc = Arc::new(session.clone());
        let lock = ConsulLockService::new(kv_arc.clone(), session_arc.clone());
        let semaphore = ConsulSemaphoreService::new(kv_arc, session_arc);
        let check_index = Arc::new(crate::check_index::ConsulCheckIndex::new());

        ConsulPluginInner {
            naming_store: naming_store.clone(),
            agent: ConsulAgentService::new(
                naming_store.clone(),
                registry.clone(),
                check_index.clone(),
            ),
            health: ConsulHealthService::new(registry.clone(), check_index)
                .with_node_name(self.dc_config.node_name.clone()),
            kv,
            catalog: ConsulCatalogService::with_dc_config(naming_store.clone(), &self.dc_config)
                .with_index_provider(index_provider.clone()),
            acl: if self.acl_enabled {
                AclService::with_raft(db.clone(), consul_raft.clone())
            } else {
                AclService::disabled()
            },
            session,
            event: ConsulEventService::new(index_provider.clone()),
            query: ConsulQueryService::with_raft(db.clone(), consul_raft.clone()),
            lock,
            semaphore,
            peering: Arc::new(ConsulPeeringService::with_raft(
                db.clone(),
                consul_raft.clone(),
                self.dc_config.datacenter.clone(),
                self.dc_config.consul_port,
            )),
            config_entry: ConsulConfigEntryService::with_raft(db.clone(), consul_raft.clone()),
            connect: ConsulConnectService::new(),
            connect_ca: ConsulConnectCAService::with_raft(db.clone(), consul_raft.clone()),
            coordinate: ConsulCoordinateService::with_raft(
                db.clone(),
                consul_raft.clone(),
                self.dc_config.datacenter.clone(),
            )
            .with_node_name(self.dc_config.node_name.clone()),
            snapshot: ConsulSnapshotService::with_rocks(db.clone()),
            operator: ConsulOperatorService::with_raft(db, consul_raft.clone(), &self.dc_config),
            namespace_service: crate::namespace::ConsulNamespaceService::with_raft(
                consul_raft,
                index_provider.clone(),
            ),
            dc_config: self.dc_config.clone(),
            index_provider,
            registry,
        }
    }

    // ========================================================================
    // Legacy constructors (backward compatibility with consul_init.rs)
    // ========================================================================

    /// Creates Consul plugin with in-memory storage.
    ///
    /// **Deprecated**: Use `from_config()` + `init()` instead. Kept for backward
    /// compatibility with `consul_init.rs`.
    ///
    /// If `naming_store` is provided, uses it. Otherwise creates a new one.
    pub fn new(
        naming_store: Option<Arc<crate::naming_store::ConsulNamingStore>>,
        registry: Arc<InstanceCheckRegistry>,
        acl_enabled: bool,
        dc_config: ConsulDatacenterConfig,
    ) -> Self {
        let naming_store =
            naming_store.unwrap_or_else(|| Arc::new(crate::naming_store::ConsulNamingStore::new()));

        let plugin = Self {
            enabled: true,
            acl_enabled,
            initial_management_token: None,
            dc_config: dc_config.clone(),
            register_self: false,
            inner: std::sync::OnceLock::new(),
        };

        let inner = plugin.build_inner_standalone(naming_store, registry);
        let _ = plugin.inner.set(inner);
        plugin
    }

    /// Creates Consul plugin with Raft-replicated RocksDB storage (cluster mode).
    ///
    /// **Deprecated**: Use `from_config()` + `init()` instead. Kept for backward
    /// compatibility with `consul_init.rs`.
    ///
    /// Uses `ConsulRaftWriter` to route Consul writes through the core Raft group
    /// via `PluginWrite` instead of a separate Consul-only Raft.
    pub fn with_consul_raft(
        naming_store: Option<Arc<crate::naming_store::ConsulNamingStore>>,
        registry: Arc<InstanceCheckRegistry>,
        acl_enabled: bool,
        db: Arc<DB>,
        consul_raft: Arc<ConsulRaftWriter>,
        table_index: ConsulTableIndex,
        dc_config: ConsulDatacenterConfig,
    ) -> Self {
        let naming_store =
            naming_store.unwrap_or_else(|| Arc::new(crate::naming_store::ConsulNamingStore::new()));

        let plugin = Self {
            enabled: true,
            acl_enabled,
            initial_management_token: None,
            dc_config: dc_config.clone(),
            register_self: false,
            inner: std::sync::OnceLock::new(),
        };

        let inner =
            plugin.build_inner_cluster(naming_store, registry, db, consul_raft, table_index);
        let _ = plugin.inner.set(inner);
        plugin
    }

    // ========================================================================
    // Public accessors for backward compatibility with consul_init.rs
    // ========================================================================

    /// Get the agent service. Panics if not initialized.
    pub fn agent(&self) -> &ConsulAgentService {
        &self.inner().agent
    }

    /// Get the session service. Panics if not initialized.
    pub fn session(&self) -> &ConsulSessionService {
        &self.inner().session
    }

    /// Get the KV service. Panics if not initialized.
    pub fn kv(&self) -> &ConsulKVService {
        &self.inner().kv
    }

    /// Get the registry. Panics if not initialized.
    pub fn registry(&self) -> &Arc<InstanceCheckRegistry> {
        &self.inner().registry
    }

    /// Get the event service. Panics if not initialized.
    pub fn event_service(&self) -> &ConsulEventService {
        &self.inner().event
    }
}

// Implement the unified ProtocolAdapterPlugin SPI
#[async_trait::async_trait]
impl ProtocolAdapterPlugin for ConsulPlugin {
    fn name(&self) -> &str {
        "consul-compatibility"
    }

    fn protocol(&self) -> &str {
        "consul"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn default_port(&self) -> u16 {
        self.dc_config.consul_port
    }

    fn required_column_families(&self) -> Vec<String> {
        use crate::constants::*;
        vec![
            CF_CONSUL_KV.to_string(),
            CF_CONSUL_SESSIONS.to_string(),
            CF_CONSUL_ACL.to_string(),
            CF_CONSUL_QUERIES.to_string(),
            CF_CONSUL_CONFIG_ENTRIES.to_string(),
            CF_CONSUL_CA_ROOTS.to_string(),
            CF_CONSUL_INTENTIONS.to_string(),
            CF_CONSUL_COORDINATES.to_string(),
            CF_CONSUL_PEERING.to_string(),
            CF_CONSUL_OPERATOR.to_string(),
            CF_CONSUL_EVENTS.to_string(),
            CF_CONSUL_NAMESPACES.to_string(),
            CF_CONSUL_CATALOG.to_string(),
        ]
    }

    async fn init(&self, ctx: &batata_plugin::PluginContext) -> anyhow::Result<()> {
        // If inner is already set (legacy constructor), just log and return.
        if self.inner.get().is_some() {
            tracing::info!(
                "Consul compatibility plugin already initialized (legacy constructor) (dc={}, version={})",
                self.dc_config.datacenter,
                self.dc_config.consul_version
            );
            return Ok(());
        }

        tracing::info!(
            "Initializing Consul compatibility plugin (dc={}, version={})",
            self.dc_config.datacenter,
            self.dc_config.consul_version
        );

        // Extract cluster context
        let is_cluster = ctx.get::<bool>("is_cluster").map(|v| *v).unwrap_or(false);
        let raft_node = ctx.get::<batata_consistency::RaftNode>("raft_node");

        // Create Consul naming store and result handler
        let consul_naming_store = Arc::new(crate::naming_store::ConsulNamingStore::new());
        let consul_index_provider = Arc::new(ConsulIndexProvider::new());
        let consul_result_handler: Arc<dyn batata_plugin::HealthCheckResultHandler> =
            Arc::new(crate::result_handler::ConsulResultHandler::new(
                consul_naming_store.clone(),
                consul_index_provider.clone(),
            ));
        let consul_registry = Arc::new(InstanceCheckRegistry::new(consul_result_handler));

        let inner = if is_cluster {
            self.init_cluster(raft_node, consul_naming_store, consul_registry)
                .await
        } else {
            tracing::info!("Consul services using in-memory storage (standalone/console mode)");
            self.build_inner_standalone(consul_naming_store, consul_registry)
        };

        self.inner
            .set(inner)
            .map_err(|_| anyhow::anyhow!("ConsulPlugin::init() called more than once"))?;

        tracing::info!("Consul compatibility plugin initialized successfully");
        Ok(())
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        tracing::info!("Consul compatibility plugin shutting down");
        Ok(())
    }

    fn configure(&self, cfg: &mut actix_web::web::ServiceConfig) {
        let inner = self.inner();
        cfg.app_data(actix_web::web::Data::from(inner.naming_store.clone()))
            .app_data(actix_web::web::Data::new(inner.agent.clone()))
            .app_data(actix_web::web::Data::new(inner.health.clone()))
            .app_data(actix_web::web::Data::new(inner.kv.clone()))
            .app_data(actix_web::web::Data::new(inner.catalog.clone()))
            .app_data(actix_web::web::Data::new(inner.acl.clone()))
            .app_data(actix_web::web::Data::new(inner.session.clone()))
            .app_data(actix_web::web::Data::new(inner.event.clone()))
            .app_data(actix_web::web::Data::new(inner.query.clone()))
            .app_data(actix_web::web::Data::new(inner.lock.clone()))
            .app_data(actix_web::web::Data::new(inner.semaphore.clone()))
            .app_data(actix_web::web::Data::from(inner.peering.clone()))
            .app_data(actix_web::web::Data::new(inner.config_entry.clone()))
            .app_data(actix_web::web::Data::new(inner.connect.clone()))
            .app_data(actix_web::web::Data::new(inner.connect_ca.clone()))
            .app_data(actix_web::web::Data::new(inner.coordinate.clone()))
            .app_data(actix_web::web::Data::new(inner.snapshot.clone()))
            .app_data(actix_web::web::Data::new(inner.operator.clone()))
            .app_data(actix_web::web::Data::new(inner.namespace_service.clone()))
            .app_data(actix_web::web::Data::new(inner.dc_config.clone()))
            .app_data(actix_web::web::Data::new(inner.index_provider.clone()))
            .service(crate::route::routes());
    }

    async fn start_background_tasks(&self) -> anyhow::Result<()> {
        let inner = self.inner();

        // Auto-register Consul service if enabled
        if self.register_self {
            tracing::info!("Auto-registering Consul service...");
            let agent = inner.agent.clone();
            let dc_config = self.dc_config.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                if let Err(e) = agent.register_consul_service(&dc_config).await {
                    tracing::error!("Failed to auto-register Consul service: {}", e);
                }
            });
        }

        // TTL monitor
        tracing::info!("Starting Consul TTL monitor...");
        let ttl_monitor = TtlMonitor::new(inner.registry.clone());
        tokio::spawn(async move {
            ttl_monitor.start().await;
        });

        // Deregister monitor
        tracing::info!("Starting Consul deregister monitor...");
        let deregister_monitor = DeregisterMonitor::new(inner.registry.clone(), 30);
        tokio::spawn(async move {
            deregister_monitor.start().await;
        });

        // Session TTL cleanup
        let session_svc = inner.session.clone();
        let kv_svc = inner.kv.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let expired = session_svc.scan_expired_session_ids();
                if !expired.is_empty() {
                    tracing::info!("Cleaning up {} expired Consul sessions", expired.len());
                    for id in &expired {
                        kv_svc.release_session(id).await;
                    }
                    session_svc.cleanup_expired();
                }
            }
        });

        Ok(())
    }
}

impl ConsulPlugin {
    /// Initialize in cluster mode using the core Raft group.
    async fn init_cluster(
        &self,
        raft_node: Option<Arc<batata_consistency::RaftNode>>,
        consul_naming_store: Arc<crate::naming_store::ConsulNamingStore>,
        consul_registry: Arc<InstanceCheckRegistry>,
    ) -> ConsulPluginInner {
        let Some(core_raft) = raft_node else {
            tracing::error!(
                "Core Raft node not available for Consul cluster mode, falling back to in-memory"
            );
            return self.build_inner_standalone(consul_naming_store, consul_registry);
        };

        // Create a table index for blocking query notifications
        let table_index = ConsulTableIndex::new();

        // Register the Consul plugin handler with the core Raft state machine
        let plugin_handler =
            crate::raft::plugin_handler::ConsulRaftPluginHandler::new_arc(table_index.clone());
        if let Err(e) = core_raft.register_plugin(plugin_handler).await {
            tracing::error!(
                "Failed to register Consul Raft plugin: {}, falling back to in-memory",
                e
            );
            return self.build_inner_standalone(consul_naming_store, consul_registry);
        }
        tracing::info!("Consul Raft plugin handler registered with core Raft");

        // Create a writer adapter that routes Consul requests through PluginWrite
        let consul_writer = Arc::new(ConsulRaftWriter::new(core_raft.clone()));

        // Get shared DB from core Raft state machine
        let db = core_raft.db();

        self.build_inner_cluster(
            consul_naming_store,
            consul_registry,
            db,
            consul_writer,
            table_index,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consul_plugin_trait_impl() {
        // Verify ProtocolAdapterPlugin is correctly implemented
        let plugin = create_test_plugin();
        assert_eq!(ProtocolAdapterPlugin::name(&plugin), "consul-compatibility");
        assert_eq!(plugin.protocol(), "consul");
        assert!(plugin.is_enabled());
        assert_eq!(plugin.priority(), 0);
    }

    #[tokio::test]
    async fn test_consul_plugin_lifecycle() {
        let plugin = create_test_plugin();
        assert_eq!(ProtocolAdapterPlugin::name(&plugin), "consul-compatibility");
        let ctx = batata_plugin::PluginContext::new();
        assert!(plugin.init(&ctx).await.is_ok());
        assert!(plugin.shutdown().await.is_ok());
        assert_eq!(plugin.default_port(), 8500);
    }

    #[test]
    fn test_consul_plugin_from_config() {
        let dc_config = ConsulDatacenterConfig::new("dc1".to_string());
        let plugin =
            ConsulPlugin::from_config(true, false, None, dc_config, false, "127.0.0.1".to_string(), 8500);
        assert!(plugin.is_enabled());
        assert_eq!(plugin.protocol(), "consul");
        assert_eq!(plugin.default_port(), 8500);
        // inner is not yet initialized
        assert!(plugin.inner.get().is_none());
    }

    #[tokio::test]
    async fn test_consul_plugin_from_config_init() {
        let dc_config = ConsulDatacenterConfig::new("dc1".to_string());
        let plugin =
            ConsulPlugin::from_config(true, false, None, dc_config, false, "127.0.0.1".to_string(), 8500);

        // Initialize with empty context (standalone mode)
        let ctx = batata_plugin::PluginContext::new();
        assert!(plugin.init(&ctx).await.is_ok());

        // inner is now initialized
        assert!(plugin.inner.get().is_some());

        // Services are accessible
        let inner = plugin.inner();
        assert_eq!(inner.dc_config.datacenter, "dc1");
    }

    #[tokio::test]
    async fn test_consul_plugin_double_init_legacy() {
        // Legacy constructor already initializes inner; init() should be a no-op
        let plugin = create_test_plugin();
        let ctx = batata_plugin::PluginContext::new();
        assert!(plugin.init(&ctx).await.is_ok());
        // Should not panic or error
    }

    fn create_test_plugin() -> ConsulPlugin {
        let naming = Arc::new(batata_naming::service::NamingService::new());
        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(naming));
        let dc_config = ConsulDatacenterConfig::new("dc1".to_string());
        ConsulPlugin::new(None, registry, false, dc_config)
    }
}
