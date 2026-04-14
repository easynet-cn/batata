//! ConsulPlugin - Protocol adapter plugin for Consul API compatibility.
//!
//! Implements the `Plugin` and `ProtocolAdapterPlugin` traits, encapsulating
//! all Consul services, routes, and app_data configuration.
//!
//! Supports two-phase initialization:
//! 1. Construction via `from_plugin_config()` — lightweight, no DB or Raft needed
//! 2. Initialization via `init(ctx)` — creates stores, registers Raft, builds services

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rocksdb::DB;

use batata_naming::InstanceCheckRegistry;
use batata_naming::healthcheck::{deregister_monitor::DeregisterMonitor, ttl_monitor::TtlMonitor};
use batata_plugin::{PluginStateProvider, ProtocolAdapterPlugin};

use crate::acl::AclService;
use crate::agent::ConsulAgentService;
use crate::catalog::ConsulCatalogService;
use crate::config_entry::ConsulConfigEntryService;
use crate::connect::ConsulConnectService;
use crate::connect_ca::ConsulConnectCAService;
use crate::constants::CF_CONSUL_CATALOG;
use crate::coordinate::ConsulCoordinateService;
use crate::event::ConsulEventService;
use crate::health::ConsulHealthService;
use crate::index_provider::{ConsulIndexProvider, ConsulTableIndex};
use crate::kv::ConsulKVService;
use crate::lock::{ConsulLockService, ConsulSemaphoreService};
use crate::model::{ConsulDatacenterConfig, ConsulPluginConfig};
use crate::naming_store::ConsulNamingStore;
use crate::operator::ConsulOperatorService;
use crate::peering::ConsulPeeringService;
use crate::query::ConsulQueryService;
use crate::raft::ConsulRaftWriter;
use crate::session::ConsulSessionService;
use crate::snapshot::ConsulSnapshotService;

/// Bridge from the Consul Raft plugin handler's apply-back hook to the
/// in-memory `ConsulNamingStore`. Keeps follower-side query state
/// consistent with committed Raft log.
struct NamingStoreApplyHook {
    store: Arc<ConsulNamingStore>,
}

impl NamingStoreApplyHook {
    fn new(store: Arc<ConsulNamingStore>) -> Self {
        Self { store }
    }
}

impl crate::raft::ConsulApplyHook for NamingStoreApplyHook {
    fn on_catalog_register(&self, key: &str, registration_json: &str) {
        use batata_plugin::PluginNamingStore;
        let data = bytes::Bytes::copy_from_slice(registration_json.as_bytes());
        if let Err(e) = self.store.register(key, data) {
            tracing::warn!(
                "NamingStoreApplyHook: register failed for key {}: {:?}",
                key,
                e
            );
        }
    }

    fn on_catalog_deregister(&self, key: &str) {
        use batata_plugin::PluginNamingStore;
        if let Err(e) = self.store.deregister(key) {
            tracing::warn!(
                "NamingStoreApplyHook: deregister failed for key {}: {:?}",
                key,
                e
            );
        }
    }

    fn on_acl_change(&self) {
        // Blunt but correct: clear every ACL cache so follower
        // authorization decisions reflect the committed policy change
        // within the apply latency rather than waiting up to 60 s for
        // the moka TTL to fire.
        crate::acl::invalidate_all_caches();
    }
}

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
    /// Raft writer for routing Consul writes through the unified Raft
    /// group. `None` in standalone mode; `Some` in cluster mode. Used by
    /// `ConsulResultHandler` auto-deregister and other leader-gated cascades.
    pub raft_writer: Option<Arc<ConsulRaftWriter>>,
    /// Cluster manager for real cluster mode. `None` in standalone mode.
    /// Used to provide cluster-aware responses for agent/members, status/leader,
    /// operator/raft, and internal/ui/nodes endpoints.
    pub cluster_manager: Option<Arc<dyn batata_common::ClusterManager>>,
}

/// Consul compatibility protocol adapter plugin.
///
/// Supports two-phase initialization:
/// - Phase 1: `from_plugin_config()` creates a lightweight plugin with only config
/// - Phase 2: `init(ctx)` reads Raft/cluster info from context, builds all services
///
/// Legacy constructors (`new()`, `with_consul_raft()`) are preserved for backward
/// compatibility but will be removed once `consul_init.rs` is deleted.
pub struct ConsulPlugin {
    /// Plugin configuration (all consul-specific settings).
    config: ConsulPluginConfig,
    /// Datacenter configuration (convenience alias for config.dc_config).
    pub dc_config: ConsulDatacenterConfig,
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
    pub fn from_plugin_config(config: ConsulPluginConfig) -> Self {
        let dc_config = config.dc_config.clone();
        Self {
            config,
            dc_config,
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
            acl: if self.config.acl_enabled {
                match &self.config.initial_management_token {
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
            raft_writer: None,
            cluster_manager: None,
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

        // Restore persisted health check configs from RocksDB
        if let Some(cf) = db.cf_handle(crate::constants::CF_CONSUL_HEALTH_CHECKS) {
            let mut restored = 0u64;
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            for item in iter.flatten() {
                let (_key_bytes, value_bytes) = item;
                match serde_json::from_slice::<
                    batata_naming::healthcheck::registry::InstanceCheckConfig,
                >(&value_bytes)
                {
                    Ok(config) => {
                        registry.register_check(config);
                        restored += 1;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to restore health check config: {}", e);
                    }
                }
            }
            if restored > 0 {
                tracing::info!("Restored {} health check configs from RocksDB", restored);
            }
        }

        ConsulPluginInner {
            naming_store: naming_store.clone(),
            agent: ConsulAgentService::with_raft(
                naming_store.clone(),
                registry.clone(),
                check_index.clone(),
                consul_raft.clone(),
            ),
            health: ConsulHealthService::new(registry.clone(), check_index)
                .with_node_name(self.dc_config.node_name.clone()),
            kv,
            catalog: ConsulCatalogService::with_dc_config(naming_store.clone(), &self.dc_config)
                .with_index_provider(index_provider.clone()),
            acl: if self.config.acl_enabled {
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
                consul_raft.clone(),
                index_provider.clone(),
            ),
            dc_config: self.dc_config.clone(),
            index_provider,
            registry,
            raft_writer: Some(consul_raft),
            cluster_manager: None, // Set later via set_cluster_manager()
        }
    }

    // ========================================================================
    // Legacy constructors (backward compatibility with consul_init.rs)
    // ========================================================================

    /// Creates Consul plugin with in-memory storage.
    ///
    /// **Deprecated**: Use `from_plugin_config()` + `init()` instead. Kept for backward
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

        let config = ConsulPluginConfig {
            enabled: true,
            acl_enabled,
            initial_management_token: None,
            register_self: false,
            dc_config: dc_config.clone(),
            ..ConsulPluginConfig::default()
        };
        let plugin = Self {
            config,
            dc_config,
            inner: std::sync::OnceLock::new(),
        };

        let inner = plugin.build_inner_standalone(naming_store, registry);
        let _ = plugin.inner.set(inner);
        plugin
    }

    /// Creates Consul plugin with Raft-replicated RocksDB storage (cluster mode).
    ///
    /// **Deprecated**: Use `from_plugin_config()` + `init()` instead. Kept for backward
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

        let config = ConsulPluginConfig {
            enabled: true,
            acl_enabled,
            initial_management_token: None,
            register_self: false,
            dc_config: dc_config.clone(),
            ..ConsulPluginConfig::default()
        };
        let plugin = Self {
            config,
            dc_config,
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
        self.config.enabled
    }

    fn default_port(&self) -> u16 {
        self.dc_config.consul_port
    }

    fn http_workers(&self) -> usize {
        self.config.http_workers
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
        // ClusterManager is stored as Arc<Arc<dyn ClusterManager>> in the context;
        // unwrap the outer Arc to get the inner Arc<dyn ClusterManager>.
        let cluster_manager: Option<Arc<dyn batata_common::ClusterManager>> = ctx
            .get::<Arc<dyn batata_common::ClusterManager>>("cluster_manager")
            .map(|arc_arc| (*arc_arc).clone());

        // Create Consul naming store and result handler
        let consul_naming_store = Arc::new(crate::naming_store::ConsulNamingStore::new());
        let consul_index_provider = Arc::new(ConsulIndexProvider::new());
        let consul_result_handler = Arc::new(crate::result_handler::ConsulResultHandler::new(
            consul_naming_store.clone(),
            consul_index_provider.clone(),
        ));
        let consul_registry = Arc::new(InstanceCheckRegistry::new(
            consul_result_handler.clone() as Arc<dyn batata_plugin::HealthCheckResultHandler>
        ));

        let mut inner = if is_cluster {
            self.init_cluster(raft_node, consul_naming_store, consul_registry)
                .await
        } else {
            tracing::info!("Consul services using in-memory storage (standalone/console mode)");
            self.build_inner_standalone(consul_naming_store, consul_registry)
        };

        // Set cluster manager for cluster-aware Consul endpoints
        if let Some(cm) = cluster_manager {
            tracing::info!("Consul plugin using real ClusterManager for cluster-aware endpoints");
            inner.cluster_manager = Some(cm);
        }

        // Wire ConsulResultHandler to session/kv/check_index for:
        // - on_deregister: check_id → service_id → store_key chain
        // - on_check_critical: session invalidation + KV lock release
        consul_result_handler.set_services(
            inner.health.check_index(),
            Arc::new(inner.session.clone()),
            Arc::new(inner.kv.clone()),
            inner.raft_writer.clone(),
        );

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
            .app_data(actix_web::web::Data::new(inner.index_provider.clone()));

        // In cluster mode, inject ClusterManager and use real cluster routes
        if let Some(ref cm) = inner.cluster_manager {
            cfg.app_data(actix_web::web::Data::new(cm.clone()))
                .service(crate::route::routes_real());
        } else {
            cfg.service(crate::route::routes());
        }
    }

    async fn start_background_tasks(&self) -> anyhow::Result<()> {
        let inner = self.inner();

        // Auto-register Consul service if enabled
        if self.config.register_self {
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

        // Deregister monitor (5s interval for responsive service cleanup)
        tracing::info!("Starting Consul deregister monitor...");
        let deregister_monitor = DeregisterMonitor::new(inner.registry.clone(), 5);
        tokio::spawn(async move {
            deregister_monitor.start().await;
        });

        // Vivaldi coordinate refresh task (cluster mode only).
        //
        // Every 60 seconds, ping each cluster member over TCP (to consul_port)
        // to measure RTT, then feed the RTT into the Vivaldi algorithm.
        // This populates real coordinates over time so `/v1/coordinate/nodes`
        // returns latency-aware data.
        if let Some(ref cm) = inner.cluster_manager {
            tracing::info!("Starting Consul Vivaldi coordinate task...");
            let coord_svc = Arc::new(inner.coordinate.clone());
            let cm_clone = cm.clone();
            let dc_config = self.dc_config.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                interval.tick().await; // skip initial firing
                let self_node = dc_config.node_name.clone();
                let port = dc_config.consul_port;
                loop {
                    interval.tick().await;
                    let members = cm_clone.all_members_extended();
                    for member in &members {
                        if cm_clone.is_self(&member.address) {
                            continue;
                        }
                        let addr = format!("{}:{}", member.ip, port);
                        // Measure TCP connect RTT (reasonable proxy for path latency)
                        let start = std::time::Instant::now();
                        let res = tokio::time::timeout(
                            Duration::from_secs(2),
                            tokio::net::TcpStream::connect(&addr),
                        )
                        .await;
                        let rtt = start.elapsed();
                        if matches!(res, Ok(Ok(_))) {
                            // Use remote node's last known coord (default for new peers)
                            let remote_node = format!(
                                "node-{}",
                                member.ip.replace('.', "-")
                            );
                            let remote_coord = coord_svc
                                .coordinates_arc()
                                .get(&format!("{}:", remote_node))
                                .map(|r| r.value().coord.clone())
                                .unwrap_or_default();
                            coord_svc.apply_rtt_measurement(
                                &self_node,
                                &remote_node,
                                &remote_coord,
                                rtt,
                            );
                        }
                    }
                }
            });
        }

        // Active health check execution (HTTP/TCP/gRPC/MySQL).
        // Creates a reactor for the Consul registry. When checks are registered
        // with HTTP/TCP/gRPC type, the reactor periodically executes them.
        {
            let naming_service = Arc::new(batata_naming::NamingService::new());
            let config = Arc::new(batata_naming::healthcheck::HealthCheckConfig::default());
            let reactor =
                batata_naming::healthcheck::HealthCheckReactor::new(naming_service, config);
            reactor.set_registry(inner.registry.clone());

            // Wire the check scheduler so new active check registrations are auto-scheduled
            let reactor_ref = Arc::new(reactor);
            let reactor_for_scheduler = reactor_ref.clone();
            inner.health.set_check_scheduler(Arc::new(move |check_key| {
                reactor_for_scheduler.schedule_registry_check(check_key);
            }));

            // Schedule any active checks that were registered before this point
            for (config, _status) in inner.registry.get_all_checks() {
                if config.check_type.is_active() {
                    reactor_ref.schedule_registry_check(&config.check_id);
                }
            }

            tracing::info!("Consul active health check reactor started");
        }

        // Session TTL cleanup — leader-only, routes destroys through Raft.
        //
        // Before this fix every node independently swept its own RocksDB,
        // producing divergent state in a multi-node cluster (leader and
        // followers could disagree about which sessions had expired).
        // Now only the leader decides, and the decisions replicate via
        // `SessionDestroy` Raft entries so all nodes apply the same
        // cleanup in the same order. The sweep still runs on every node
        // (cheap) so that leader changes don't stall cleanup.
        let session_svc = inner.session.clone();
        let kv_svc = inner.kv.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                // `cleanup_expired_via_raft` internally checks leader
                // status and routes each destroy through Raft. It's a
                // no-op on followers.
                let _ = session_svc.cleanup_expired_via_raft(&kv_svc).await;
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

        // Create the plugin handler and capture its apply-hook slot before
        // the handler is moved into the Raft state machine. The slot is a
        // shared `Arc<RwLock<_>>`, so installing the concrete hook after
        // registration is still visible to the handler inside Raft.
        let plugin_handler =
            crate::raft::plugin_handler::ConsulRaftPluginHandler::new_arc(table_index.clone());
        let apply_hook_slot = plugin_handler.apply_hook();

        if let Err(e) = core_raft.register_plugin(plugin_handler).await {
            tracing::error!(
                "Failed to register Consul Raft plugin: {}, falling back to in-memory",
                e
            );
            return self.build_inner_standalone(consul_naming_store, consul_registry);
        }
        tracing::info!("Consul Raft plugin handler registered with core Raft");

        // Install the apply-back hook: when the state machine applies a
        // `CatalogRegister`/`CatalogDeregister` log entry, update the
        // in-memory `ConsulNamingStore` on ALL nodes (leader + followers).
        // Without this, followers only see new registrations in RocksDB,
        // and `ConsulNamingStore::get_service_entries` — the actual query
        // path — returns stale/empty results.
        {
            let hook: Arc<dyn crate::raft::ConsulApplyHook> =
                Arc::new(NamingStoreApplyHook::new(consul_naming_store.clone()));
            *apply_hook_slot.write().await = Some(hook);
            tracing::info!("Consul apply-back hook installed (NamingStoreApplyHook)");
        }

        // Cold-start recovery: after the hook is installed, rehydrate the
        // in-memory naming store from committed catalog data in RocksDB.
        // New applies after this point flow through the hook naturally,
        // so we only need to replay what was already committed.
        {
            use batata_plugin::PluginNamingStore;
            let db = core_raft.db();
            if let Some(cf) = db.cf_handle(CF_CONSUL_CATALOG) {
                let mut count = 0usize;
                let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
                for item in iter {
                    let Ok((k, v)) = item else { continue };
                    let Ok(key) = std::str::from_utf8(&k) else {
                        continue;
                    };
                    let data = bytes::Bytes::copy_from_slice(&v);
                    if consul_naming_store.register(key, data).is_ok() {
                        count += 1;
                    }
                }
                tracing::info!(
                    "Consul catalog cold-start replay complete: {} entries restored",
                    count
                );
            }
        }

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

impl PluginStateProvider for ConsulPlugin {
    fn plugin_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(4);
        state.insert(
            "consul_enabled".to_string(),
            Some(format!("{}", self.config.enabled)),
        );
        state.insert(
            "consul_port".to_string(),
            Some(format!("{}", self.dc_config.consul_port)),
        );
        state.insert(
            "consul_version".to_string(),
            Some(self.dc_config.consul_version.clone()),
        );
        state.insert(
            "consul_acl_enabled".to_string(),
            Some(format!("{}", self.config.acl_enabled)),
        );
        state
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
    fn test_consul_plugin_from_plugin_config() {
        let config = ConsulPluginConfig {
            enabled: true,
            dc_config: ConsulDatacenterConfig::new("dc1".to_string()),
            ..ConsulPluginConfig::default()
        };
        let plugin = ConsulPlugin::from_plugin_config(config);
        assert!(plugin.is_enabled());
        assert_eq!(plugin.protocol(), "consul");
        assert_eq!(plugin.default_port(), 8500);
        // inner is not yet initialized
        assert!(plugin.inner.get().is_none());
    }

    #[tokio::test]
    async fn test_consul_plugin_from_config_init() {
        let config = ConsulPluginConfig {
            enabled: true,
            dc_config: ConsulDatacenterConfig::new("dc1".to_string()),
            ..ConsulPluginConfig::default()
        };
        let plugin = ConsulPlugin::from_plugin_config(config);

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

    #[test]
    fn test_consul_plugin_state_provider() {
        let plugin = create_test_plugin();
        let state = plugin.plugin_state();
        assert_eq!(state.get("consul_enabled"), Some(&Some("true".to_string())));
        assert_eq!(state.get("consul_port"), Some(&Some("8500".to_string())));
        assert!(state.contains_key("consul_version"));
        assert_eq!(
            state.get("consul_acl_enabled"),
            Some(&Some("false".to_string()))
        );
        assert_eq!(state.len(), 4);
    }

    #[test]
    fn test_consul_plugin_state_provider_disabled() {
        let config = ConsulPluginConfig {
            enabled: false,
            dc_config: ConsulDatacenterConfig::new("dc1".to_string()),
            ..ConsulPluginConfig::default()
        };
        let plugin = ConsulPlugin::from_plugin_config(config);
        let state = plugin.plugin_state();
        assert_eq!(
            state.get("consul_enabled"),
            Some(&Some("false".to_string()))
        );
    }

    fn create_test_plugin() -> ConsulPlugin {
        let naming = Arc::new(batata_naming::service::NamingService::new());
        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(naming));
        let dc_config = ConsulDatacenterConfig::new("dc1".to_string());
        ConsulPlugin::new(None, registry, false, dc_config)
    }
}
