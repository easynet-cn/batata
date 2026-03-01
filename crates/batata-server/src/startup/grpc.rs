//! gRPC server setup and handler registration module.

use std::sync::Arc;

use dashmap::DashMap;
use tonic::service::InterceptorLayer;
use tower::ServiceBuilder;
use tracing::info;

use batata_api::model::Member;
use batata_consistency::RaftNode;
use batata_core::{
    GrpcAuthService,
    service::{
        cluster_client::{ClusterClientConfig, ClusterClientManager},
        distro::{DistroConfig, DistroProtocol},
        remote::{ConnectionManager, context_interceptor},
    },
};
use batata_naming::handler::distro::NamingInstanceDistroHandler;

use crate::model::tls::validate_tls_config;
use crate::startup::AIServices;

use crate::{
    api::grpc::{bi_request_stream_server::BiRequestStreamServer, request_server::RequestServer},
    model::common::AppState,
    service::{
        ai_handler::{
            AgentEndpointHandler, McpServerEndpointHandler, QueryAgentCardHandler,
            QueryMcpServerHandler, ReleaseAgentCardHandler, ReleaseMcpServerHandler,
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
            DistroDataSnapshotHandler, DistroDataSyncHandler, DistroDataVerifyHandler,
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
    /// Handle for the SDK gRPC server task (kept for potential graceful shutdown).
    _sdk_server: tokio::task::JoinHandle<()>,
    /// Handle for the cluster gRPC server task (kept for potential graceful shutdown).
    _cluster_server: tokio::task::JoinHandle<()>,
    /// Handle for the Raft gRPC server task (only in distributed embedded mode).
    _raft_server: Option<tokio::task::JoinHandle<()>>,
    /// The naming service used by handlers.
    pub naming_service: Arc<NamingService>,
    /// The connection manager for tracking client connections.
    pub connection_manager: Arc<ConnectionManager>,
    /// The distro protocol for cluster data synchronization.
    pub distro_protocol: Arc<DistroProtocol>,
    /// The cluster client manager for inter-node communication.
    pub cluster_client_manager: Option<Arc<ClusterClientManager>>,
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
    registry.register_handler(Arc::new(ServerLoaderInfoHandler { connection_manager }));
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
) {
    registry.register_handler(Arc::new(ConfigQueryHandler {
        app_state: app_state.clone(),
    }));
    registry.register_handler(Arc::new(ConfigPublishHandler {
        app_state: app_state.clone(),
        fuzzy_watch_manager: fuzzy_watch_manager.clone(),
        connection_manager: connection_manager.clone(),
        cluster_client_manager: cluster_client_manager.clone(),
    }));
    registry.register_handler(Arc::new(ConfigRemoveHandler {
        app_state: app_state.clone(),
        fuzzy_watch_manager: fuzzy_watch_manager.clone(),
        connection_manager: connection_manager.clone(),
        cluster_client_manager,
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
    }));
    registry.register_handler(Arc::new(ConfigFuzzyWatchHandler {
        app_state: app_state.clone(),
        fuzzy_watch_manager: fuzzy_watch_manager.clone(),
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
) {
    registry.register_handler(Arc::new(InstanceRequestHandler {
        naming_service: naming_service.clone(),
        naming_fuzzy_watch_manager: naming_fuzzy_watch_manager.clone(),
        connection_manager: connection_manager.clone(),
        distro_protocol: distro_protocol.clone(),
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
    }));
    registry.register_handler(Arc::new(NotifySubscriberHandler {
        naming_service: naming_service.clone(),
    }));
    registry.register_handler(Arc::new(NamingFuzzyWatchHandler {
        naming_service: naming_service.clone(),
        naming_fuzzy_watch_manager: naming_fuzzy_watch_manager.clone(),
    }));
    registry.register_handler(Arc::new(NamingFuzzyWatchChangeNotifyHandler {
        naming_service: naming_service.clone(),
        naming_fuzzy_watch_manager: naming_fuzzy_watch_manager.clone(),
    }));
    registry.register_handler(Arc::new(NamingFuzzyWatchSyncHandler {
        naming_service,
        naming_fuzzy_watch_manager,
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
    registry.register_handler(Arc::new(DistroDataSnapshotHandler { distro_protocol }));
}

/// Registers the cluster member report handler.
fn register_cluster_handlers(registry: &mut HandlerRegistry, app_state: Arc<AppState>) {
    if let Some(member_manager) = app_state.try_member_manager() {
        registry.register_handler(Arc::new(MemberReportHandler {
            member_manager: member_manager.clone(),
        }));
    }
}

/// Registers the lock operation handler.
fn register_lock_handlers(
    registry: &mut HandlerRegistry,
    lock_service: Arc<LockService>,
    auth_service: Arc<GrpcAuthService>,
) {
    registry.register_handler(Arc::new(LockOperationHandler {
        lock_service,
        auth_service,
    }));
}

/// Registers all AI handlers (MCP + A2A).
fn register_ai_handlers(
    registry: &mut HandlerRegistry,
    ai_services: &AIServices,
    auth_service: Arc<GrpcAuthService>,
) {
    registry.register_handler(Arc::new(McpServerEndpointHandler {
        mcp_registry: ai_services.mcp_registry.clone(),
        mcp_service: ai_services.mcp_service.clone(),
        endpoint_service: ai_services.endpoint_service.clone(),
        auth_service: auth_service.clone(),
    }));
    registry.register_handler(Arc::new(QueryMcpServerHandler {
        mcp_registry: ai_services.mcp_registry.clone(),
        mcp_service: ai_services.mcp_service.clone(),
        auth_service: auth_service.clone(),
    }));
    registry.register_handler(Arc::new(ReleaseMcpServerHandler {
        mcp_registry: ai_services.mcp_registry.clone(),
        mcp_service: ai_services.mcp_service.clone(),
        auth_service: auth_service.clone(),
    }));
    registry.register_handler(Arc::new(AgentEndpointHandler {
        agent_registry: ai_services.agent_registry.clone(),
        a2a_service: ai_services.a2a_service.clone(),
        endpoint_service: ai_services.endpoint_service.clone(),
        auth_service: auth_service.clone(),
    }));
    registry.register_handler(Arc::new(QueryAgentCardHandler {
        agent_registry: ai_services.agent_registry.clone(),
        a2a_service: ai_services.a2a_service.clone(),
        auth_service: auth_service.clone(),
    }));
    registry.register_handler(Arc::new(ReleaseAgentCardHandler {
        agent_registry: ai_services.agent_registry.clone(),
        a2a_service: ai_services.a2a_service.clone(),
        auth_service,
    }));
}

/// Creates and initializes the Distro protocol with the naming service handler.
fn create_distro_protocol(
    local_address: &str,
    naming_service: Arc<batata_naming::NamingService>,
    members: Arc<DashMap<String, Member>>,
    cluster_client_manager: Arc<ClusterClientManager>,
) -> Arc<DistroProtocol> {
    // Create distro protocol with the real cluster members and client manager
    let distro_config = DistroConfig::default();
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

/// Creates and starts gRPC servers for SDK and cluster communication.
///
/// # Arguments
/// * `app_state` - Application state containing configuration and services
/// * `sdk_server_port` - Port for SDK gRPC server
/// * `cluster_server_port` - Port for cluster gRPC server
///
/// # Returns
/// A `GrpcServers` struct containing the spawned server handles and naming service.
pub fn start_grpc_servers(
    app_state: Arc<AppState>,
    naming_service: Option<Arc<NamingService>>,
    ai_services: &AIServices,
    sdk_server_port: u16,
    cluster_server_port: u16,
    raft_node: Option<Arc<RaftNode>>,
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
    // Setup gRPC interceptor layer
    let layer = ServiceBuilder::new()
        .load_shed()
        .layer(InterceptorLayer::new(context_interceptor))
        .into_inner();

    // Create connection manager first (needed by handlers and stream service)
    let connection_manager = Arc::new(ConnectionManager::new());
    // Keep a clone for the HTTP server
    let connection_manager_for_http = connection_manager.clone();

    // Create gRPC auth service based on configuration
    let auth_enabled = app_state.configuration.auth_enabled();
    let token_secret_key = app_state.configuration.token_secret_key();
    let server_identity_key = app_state.configuration.server_identity_key();
    let server_identity_value = app_state.configuration.server_identity_value();

    let grpc_auth_service = GrpcAuthService::new(
        auth_enabled,
        token_secret_key,
        server_identity_key,
        server_identity_value,
    );
    let grpc_auth_service_arc = Arc::new(grpc_auth_service.clone());

    if auth_enabled {
        info!("gRPC authentication is enabled");
    }

    // Initialize gRPC handlers with auth service
    let mut handler_registry = HandlerRegistry::with_auth(grpc_auth_service);

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
        let members = app_state
            .server_member_manager
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

    register_config_handlers(
        &mut handler_registry,
        app_state.clone(),
        config_fuzzy_watch_manager.clone(),
        connection_manager.clone(),
        cluster_client_manager.clone(),
    );

    let naming_service = naming_service.unwrap_or_else(|| Arc::new(NamingService::new()));

    // Create and initialize distro protocol using the SAME naming service and real cluster members
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
    );

    // Pass distro_protocol to naming handlers only in cluster mode
    let distro_for_naming = if !is_standalone {
        Some(distro_protocol.clone())
    } else {
        None
    };

    register_naming_handlers(
        &mut handler_registry,
        naming_service.clone(),
        naming_fuzzy_watch_manager,
        connection_manager.clone(),
        distro_for_naming,
    );

    // Register distro handlers
    register_distro_handlers(&mut handler_registry, distro_protocol.clone());

    // Register cluster handlers
    register_cluster_handlers(&mut handler_registry, app_state.clone());

    // Register lock handlers
    let lock_service = Arc::new(LockService::new());
    register_lock_handlers(
        &mut handler_registry,
        lock_service,
        grpc_auth_service_arc.clone(),
    );

    // Register AI handlers (MCP + A2A)
    register_ai_handlers(&mut handler_registry, ai_services, grpc_auth_service_arc);

    let handler_registry_arc = Arc::new(handler_registry);

    // Create gRPC services (reuse connection_manager created earlier)
    let grpc_request_service = GrpcRequestService::from_arc(handler_registry_arc.clone());
    let grpc_bi_request_stream_service = GrpcBiRequestStreamService::from_arc(
        handler_registry_arc,
        connection_manager,
        app_state.config_subscriber_manager.clone(),
        Some(naming_service.clone() as Arc<dyn batata_core::handler::rpc::ConnectionCleanupHandler>),
    );

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
        tokio::spawn(async move {
            let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
                if sdk_use_tls {
                    info!("SDK gRPC server starting with TLS enabled");
                    let server_tls_config = sdk_tls_config.create_server_tls_config().await?;
                    tonic::transport::Server::builder()
                        .tls_config(server_tls_config)?
                        .layer(layer)
                        .add_service(RequestServer::new(grpc_request_service))
                        .add_service(BiRequestStreamServer::new(grpc_bi_request_stream_service))
                        .serve(grpc_sdk_addr)
                        .await?;
                } else {
                    tonic::transport::Server::builder()
                        .layer(layer)
                        .add_service(RequestServer::new(grpc_request_service))
                        .add_service(BiRequestStreamServer::new(grpc_bi_request_stream_service))
                        .serve(grpc_sdk_addr)
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
    let cluster_server = tokio::spawn(async move {
        let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
            if cluster_use_tls {
                info!("Cluster gRPC server starting with TLS enabled");
                let server_tls_config = cluster_tls_config.create_server_tls_config().await?;
                tonic::transport::Server::builder()
                    .tls_config(server_tls_config)?
                    .layer(layer)
                    .add_service(RequestServer::new(grpc_request_service))
                    .add_service(BiRequestStreamServer::new(grpc_bi_request_stream_service))
                    .serve(grpc_cluster_addr)
                    .await?;
            } else {
                tonic::transport::Server::builder()
                    .layer(layer)
                    .add_service(RequestServer::new(grpc_request_service))
                    .add_service(BiRequestStreamServer::new(grpc_bi_request_stream_service))
                    .serve(grpc_cluster_addr)
                    .await?;
            }
            Ok(())
        }
        .await;
        if let Err(e) = result {
            tracing::error!("Cluster gRPC server error: {}", e);
        }
    });

    // Start dedicated Raft gRPC server (only in distributed embedded mode)
    // Pre-bind the TCP port synchronously so it is listening BEFORE this
    // function returns. This prevents a race condition where
    // `raft_node.initialize()` (called later in main.rs) triggers leader
    // election before peer Raft gRPC servers have bound their ports.
    let raft_server_handle = if let Some(raft_node) = raft_node {
        let raft_port = app_state.configuration.raft_port();
        let grpc_raft_addr: std::net::SocketAddr = format!("0.0.0.0:{}", raft_port).parse()?;
        info!("Starting Raft gRPC server on {}", grpc_raft_addr);

        // Pre-bind the port synchronously to guarantee it is listening
        let std_listener = std::net::TcpListener::bind(grpc_raft_addr)?;
        std_listener.set_nonblocking(true)?;
        info!("Raft gRPC port {} bound and listening", raft_port);

        // Wrap in Arc<RwLock<Option<...>>> as required by the Raft gRPC services
        let raft_holder = Arc::new(tokio::sync::RwLock::new(Some(raft_node)));

        let raft_service = batata_consistency::raft::RaftGrpcService::new(raft_holder.clone());
        let raft_mgmt_service =
            batata_consistency::raft::RaftManagementGrpcService::new(raft_holder);

        let handle = tokio::spawn(async move {
            let tokio_listener = match tokio::net::TcpListener::from_std(std_listener) {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Failed to convert Raft TCP listener: {}", e);
                    return;
                }
            };
            let incoming = tokio_stream::wrappers::TcpListenerStream::new(tokio_listener);
            let result = tonic::transport::Server::builder()
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
                tracing::error!("Raft gRPC server error: {}", e);
            }
        });
        Some(handle)
    } else {
        None
    };

    Ok(GrpcServers {
        _sdk_server: sdk_server,
        _cluster_server: cluster_server,
        _raft_server: raft_server_handle,
        naming_service,
        connection_manager: connection_manager_for_http,
        distro_protocol,
        cluster_client_manager,
    })
}
