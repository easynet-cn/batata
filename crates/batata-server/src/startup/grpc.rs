//! gRPC server setup and handler registration module.

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tonic::service::InterceptorLayer;
use tower::ServiceBuilder;
use tracing::info;

use batata_api::model::Member;
use batata_consistency::RaftNode;
use batata_core::{
    GrpcAuthRoleProvider, GrpcAuthService, GrpcPermissionInfo, GrpcRoleInfo,
    service::{
        cluster_client::{ClusterClientConfig, ClusterClientManager},
        distro::{DistroConfig, DistroProtocol},
        remote::{ConnectionManager, context_interceptor},
    },
};
use batata_naming::handler::distro::NamingInstanceDistroHandler;
use batata_persistence::PersistenceService;

use crate::model::tls::validate_tls_config;
use crate::startup::AIServices;

use crate::{
    api::grpc::{bi_request_stream_server::BiRequestStreamServer, request_server::RequestServer},
    model::common::AppState,
    service::{
        ai_handler::{
            AgentEndpointHandler, McpServerEndpointHandler, QueryAgentCardHandler,
            QueryMcpServerHandler, QueryPromptHandler, ReleaseAgentCardHandler,
            ReleaseMcpServerHandler,
        },
        cluster_handler::MemberReportHandler,
        config_fuzzy_watch::ConfigFuzzyWatchManager,
        config_handler::{
            ClientConfigMetricHandler, ConfigBatchListenHandler, ConfigChangeClusterSyncHandler,
            ConfigChangeNotifyHandler, ConfigFuzzyWatchChangeNotifyHandler,
            ConfigFuzzyWatchHandler, ConfigFuzzyWatchSyncHandler, ConfigPublishHandler,
            ConfigQueryHandler, ConfigRemoveHandler,
        },
        distro_handler::{
            DistroDataBatchSyncHandler, DistroDataSnapshotHandler, DistroDataSyncHandler,
            DistroDataVerifyHandler,
        },
        handler::{
            ClientDetectionHandler, ConnectResetHandler, ConnectionSetupHandler,
            HealthCheckHandler, PushAckHandler, ServerCheckHandler, ServerLoaderInfoHandler,
            ServerReloadHandler, SetupAckHandler,
        },
        lock::LockService,
        lock_handler::LockOperationHandler,
        naming::NamingService,
        naming_fuzzy_watch::NamingFuzzyWatchManager,
        naming_handler::{
            BatchInstanceRequestHandler, InstanceRequestHandler,
            NamingFuzzyWatchChangeNotifyHandler, NamingFuzzyWatchHandler,
            NamingFuzzyWatchSyncHandler, NotifySubscriberHandler, PersistentInstanceRequestHandler,
            ServiceListRequestHandler, ServiceQueryRequestHandler, SubscribeServiceRequestHandler,
        },
        rpc::{GrpcBiRequestStreamService, GrpcRequestService, HandlerRegistry},
    },
};

/// gRPC server configuration and state.
pub struct GrpcServers {
    /// Handle for the SDK gRPC server task.
    _sdk_server: tokio::task::JoinHandle<()>,
    /// Handle for the cluster gRPC server task.
    _cluster_server: tokio::task::JoinHandle<()>,
    /// Handle for the Raft gRPC server task (only in distributed embedded mode).
    _raft_server: Option<tokio::task::JoinHandle<()>>,
    /// Shutdown signal sender — dropping or sending signals graceful stop.
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    /// Per-server state trackers for health aggregation.
    pub sdk_state: batata_core::ServerStateTracker,
    pub cluster_state: batata_core::ServerStateTracker,
    pub raft_state: batata_core::ServerStateTracker,
    /// The naming service used by handlers.
    naming_service: Arc<NamingService>,
    /// The connection manager for tracking client connections.
    connection_manager: Arc<ConnectionManager>,
    /// The distro protocol for cluster data synchronization.
    distro_protocol: Arc<DistroProtocol>,
    /// The cluster client manager for inter-node communication.
    cluster_client_manager: Option<Arc<ClusterClientManager>>,
    /// Config change notifier for long-polling HTTP listeners.
    config_change_notifier: Arc<batata_config::ConfigChangeNotifier>,
    /// Handler registry for dynamic handler registration (e.g., plugin handlers).
    handler_registry: Arc<HandlerRegistry>,
}

impl GrpcServers {
    /// Signal all gRPC servers to stop accepting new connections and shut down gracefully.
    pub fn shutdown(&self) {
        self.sdk_state.set_draining();
        self.cluster_state.set_draining();
        self.raft_state.set_draining();
        let _ = self.shutdown_tx.send(true);
        self.sdk_state.set_stopped();
        self.cluster_state.set_stopped();
        self.raft_state.set_stopped();
    }

    /// Get naming service as trait object for HTTP consumers.
    pub fn naming_provider(&self) -> Arc<dyn batata_api::naming::NamingServiceProvider> {
        self.naming_service.clone()
    }

    /// Get concrete naming service (for internal components that need it).
    pub fn naming_service(&self) -> &Arc<NamingService> {
        &self.naming_service
    }

    /// Get connection manager reference.
    pub fn connection_manager(&self) -> &Arc<ConnectionManager> {
        &self.connection_manager
    }

    /// Get distro protocol reference.
    pub fn distro_protocol(&self) -> &Arc<DistroProtocol> {
        &self.distro_protocol
    }

    /// Get cluster client manager reference.
    pub fn cluster_client_manager(&self) -> &Option<Arc<ClusterClientManager>> {
        &self.cluster_client_manager
    }

    /// Get config change notifier reference.
    pub fn config_change_notifier(&self) -> &Arc<batata_config::ConfigChangeNotifier> {
        &self.config_change_notifier
    }

    /// Get handler registry for dynamic handler registration (e.g., plugin handlers).
    pub fn handler_registry(&self) -> &Arc<HandlerRegistry> {
        &self.handler_registry
    }
}

/// Registers all internal handlers (health check, connection setup, etc.).
fn register_internal_handlers(
    registry: &mut HandlerRegistry,
    connection_manager: Arc<ConnectionManager>,
) {
    registry.register_handler(Arc::new(HealthCheckHandler {}));
    registry.register_handler(Arc::new(ServerCheckHandler {}));
    registry.register_handler(Arc::new(ConnectionSetupHandler {}));
    registry.register_handler(Arc::new(ClientDetectionHandler {}));
    registry.register_handler(Arc::new(ServerLoaderInfoHandler {
        connection_manager: connection_manager as Arc<dyn batata_core::ClientConnectionManager>,
    }));
    // ServerReloadHandler with config path
    let config_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("conf/application.yml")
        .to_string_lossy()
        .to_string();
    registry.register_handler(Arc::new(ServerReloadHandler { config_path }));
    registry.register_handler(Arc::new(ConnectResetHandler {}));
    registry.register_handler(Arc::new(SetupAckHandler {}));
    registry.register_handler(Arc::new(PushAckHandler {}));
}

/// Registers all config handlers.
fn register_config_handlers(
    registry: &mut HandlerRegistry,
    app_state: Arc<AppState>,
    fuzzy_watch_manager: Arc<ConfigFuzzyWatchManager>,
    connection_manager: Arc<ConnectionManager>,
    cluster_client_manager: Option<Arc<ClusterClientManager>>,
    config_change_notifier: Arc<batata_config::ConfigChangeNotifier>,
) {
    let connection_manager: Arc<dyn batata_core::ClientConnectionManager> = connection_manager;
    registry.register_handler(Arc::new(ConfigQueryHandler {
        app_state: app_state.clone(),
    }));
    registry.register_handler(Arc::new(ConfigPublishHandler {
        app_state: app_state.clone(),
        fuzzy_watch_manager: fuzzy_watch_manager.clone(),
        connection_manager: connection_manager.clone(),
        cluster_client_manager: cluster_client_manager.clone(),
        config_change_notifier: config_change_notifier.clone(),
    }));
    registry.register_handler(Arc::new(ConfigRemoveHandler {
        app_state: app_state.clone(),
        fuzzy_watch_manager: fuzzy_watch_manager.clone(),
        connection_manager: connection_manager.clone(),
        cluster_client_manager,
        config_change_notifier: config_change_notifier.clone(),
    }));
    registry.register_handler(Arc::new(ConfigBatchListenHandler {
        app_state: app_state.clone(),
    }));
    registry.register_handler(Arc::new(ConfigChangeNotifyHandler {
        app_state: app_state.clone(),
    }));
    registry.register_handler(Arc::new(ConfigChangeClusterSyncHandler {
        app_state: app_state.clone(),
        fuzzy_watch_manager: fuzzy_watch_manager.clone(),
        connection_manager: connection_manager.clone(),
        config_change_notifier: config_change_notifier.clone(),
    }));
    registry.register_handler(Arc::new(ConfigFuzzyWatchHandler {
        app_state: app_state.clone(),
        fuzzy_watch_manager: fuzzy_watch_manager.clone(),
        connection_manager: connection_manager.clone(),
    }));
    registry.register_handler(Arc::new(ConfigFuzzyWatchChangeNotifyHandler {
        app_state: app_state.clone(),
    }));
    registry.register_handler(Arc::new(ConfigFuzzyWatchSyncHandler {
        app_state: app_state.clone(),
        fuzzy_watch_manager: fuzzy_watch_manager.clone(),
    }));
    registry.register_handler(Arc::new(ClientConfigMetricHandler {
        app_state,
        fuzzy_watch_manager,
    }));
}

/// Registers all naming handlers.
fn register_naming_handlers(
    registry: &mut HandlerRegistry,
    naming_service: Arc<NamingService>,
    naming_fuzzy_watch_manager: Arc<NamingFuzzyWatchManager>,
    connection_manager: Arc<ConnectionManager>,
    distro_protocol: Option<Arc<DistroProtocol>>,
    raft_node: Option<Arc<RaftNode>>,
    client_op_proxy: Arc<batata_naming::service::ClientOperationServiceProxy>,
) {
    let naming_service: Arc<dyn batata_api::naming::NamingServiceProvider> = naming_service;
    registry.register_handler(Arc::new(InstanceRequestHandler {
        naming_service: naming_service.clone(),
        naming_fuzzy_watch_manager: naming_fuzzy_watch_manager.clone(),
        connection_manager: connection_manager.clone(),
        distro_protocol: distro_protocol.clone(),
        client_op_proxy: Some(client_op_proxy.clone()),
    }));
    registry.register_handler(Arc::new(BatchInstanceRequestHandler {
        naming_service: naming_service.clone(),
        connection_manager: connection_manager.clone(),
        distro_protocol,
    }));
    registry.register_handler(Arc::new(ServiceListRequestHandler {
        naming_service: naming_service.clone(),
    }));
    registry.register_handler(Arc::new(ServiceQueryRequestHandler {
        naming_service: naming_service.clone(),
    }));
    registry.register_handler(Arc::new(SubscribeServiceRequestHandler {
        naming_service: naming_service.clone(),
    }));
    registry.register_handler(Arc::new(PersistentInstanceRequestHandler {
        naming_service: naming_service.clone(),
        connection_manager: connection_manager.clone(),
        raft_node,
    }));
    registry.register_handler(Arc::new(NotifySubscriberHandler {
        naming_service: naming_service.clone(),
    }));
    registry.register_handler(Arc::new(NamingFuzzyWatchHandler {
        naming_service: naming_service.clone(),
        naming_fuzzy_watch_manager: naming_fuzzy_watch_manager.clone(),
        connection_manager: connection_manager.clone(),
    }));
    registry.register_handler(Arc::new(NamingFuzzyWatchChangeNotifyHandler {
        naming_service: naming_service.clone(),
        naming_fuzzy_watch_manager: naming_fuzzy_watch_manager.clone(),
    }));
    registry.register_handler(Arc::new(NamingFuzzyWatchSyncHandler {
        naming_service,
        naming_fuzzy_watch_manager,
        connection_manager,
    }));
}

/// Registers all distro protocol handlers.
fn register_distro_handlers(registry: &mut HandlerRegistry, distro_protocol: Arc<DistroProtocol>) {
    registry.register_handler(Arc::new(DistroDataSyncHandler {
        distro_protocol: distro_protocol.clone(),
    }));
    registry.register_handler(Arc::new(DistroDataVerifyHandler {
        distro_protocol: distro_protocol.clone(),
    }));
    registry.register_handler(Arc::new(DistroDataBatchSyncHandler {
        distro_protocol: distro_protocol.clone(),
    }));
    registry.register_handler(Arc::new(DistroDataSnapshotHandler { distro_protocol }));
}

/// Registers the cluster member report handler.
fn register_cluster_handlers(
    registry: &mut HandlerRegistry,
    server_member_manager: &Option<Arc<batata_core::cluster::ServerMemberManager>>,
) {
    if let Some(member_manager) = server_member_manager {
        registry.register_handler(Arc::new(MemberReportHandler {
            member_manager: member_manager.clone(),
        }));
    }

    // Auth cache invalidation handler — receives cache eviction requests from peers
    registry.register_handler(Arc::new(
        batata_core::handler::auth_cache::AuthCacheInvalidateHandler {
            invalidator: Arc::new(AuthCacheInvalidatorImpl),
        },
    ));
}

/// Concrete implementation of AuthCacheInvalidator that calls batata_auth functions.
/// Lives in batata-server because this is the only crate that depends on both
/// batata-core (trait definition) and batata-auth (cache functions).
struct AuthCacheInvalidatorImpl;

impl batata_core::handler::auth_cache::AuthCacheInvalidator for AuthCacheInvalidatorImpl {
    fn invalidate(&self, invalidate_type: &str, target: &str) {
        match invalidate_type {
            "role" => {
                batata_auth::service::role::invalidate_roles_cache(target);
                batata_auth::service::permission::invalidate_permissions_cache_for_role(target);
            }
            "permission" => {
                batata_auth::service::permission::invalidate_permissions_cache_for_role(target);
            }
            "token" => {
                batata_auth::service::auth::invalidate_token(target);
            }
            "user" => {
                batata_auth::service::role::invalidate_roles_cache(target);
                batata_core::service::grpc_auth::GrpcAuthService::invalidate_cache_for_user(target);
            }
            "all" => {
                batata_auth::service::auth::clear_token_cache();
                batata_auth::service::role::invalidate_all_roles_cache();
                batata_auth::service::permission::invalidate_all_permissions_cache();
                batata_core::service::grpc_auth::GrpcAuthService::clear_cache();
            }
            other => {
                tracing::warn!("Unknown auth cache invalidation type: {}", other);
            }
        }
    }
}

/// Registers the lock operation handler.
///
/// When `raft_node` is supplied, lock writes are routed through Raft
/// consensus (CP lock, the mode Nacos SDKs expect for compatibility). In standalone mode
/// `raft_node` is `None` and the handler falls back to the in-memory
/// `LockService` — locks are then single-node and lost on restart.
fn register_lock_handlers(
    registry: &mut HandlerRegistry,
    lock_service: Arc<LockService>,
    auth_service: Arc<GrpcAuthService>,
    raft_node: Option<Arc<RaftNode>>,
) {
    registry.register_handler(Arc::new(LockOperationHandler {
        lock_service,
        auth_service,
        raft_node,
        fence_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
    }));
}

/// Registers all AI handlers (MCP + A2A).
fn register_ai_handlers(registry: &mut HandlerRegistry, ai_services: &AIServices) {
    registry.register_handler(Arc::new(McpServerEndpointHandler {
        mcp_registry: ai_services.mcp_registry.clone(),
        mcp_service: ai_services.mcp_service.clone(),
        endpoint_service: ai_services.endpoint_service.clone(),
    }));
    registry.register_handler(Arc::new(QueryMcpServerHandler {
        mcp_registry: ai_services.mcp_registry.clone(),
        mcp_service: ai_services.mcp_service.clone(),
    }));
    registry.register_handler(Arc::new(ReleaseMcpServerHandler {
        mcp_registry: ai_services.mcp_registry.clone(),
        mcp_service: ai_services.mcp_service.clone(),
    }));
    registry.register_handler(Arc::new(AgentEndpointHandler {
        agent_registry: ai_services.agent_registry.clone(),
        a2a_service: ai_services.a2a_service.clone(),
        endpoint_service: ai_services.endpoint_service.clone(),
    }));
    registry.register_handler(Arc::new(QueryAgentCardHandler {
        agent_registry: ai_services.agent_registry.clone(),
        a2a_service: ai_services.a2a_service.clone(),
    }));
    registry.register_handler(Arc::new(ReleaseAgentCardHandler {
        agent_registry: ai_services.agent_registry.clone(),
        a2a_service: ai_services.a2a_service.clone(),
    }));

    // Prompt handler
    if let Some(ref prompt_service) = ai_services.prompt_service {
        registry.register_handler(Arc::new(QueryPromptHandler {
            prompt_service: prompt_service.clone(),
        }));
    }
}

/// Creates and initializes the Distro protocol with the naming service handler.
fn create_distro_protocol(
    local_address: &str,
    naming_service: Arc<batata_naming::NamingService>,
    members: Arc<DashMap<String, Member>>,
    cluster_client_manager: Arc<ClusterClientManager>,
    distro_config: DistroConfig,
) -> Arc<DistroProtocol> {
    let distro_protocol = DistroProtocol::new(
        local_address.to_string(),
        distro_config,
        cluster_client_manager,
        members,
    );

    // Create and register naming instance handler
    let naming_handler =
        NamingInstanceDistroHandler::new(local_address.to_string(), naming_service);
    distro_protocol.register_handler(Arc::new(naming_handler));

    Arc::new(distro_protocol)
}

/// Adapter that implements GrpcAuthRoleProvider using PersistenceService.
/// Bridges the gRPC auth layer with the database persistence layer.
struct PersistenceRoleProvider {
    persistence: Arc<dyn PersistenceService>,
}

#[async_trait::async_trait]
impl GrpcAuthRoleProvider for PersistenceRoleProvider {
    async fn find_roles_by_username(&self, username: &str) -> Vec<GrpcRoleInfo> {
        self.persistence
            .role_find_by_username(username)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| GrpcRoleInfo {
                username: r.username,
                role: r.role,
            })
            .collect()
    }

    async fn find_permissions_by_roles(&self, roles: &[String]) -> Vec<GrpcPermissionInfo> {
        self.persistence
            .permission_find_by_roles(roles.to_vec())
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| GrpcPermissionInfo {
                role: p.role,
                resource: p.resource,
                action: p.action,
            })
            .collect()
    }
}

/// Creates and starts gRPC servers for SDK and cluster communication.
///
/// # Arguments
/// * `app_state` - Application state containing configuration and services
/// * `sdk_server_port` - Port for SDK gRPC server
/// * `cluster_server_port` - Port for cluster gRPC server
///
/// # Returns
/// A `GrpcServers` struct containing the spawned server handles and naming service.
#[allow(clippy::too_many_arguments)]
pub fn start_grpc_servers(
    app_state: Arc<AppState>,
    naming_service: Option<Arc<NamingService>>,
    ai_services: &AIServices,
    sdk_server_port: u16,
    cluster_server_port: u16,
    raft_node: Option<Arc<RaftNode>>,
    server_member_manager: Option<Arc<batata_core::cluster::ServerMemberManager>>,
    config_subscriber_manager: Arc<dyn batata_common::ConfigSubscriptionService>,
) -> Result<GrpcServers, Box<dyn std::error::Error>> {
    // Get TLS configuration
    let tls_config = app_state.configuration.grpc_tls_config();

    // Validate TLS configuration if enabled
    if tls_config.sdk_enabled || tls_config.cluster_enabled {
        let validation = validate_tls_config(&tls_config);
        if !validation.valid {
            for error in &validation.errors {
                tracing::error!("TLS configuration error: {}", error);
            }
            return Err(format!(
                "TLS configuration validation failed: {:?}",
                validation.errors
            )
            .into());
        }
        for warning in &validation.warnings {
            tracing::warn!("TLS configuration warning: {}", warning);
        }
    }

    // Shutdown signal: send `true` to stop all gRPC servers gracefully
    let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);

    // Setup gRPC interceptor layer
    let layer = ServiceBuilder::new()
        .load_shed()
        .layer(InterceptorLayer::new(context_interceptor))
        .into_inner();

    // Create connection manager first (needed by handlers and stream service)
    let mut cm = ConnectionManager::new();
    cm.set_push_timeout(std::time::Duration::from_millis(
        app_state.configuration.grpc_push_message_timeout_ms(),
    ));
    cm.set_max_push_timeouts(app_state.configuration.grpc_max_push_timeouts());
    cm.set_max_connections(app_state.configuration.grpc_max_connections());
    let connection_manager = Arc::new(cm);
    // Wire connection limit checker if control plugin is available
    if let Some(ref control_plugin) = app_state.control_plugin {
        let limiter = Arc::new(
            crate::service::connection_limit::ControlPluginConnectionLimiter::new(
                control_plugin.clone(),
            ),
        );
        connection_manager.set_connection_limit_checker(limiter);
        info!("gRPC connection limiting enabled");
    }
    // Start background health checker to eject stale connections
    connection_manager.start_health_checker(app_state.configuration.grpc_connection_stale_ms());
    // Keep a clone for the HTTP server
    let connection_manager_for_http = connection_manager.clone();

    // Create gRPC auth service based on configuration
    let auth_enabled = app_state.configuration.auth_enabled();
    let token_secret_key = app_state.configuration.token_secret_key();
    let server_identity_key = app_state.configuration.server_identity_key();
    let server_identity_value = app_state.configuration.server_identity_value();

    let grpc_auth_service = if let Some(persistence) = app_state.persistence.clone() {
        let role_provider = Arc::new(PersistenceRoleProvider { persistence });
        GrpcAuthService::with_role_provider(
            auth_enabled,
            token_secret_key,
            server_identity_key,
            server_identity_value,
            role_provider,
        )
    } else {
        GrpcAuthService::new(
            auth_enabled,
            token_secret_key,
            server_identity_key,
            server_identity_value,
        )
    };
    let grpc_auth_service_arc = Arc::new(grpc_auth_service.clone());

    if auth_enabled {
        info!("gRPC authentication is enabled");
    }

    // Initialize gRPC handlers with auth service
    let mut handler_registry = HandlerRegistry::with_auth(grpc_auth_service);

    // Wire TPS control into gRPC handler dispatch
    if let Some(ref control_plugin) = app_state.control_plugin {
        let tps_checker = Arc::new(crate::service::tps_checker::ControlPluginTpsChecker::new(
            control_plugin.clone(),
        ));
        handler_registry.set_tps_checker(tps_checker);
        info!("gRPC TPS control enabled");
    }

    // Create config fuzzy watch manager
    let config_fuzzy_watch_manager = Arc::new(ConfigFuzzyWatchManager::new());

    // Create naming fuzzy watch manager
    let naming_fuzzy_watch_manager = Arc::new(NamingFuzzyWatchManager::new());

    register_internal_handlers(&mut handler_registry, connection_manager.clone());
    // Determine cluster mode and create shared cluster resources
    let is_standalone = app_state.configuration.is_standalone();
    let local_ip = batata_common::local_ip();
    let main_port = app_state.configuration.server_main_port();
    let local_address = format!("{}:{}", local_ip, main_port);

    // Get the real cluster members map and create a shared ClusterClientManager
    let core_config = app_state.configuration.to_core_config();
    let cluster_client_config = ClusterClientConfig::from_configuration(&core_config);
    let (members, cluster_client_manager) = if !is_standalone {
        let members = server_member_manager
            .as_ref()
            .map(|smm| smm.server_list())
            .unwrap_or_else(|| Arc::new(DashMap::new()));
        let ccm = Arc::new(ClusterClientManager::new(
            local_address.clone(),
            cluster_client_config.clone(),
        ));
        (members, Some(ccm))
    } else {
        (Arc::new(DashMap::new()), None)
    };

    let config_change_notifier = Arc::new(batata_config::ConfigChangeNotifier::new());

    register_config_handlers(
        &mut handler_registry,
        app_state.clone(),
        config_fuzzy_watch_manager.clone(),
        connection_manager.clone(),
        cluster_client_manager.clone(),
        config_change_notifier.clone(),
    );

    let naming_service = naming_service.unwrap_or_else(|| Arc::new(NamingService::new()));

    // Create and initialize distro protocol using the SAME naming service and real cluster members
    let distro_config = DistroConfig::from_configuration(&core_config);
    let distro_protocol = create_distro_protocol(
        &local_address,
        naming_service.clone(),
        members,
        cluster_client_manager.clone().unwrap_or_else(|| {
            Arc::new(ClusterClientManager::new(
                local_address.clone(),
                cluster_client_config.clone(),
            ))
        }),
        distro_config,
    );

    // Pass distro_protocol to naming handlers only in cluster mode
    let distro_for_naming = if !is_standalone {
        Some(distro_protocol.clone())
    } else {
        None
    };

    // Wire distro to membership events: whenever a peer joins/leaves/state
    // changes, refresh the responsibility mapper and drop keys no longer
    // owned by this node. Without this, reconciliation waits up to one
    // verify cycle (~5s). The verify loop still runs its own periodic
    // refresh as a safety net.
    if !is_standalone {
        if let Some(ref smm) = server_member_manager {
            distro_protocol.subscribe_member_changes(smm.event_publisher().clone());
        }
    }

    // Build the ClientOperationServiceProxy that routes instance writes by
    // the `instance.ephemeral` flag:
    //   ephemeral=true  → EphemeralClientOperationService  (DashMap + Distro)
    //   ephemeral=false → PersistentClientOperationService (Raft + apply hook)
    //
    // Handlers consume this via trait object so both paths share the same
    // invocation surface. Standalone deployments still use the proxy, but
    // its persistent branch falls back to direct DashMap writes (logged
    // as a WARN at construction).
    let client_op_proxy = {
        use batata_naming::service::{
            ClientOperationServiceProxy, EphemeralClientOperationService,
            PersistentClientOperationService,
        };
        let ephemeral = Arc::new(EphemeralClientOperationService::new(
            naming_service.clone(),
            distro_for_naming.clone(),
        ));
        let persistent = Arc::new(PersistentClientOperationService::new(
            naming_service.clone(),
            raft_node.clone(),
        ));
        Arc::new(ClientOperationServiceProxy::new(ephemeral, persistent))
    };

    register_naming_handlers(
        &mut handler_registry,
        naming_service.clone(),
        naming_fuzzy_watch_manager,
        connection_manager.clone(),
        distro_for_naming.clone(),
        raft_node.clone(),
        client_op_proxy,
    );

    // Register the naming disconnect listener on the core ConnectionManager.
    //
    // Equivalent to Nacos `ClientConnectionEventListener.clientDisconnected()`:
    // whenever a client's connection is torn down (stale ejection, push
    // circuit breaker, explicit unregister), every ephemeral instance that
    // client published is deregistered and affected subscribers are notified.
    //
    // Note: the bi-stream close path already runs `deregister_all_by_connection`
    // via `ConnectionCleanupHandler` before calling `unregister()`. The listener
    // is idempotent (deregister_all_by_connection on an already-cleared
    // connection is a cheap no-op) and covers all the *other* unregister paths
    // that don't go through the bi-stream close.
    {
        let pusher = Arc::new(batata_naming::service::ConnectionManagerPusher::new(
            connection_manager.clone(),
        ));
        let listener = Arc::new(batata_naming::service::NamingDisconnectListener::new(
            naming_service.clone(),
            pusher,
            distro_for_naming.clone(),
        ));
        connection_manager.add_listener(listener);
    }

    // Register distro handlers
    register_distro_handlers(&mut handler_registry, distro_protocol.clone());

    // Register cluster handlers
    register_cluster_handlers(&mut handler_registry, &server_member_manager);

    // Register lock handlers. In cluster mode, pass raft_node so locks go
    // through Raft consensus (for Nacos SDK compatibility).
    let lock_service = Arc::new(LockService::new());
    register_lock_handlers(
        &mut handler_registry,
        lock_service,
        grpc_auth_service_arc.clone(),
        raft_node.clone(),
    );

    // Register AI handlers (MCP + A2A)
    register_ai_handlers(&mut handler_registry, ai_services);

    let handler_registry_arc = Arc::new(handler_registry);

    // Build the connection cleanup handler. In cluster mode, wrap
    // NamingService so bi-stream disconnect triggers an immediate
    // Distro sync push to peers; in standalone mode, use NamingService
    // directly since there are no peers to notify.
    let cleanup_handler: Arc<dyn batata_core::handler::rpc::ConnectionCleanupHandler> =
        if let Some(ref distro) = distro_for_naming {
            Arc::new(batata_naming::DistroAwareCleanup::new(
                naming_service.clone(),
                distro.clone(),
            ))
        } else {
            naming_service.clone()
        };

    // Create gRPC services (reuse connection_manager created earlier)
    let grpc_request_service = GrpcRequestService::from_arc(handler_registry_arc.clone());
    let grpc_bi_request_stream_service = GrpcBiRequestStreamService::from_arc(
        handler_registry_arc.clone(),
        connection_manager,
        config_subscriber_manager,
        Some(cleanup_handler),
    );

    // Capture gRPC performance tuning parameters from configuration
    let tcp_keepalive = Duration::from_secs(app_state.configuration.grpc_tcp_keepalive_secs());
    let tcp_nodelay = app_state.configuration.grpc_tcp_nodelay();
    let http2_interval =
        Duration::from_secs(app_state.configuration.grpc_http2_keepalive_interval_secs());
    let http2_timeout =
        Duration::from_secs(app_state.configuration.grpc_http2_keepalive_timeout_secs());
    let concurrency = app_state.configuration.grpc_concurrency_limit();
    let max_concurrent_streams = app_state.configuration.grpc_max_concurrent_streams();
    let initial_connection_window = app_state
        .configuration
        .grpc_initial_connection_window_size();
    let initial_stream_window = app_state.configuration.grpc_initial_stream_window_size();
    let max_frame_size = app_state.configuration.grpc_max_frame_size();

    // Start SDK gRPC server
    let grpc_sdk_addr = format!("0.0.0.0:{}", sdk_server_port).parse()?;
    let sdk_tls_config = tls_config.clone();
    let sdk_use_tls = sdk_tls_config.should_use_sdk_tls();
    info!(
        "Starting SDK gRPC server on {} (TLS: {})",
        grpc_sdk_addr, sdk_use_tls
    );
    let sdk_server = {
        let grpc_request_service = grpc_request_service.clone();
        let grpc_bi_request_stream_service = grpc_bi_request_stream_service.clone();
        let layer = layer.clone();
        let mut sdk_shutdown_rx = _shutdown_rx.clone();
        let sdk_shutdown = async move {
            let _ = sdk_shutdown_rx.wait_for(|&v| v).await;
        };
        tokio::spawn(async move {
            let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
                if sdk_use_tls {
                    info!("SDK gRPC server starting with TLS enabled");
                    let server_tls_config = sdk_tls_config.create_server_tls_config().await?;
                    tonic::transport::Server::builder()
                        .tls_config(server_tls_config)?
                        .tcp_keepalive(Some(tcp_keepalive))
                        .tcp_nodelay(tcp_nodelay)
                        .http2_keepalive_interval(Some(http2_interval))
                        .http2_keepalive_timeout(Some(http2_timeout))
                        .initial_connection_window_size(initial_connection_window)
                        .initial_stream_window_size(initial_stream_window)
                        .max_frame_size(max_frame_size)
                        .concurrency_limit_per_connection(concurrency)
                        .max_concurrent_streams(max_concurrent_streams)
                        .layer(layer)
                        .add_service(RequestServer::new(grpc_request_service))
                        .add_service(BiRequestStreamServer::new(grpc_bi_request_stream_service))
                        .serve_with_shutdown(grpc_sdk_addr, sdk_shutdown)
                        .await?;
                } else {
                    tonic::transport::Server::builder()
                        .tcp_keepalive(Some(tcp_keepalive))
                        .tcp_nodelay(tcp_nodelay)
                        .http2_keepalive_interval(Some(http2_interval))
                        .http2_keepalive_timeout(Some(http2_timeout))
                        .initial_connection_window_size(initial_connection_window)
                        .initial_stream_window_size(initial_stream_window)
                        .max_frame_size(max_frame_size)
                        .concurrency_limit_per_connection(concurrency)
                        .max_concurrent_streams(max_concurrent_streams)
                        .layer(layer)
                        .add_service(RequestServer::new(grpc_request_service))
                        .add_service(BiRequestStreamServer::new(grpc_bi_request_stream_service))
                        .serve_with_shutdown(grpc_sdk_addr, sdk_shutdown)
                        .await?;
                }
                Ok(())
            }
            .await;
            if let Err(e) = result {
                tracing::error!("SDK gRPC server error: {}", e);
            }
        })
    };

    // Start cluster gRPC server
    let grpc_cluster_addr = format!("0.0.0.0:{}", cluster_server_port).parse()?;
    let cluster_tls_config = tls_config;
    let cluster_use_tls = cluster_tls_config.should_use_cluster_tls();
    info!(
        "Starting cluster gRPC server on {} (TLS: {})",
        grpc_cluster_addr, cluster_use_tls
    );
    let cluster_server = {
        let mut cluster_shutdown_rx = _shutdown_rx.clone();
        let cluster_shutdown = async move {
            let _ = cluster_shutdown_rx.wait_for(|&v| v).await;
        };
        tokio::spawn(async move {
            let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
                if cluster_use_tls {
                    info!("Cluster gRPC server starting with TLS enabled");
                    let server_tls_config = cluster_tls_config.create_server_tls_config().await?;
                    tonic::transport::Server::builder()
                        .tls_config(server_tls_config)?
                        .tcp_keepalive(Some(tcp_keepalive))
                        .http2_keepalive_interval(Some(http2_interval))
                        .http2_keepalive_timeout(Some(http2_timeout))
                        .concurrency_limit_per_connection(concurrency)
                        .layer(layer)
                        .add_service(RequestServer::new(grpc_request_service))
                        .add_service(BiRequestStreamServer::new(grpc_bi_request_stream_service))
                        .serve_with_shutdown(grpc_cluster_addr, cluster_shutdown)
                        .await?;
                } else {
                    tonic::transport::Server::builder()
                        .tcp_keepalive(Some(tcp_keepalive))
                        .http2_keepalive_interval(Some(http2_interval))
                        .http2_keepalive_timeout(Some(http2_timeout))
                        .concurrency_limit_per_connection(concurrency)
                        .layer(layer)
                        .add_service(RequestServer::new(grpc_request_service))
                        .add_service(BiRequestStreamServer::new(grpc_bi_request_stream_service))
                        .serve_with_shutdown(grpc_cluster_addr, cluster_shutdown)
                        .await?;
                }
                Ok(())
            }
            .await;
            if let Err(e) = result {
                tracing::error!("Cluster gRPC server error: {}", e);
            }
        })
    };

    // Start dedicated Raft gRPC server (cluster mode — both embedded and
    // external-db). Pre-bind the TCP port synchronously so it is listening
    // BEFORE this function returns. This prevents a race condition where
    // `raft_node.initialize()` (called later in main.rs) triggers leader
    // election before peer Raft gRPC servers have bound their ports.
    let raft_server_handle = if let Some(raft_node) = raft_node {
        let raft_port = app_state.configuration.raft_port();
        let grpc_raft_addr: std::net::SocketAddr = format!("0.0.0.0:{}", raft_port).parse()?;
        info!("Starting Batata Raft gRPC server on {}", grpc_raft_addr);

        let std_listener = std::net::TcpListener::bind(grpc_raft_addr)?;
        std_listener.set_nonblocking(true)?;
        info!("Batata Raft gRPC port {} bound and listening", raft_port);

        let raft_holder = Arc::new(tokio::sync::RwLock::new(Some(raft_node)));
        let raft_service = batata_consistency::raft::RaftGrpcService::new(raft_holder.clone());
        let raft_mgmt_service =
            batata_consistency::raft::RaftManagementGrpcService::new(raft_holder);

        let raft_tcp_keepalive =
            std::time::Duration::from_secs(app_state.configuration.raft_grpc_tcp_keepalive_secs());
        let raft_tcp_nodelay = app_state.configuration.raft_grpc_tcp_nodelay();
        let raft_http2_interval = std::time::Duration::from_secs(
            app_state
                .configuration
                .raft_grpc_http2_keepalive_interval_secs(),
        );
        let raft_http2_timeout = std::time::Duration::from_secs(
            app_state
                .configuration
                .raft_grpc_http2_keepalive_timeout_secs(),
        );

        let handle = tokio::spawn(async move {
            let tokio_listener = match tokio::net::TcpListener::from_std(std_listener) {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Failed to convert Batata Raft TCP listener: {}", e);
                    return;
                }
            };
            let incoming = tokio_stream::wrappers::TcpListenerStream::new(tokio_listener);
            let result = tonic::transport::Server::builder()
                .tcp_keepalive(Some(raft_tcp_keepalive))
                .tcp_nodelay(raft_tcp_nodelay)
                .http2_keepalive_interval(Some(raft_http2_interval))
                .http2_keepalive_timeout(Some(raft_http2_timeout))
                .add_service(
                    batata_api::raft::raft_service_server::RaftServiceServer::new(raft_service),
                )
                .add_service(
                    batata_api::raft::raft_management_service_server::RaftManagementServiceServer::new(
                        raft_mgmt_service,
                    ),
                )
                .serve_with_incoming(incoming)
                .await;
            if let Err(e) = result {
                tracing::error!("Batata Raft gRPC server error: {}", e);
            }
        });
        Some(handle)
    } else {
        None
    };

    let sdk_state = batata_core::ServerStateTracker::new();
    sdk_state.set_running();
    let cluster_state = batata_core::ServerStateTracker::new();
    cluster_state.set_running();
    let raft_state = batata_core::ServerStateTracker::new();
    if raft_server_handle.is_some() {
        raft_state.set_running();
    }

    Ok(GrpcServers {
        _sdk_server: sdk_server,
        _cluster_server: cluster_server,
        _raft_server: raft_server_handle,
        shutdown_tx,
        sdk_state,
        cluster_state,
        raft_state,
        naming_service,
        connection_manager: connection_manager_for_http,
        distro_protocol,
        cluster_client_manager,
        config_change_notifier,
        handler_registry: handler_registry_arc,
    })
}
