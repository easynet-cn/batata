//! Main entry point for Batata Nacos-compatible server.
//!
//! This file sets up and starts the HTTP and gRPC servers with their respective services.

use std::sync::Arc;
use std::time::Duration;

use batata_auth::service::oauth::OAuthService;
use batata_consistency::raft::state_machine::{
    CF_CONSUL_ACL, CF_CONSUL_KV, CF_CONSUL_QUERIES, CF_CONSUL_SESSIONS,
};
use batata_core::cluster::ServerMemberManager;
use batata_migration::{Migrator, MigratorTrait};
use batata_naming::InstanceCheckRegistry;
use batata_naming::healthcheck::{HealthCheckConfig, HealthCheckManager};
use batata_naming::healthcheck::{deregister_monitor::DeregisterMonitor, ttl_monitor::TtlMonitor};
use batata_persistence::{PersistenceService, StorageMode};
use batata_plugin::Plugin as _;
use batata_server::{
    middleware::rate_limit,
    model::{self, common::AppState},
    startup::{
        self, AIServices, ConsulServices, GracefulShutdown, OtelConfig, XdsServerHandle,
        start_xds_service,
    },
};
use batata_server_common::ServerStatusManager;
use rocksdb::ColumnFamilyDescriptor;
use tracing::{error, info};

#[allow(clippy::type_complexity)]
#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize configuration and logging
    let configuration = model::common::Configuration::new()?;

    // Validate JWT secret key when auth is enabled
    if configuration.auth_enabled() && configuration.token_secret_key().is_empty() {
        eprintln!(
            "FATAL: Authentication is enabled (batata.core.auth.enabled=true) but no JWT secret key is configured."
        );
        eprintln!(
            "Set 'batata.core.auth.plugin.nacos.token.secret.key' to a non-empty Base64-encoded secret."
        );
        std::process::exit(1);
    }

    // Warn if using the default JWT secret key (insecure for production)
    const DEFAULT_SECRET_KEYS: &[&str] = &[
        "NzViOWFlNjYtMWM3MC00ZDYwLTg4OWUtMjYxYTdhMzA1Y2Jm",
        "VGhpc0lzTXlDdXN0b21TZWNyZXRLZXkwMTIzNDU2Nzg=",
    ];
    if configuration.auth_enabled()
        && DEFAULT_SECRET_KEYS.contains(&configuration.token_secret_key().as_str())
    {
        eprintln!("WARNING: Using the default JWT secret key. This is insecure for production!");
        eprintln!("Generate a new key with: openssl rand -base64 32");
        eprintln!("Set it via: batata.core.auth.plugin.nacos.token.secret.key=<your-key>");
    }

    // Initialize multi-file logging with optional OpenTelemetry support
    let logging_config = startup::LoggingConfig::from_config(
        configuration.log_dir(),
        configuration.log_console_enabled(),
        configuration.log_file_enabled(),
        configuration.log_level(),
    );
    let otel_config = OtelConfig::from_config(
        configuration.otel_enabled(),
        configuration.otel_endpoint(),
        configuration.otel_service_name(),
        configuration.otel_sampling_ratio(),
        configuration.otel_export_timeout_secs(),
    );
    let _logging_guard = startup::init_logging(&logging_config, Some(&otel_config))?;

    if otel_config.enabled {
        info!(
            "OpenTelemetry tracing enabled, exporting to {}",
            otel_config.otlp_endpoint
        );
    }

    // Initialize metrics for observability
    batata_server::metrics::init_metrics();

    // Start background cleanup task for rate limiters to prevent memory leaks
    let _rate_limit_cleanup_handle = rate_limit::start_cleanup_task();

    // Extract configuration parameters
    let deployment_type = configuration.deployment_type();
    let is_console_remote = deployment_type == model::common::NACOS_DEPLOYMENT_TYPE_CONSOLE;

    if is_console_remote
        && configuration.console_remote_username() == "batata"
        && configuration.console_remote_password() == "batata"
    {
        tracing::warn!(
            "Console remote mode is using default credentials (batata/batata). \
             This is insecure for production environments. \
             Set 'batata.console.remote.username' and 'batata.console.remote.password'."
        );
    }

    let server_address = configuration.server_address();
    let console_server_address = server_address.clone();
    let console_server_port = configuration.console_server_port();
    let console_context_path = configuration.console_server_context_path();
    let server_main_port = configuration.server_main_port();
    let server_context_path = configuration.server_context_path();
    let sdk_server_port = configuration.sdk_server_port();
    let cluster_server_port = configuration.cluster_server_port();
    let consul_enabled = configuration.consul_enabled();
    let consul_acl_enabled = configuration.consul_acl_enabled();
    let consul_server_port = configuration.consul_server_port();
    let consul_server_address = server_address.clone();
    let consul_dc_config =
        batata_plugin_consul::model::ConsulDatacenterConfig::new(configuration.consul_datacenter())
            .with_primary(configuration.consul_primary_datacenter())
            .with_consul_version(configuration.consul_version())
            .with_batata_version(configuration.batata_version())
            .with_default_namespace(configuration.consul_default_namespace())
            .with_default_group(configuration.consul_default_group())
            .with_consul_port(consul_server_port);
    let mcp_registry_enabled = configuration.mcp_registry_enabled()
        || deployment_type == model::common::NACOS_DEPLOYMENT_TYPE_SERVER_WITH_MCP;
    let mcp_registry_port = configuration.mcp_registry_port();
    let mcp_registry_address = server_address.clone();

    // Initialize database, persistence service, and server member manager based on deployment mode
    let storage_mode = configuration.persistence_mode();
    info!("Persistence mode: {}", storage_mode);

    let (database_connection, server_member_manager, persistence, _rocks_db, raft_node): (
        Option<sea_orm::DatabaseConnection>,
        Option<Arc<ServerMemberManager>>,
        Option<Arc<dyn PersistenceService>>,
        Option<Arc<rocksdb::DB>>,
        Option<Arc<batata_consistency::RaftNode>>,
    ) = if is_console_remote {
        info!("Starting in console remote mode - connecting to remote server");
        (None, None, None, None, None)
    } else {
        match storage_mode {
            StorageMode::ExternalDb => {
                let db = configuration.database_connection().await?;

                // Run database migrations if enabled
                if configuration.db_migration_enabled() {
                    info!("Running database migrations...");
                    Migrator::up(&db, None).await?;
                    info!("Database migrations completed successfully");
                }

                let core_config = configuration.to_core_config();
                let smm = Arc::new(ServerMemberManager::new(&core_config));
                let persist: Arc<dyn PersistenceService> = Arc::new(
                    batata_persistence::ExternalDbPersistService::new(db.clone()),
                );
                (Some(db), Some(smm), Some(persist), None, None)
            }
            StorageMode::StandaloneEmbedded => {
                let data_dir = configuration.embedded_data_dir();
                info!("Initializing standalone embedded storage at: {}", data_dir);
                let rocks_config = configuration.rocksdb_config();
                let sm = batata_consistency::RocksStateMachine::with_options(
                    &data_dir,
                    Some(rocks_config.to_db_options()),
                    Some(rocks_config.to_cf_options()),
                )
                .await
                .map_err(|e| format!("Failed to initialize RocksDB state machine: {}", e))?;
                let rdb = sm.db();
                let persist: Arc<dyn PersistenceService> =
                    Arc::new(batata_persistence::EmbeddedPersistService::from_state_machine(&sm));
                let core_config = configuration.to_core_config();
                let smm = Arc::new(ServerMemberManager::new(&core_config));
                (None, Some(smm), Some(persist), Some(rdb), None)
            }
            StorageMode::DistributedEmbedded => {
                let data_dir = configuration.embedded_data_dir();
                let main_port = configuration.server_main_port();

                // Determine this node's Raft address from cluster.conf.
                // All nodes must agree on the SAME set of member addresses
                // (using the SAME IPs from cluster.conf), so we find our own
                // entry by matching the port, and derive the raft port from it.
                let local_ip = batata_common::local_ip();
                let node_addr = {
                    let cluster_addrs = configuration.cluster_member_addresses();
                    let mut matched_ip = local_ip.clone();
                    for addr_str in &cluster_addrs {
                        // Strip query params (e.g., ?raft_port=xxx)
                        let addr_part = addr_str.split('?').next().unwrap_or(addr_str);
                        if let Some((ip, port_str)) = addr_part.rsplit_once(':')
                            && let Ok(port) = port_str.parse::<u16>()
                            && port == main_port
                        {
                            matched_ip = ip.to_string();
                            break;
                        }
                    }
                    let raft_port = main_port - batata_api::model::Member::DEFAULT_RAFT_OFFSET_PORT;
                    format!("{}:{}", matched_ip, raft_port)
                };
                let node_id = batata_consistency::calculate_node_id(&node_addr);
                info!(
                    "Initializing distributed embedded storage: node_id={}, addr={}, data_dir={}",
                    node_id, node_addr, data_dir
                );

                let raft_config = batata_consistency::RaftConfig {
                    data_dir: std::path::PathBuf::from(&data_dir),
                    ..Default::default()
                };

                // Create Raft node with configurable RocksDB options
                let rocks_config = configuration.rocksdb_config();
                let (raft_node, rdb) = batata_consistency::RaftNode::new_with_db_and_options(
                    node_id,
                    node_addr.clone(),
                    raft_config,
                    Some(rocks_config.to_db_options()),
                    Some(rocks_config.to_cf_options()),
                )
                .await
                .map_err(|e| format!("Failed to initialize Raft node: {}", e))?;

                let raft_node = Arc::new(raft_node);
                let reader = batata_consistency::RocksDbReader::new(rdb.clone());
                let persist: Arc<dyn PersistenceService> = Arc::new(
                    batata_persistence::DistributedPersistService::new(raft_node.clone(), reader),
                );

                // Initialize single-node cluster in standalone mode
                if configuration.is_standalone() {
                    info!("Standalone distributed mode: initializing single-node Raft cluster");
                    let mut members = std::collections::BTreeMap::new();
                    members.insert(node_id, openraft::BasicNode { addr: node_addr });
                    if let Err(e) = raft_node.initialize(members).await {
                        // Already initialized is OK (e.g. on restart)
                        info!(
                            "Raft cluster init result: {} (already initialized is OK)",
                            e
                        );
                    }
                }

                let core_config = configuration.to_core_config();
                let smm = Arc::new(ServerMemberManager::new(&core_config));
                (None, Some(smm), Some(persist), Some(rdb), Some(raft_node))
            }
        }
    };

    // Create console datasource based on mode
    // Create config subscriber manager (shared between gRPC and console)
    let config_subscriber_manager = Arc::new(batata_core::ConfigSubscriberManager::new());

    // Create NamingService early so it can be shared with both console datasource and gRPC servers
    let naming_service: Option<Arc<batata_naming::NamingService>> = if !is_console_remote {
        Some(Arc::new(batata_naming::NamingService::new()))
    } else {
        None
    };

    let console_datasource = batata_console::create_datasource(
        &configuration,
        database_connection.clone(),
        server_member_manager.clone(),
        config_subscriber_manager.clone(),
        naming_service.clone(),
        persistence.clone(),
    )
    .await?;

    // Initialize OAuth service if enabled
    let oauth_service = if configuration.is_oauth_enabled() {
        let oauth_config = configuration.oauth_config();
        info!(
            "OAuth2/OIDC authentication enabled with {} providers",
            oauth_config.providers.len()
        );
        Some(Arc::new(OAuthService::new(oauth_config)?))
    } else {
        None
    };

    // Get Consul data directory and register_self config before moving configuration to app_state
    let consul_data_dir_for_init = configuration.consul_data_dir();
    let consul_register_self_for_init = configuration.consul_register_self();

    // Create health check manager (only for server mode, not console-remote)
    let health_check_manager: Option<Arc<HealthCheckManager>> = if let Some(ref ns) = naming_service
    {
        let health_check_config = Arc::new(HealthCheckConfig::default());
        let expire_enabled = configuration.expire_instance_enabled();
        let health_check_enabled = health_check_config.is_enabled();
        let manager = Arc::new(HealthCheckManager::new(
            ns.clone(),
            health_check_config,
            expire_enabled,
        ));
        info!(
            "Health check manager created (expire_enabled={}, health_check_enabled={})",
            expire_enabled, health_check_enabled
        );
        Some(manager)
    } else {
        info!("No naming service available - skipping health check manager");
        None
    };

    // Create server status manager (starts in STARTING state)
    let server_status = Arc::new(ServerStatusManager::new());

    // Initialize control plugin for TPS rate limiting and connection control
    let control_plugin: Option<Arc<dyn batata_plugin::ControlPlugin>> = {
        let control_config = configuration.control_plugin_config();
        if !control_config.enabled {
            info!("Control plugin disabled by configuration");
            None
        } else {
            let plugin = batata_plugin::DefaultControlPlugin::new(control_config);
            if let Err(e) = plugin.init().await {
                tracing::warn!("Failed to initialize control plugin: {}", e);
                None
            } else {
                info!(
                    "Control plugin initialized (TPS limit={}, max connections={})",
                    configuration.control_plugin_default_tps(),
                    configuration.control_plugin_max_connections()
                );
                Some(Arc::new(plugin))
            }
        }
    };

    // Initialize config encryption service
    let encryption_service: Arc<batata_config::service::encryption::ConfigEncryptionService> =
        if configuration.encryption_enabled() {
            match configuration.encryption_key() {
                Some(key) => {
                    match batata_config::service::encryption::ConfigEncryptionService::new(&key) {
                        Ok(svc) => {
                            info!("Config encryption enabled");
                            Arc::new(svc)
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create encryption service: {}", e);
                            Arc::new(
                                batata_config::service::encryption::ConfigEncryptionService::disabled(),
                            )
                        }
                    }
                }
                None => {
                    tracing::warn!("Encryption enabled but no key configured");
                    Arc::new(
                        batata_config::service::encryption::ConfigEncryptionService::disabled(),
                    )
                }
            }
        } else {
            Arc::new(batata_config::service::encryption::ConfigEncryptionService::disabled())
        };

    // Create application state
    let app_state = Arc::new(AppState {
        configuration,
        server_member_manager,
        config_subscriber_manager,
        console_datasource,
        oauth_service,
        persistence,
        health_check_manager: health_check_manager
            .map(|m| m as Arc<dyn std::any::Any + Send + Sync>),
        raft_node: raft_node.clone(),
        server_status: server_status.clone(),
        control_plugin,
        encryption_service: Some(encryption_service.clone() as Arc<dyn std::any::Any + Send + Sync>),
    });

    // If data warmup is disabled (default), transition to UP immediately.
    // Otherwise a background poller will set UP once subsystems are ready.
    if !app_state.configuration.data_warmup() {
        server_status.set_up();
        info!("Server status: UP (data warmup disabled)");
    } else {
        info!("Server status: STARTING (data warmup enabled, waiting for subsystems)");
    }

    // Initialize graceful shutdown handler with configurable drain timeout
    let drain_timeout_secs = app_state.configuration.shutdown_drain_timeout_secs();
    let shutdown_signal = startup::wait_for_shutdown_signal().await;
    let graceful_shutdown = GracefulShutdown::new(
        shutdown_signal.clone(),
        Duration::from_secs(drain_timeout_secs),
    );

    // Create shared AI services (used by both main and console servers)
    // Use config-backed persistence when available
    let ai_services = match (&app_state.persistence, &naming_service) {
        (Some(persist), Some(ns)) => {
            info!("AI services using config-backed persistence");
            AIServices::with_persistence(persist.clone(), ns.clone())
        }
        _ => {
            info!("AI services using in-memory storage (no persistence)");
            AIServices::new()
        }
    };

    // For console remote mode, only start console server
    if is_console_remote {
        info!(
            "Starting console server in remote mode on port {}",
            console_server_port
        );

        let console_server = startup::console_server(
            app_state.clone(),
            None, // No NamingService in remote mode
            ai_services.clone(),
            console_context_path,
            console_server_address,
            console_server_port,
        )?;

        tokio::select! {
            result = console_server => {
                if let Err(e) = result {
                    error!("Console server error: {}", e);
                }
            }
            _ = graceful_shutdown.wait_for_shutdown() => {
                info!("Console server shutting down gracefully");
            }
        }

        return Ok(());
    }

    // Start gRPC servers (including Raft gRPC if in distributed embedded mode)
    let grpc_servers = startup::start_grpc_servers(
        app_state.clone(),
        naming_service.clone(),
        &ai_services,
        sdk_server_port,
        cluster_server_port,
        raft_node,
    )?;

    // Start health check manager (unhealthy and expired instance checkers)
    if let Some(hc_any) = app_state.health_check_manager.as_ref()
        && let Ok(hc_manager) = Arc::clone(hc_any).downcast::<HealthCheckManager>()
    {
        // Spawn in separate tasks - they will keep running in background
        let hc_manager_clone = hc_manager.clone();
        tokio::spawn(async move {
            let unhealthy = hc_manager_clone.unhealthy_checker();
            unhealthy.start().await;
        });

        let hc_manager_clone2 = hc_manager.clone();
        tokio::spawn(async move {
            let expired = hc_manager_clone2.expired_checker();
            expired.start().await;
        });

        info!("Health check manager started (unhealthy/expired instance checkers)");
    }

    // If data warmup is enabled, spawn a background poller that checks subsystem
    // readiness every 5 seconds and transitions to UP once everything is ready.
    if app_state.configuration.data_warmup() {
        let status_mgr = server_status.clone();
        let console_ds = app_state.console_datasource.clone();
        let raft_ref = app_state.raft_node.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                let db_ready = console_ds.server_readiness().await;
                let (raft_ready, raft_reason) = match &raft_ref {
                    Some(raft) => raft.is_ready(),
                    None => (true, None),
                };

                if db_ready && raft_ready {
                    if !status_mgr.is_up() {
                        status_mgr.set_up();
                        status_mgr.set_error_msg(None).await;
                        info!("Server status: UP (all subsystems ready)");
                    }
                } else {
                    let mut reasons = Vec::new();
                    if !db_ready {
                        reasons.push("database not ready".to_string());
                    }
                    if let Some(reason) = raft_reason {
                        reasons.push(reason);
                    }
                    let msg = reasons.join(", ");
                    status_mgr.set_down();
                    status_mgr.set_error_msg(Some(msg)).await;
                }
            }
        });
        info!("Data warmup poller started (checking every 5s)");
    }

    // Start periodic MCP index refresh task if persistence is available
    if let Some(ref mcp_index) = ai_services.mcp_index
        && let Some(ref persist) = app_state.persistence
    {
        let index = mcp_index.clone();
        let persist = persist.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                index.refresh(persist.as_ref()).await;
            }
        });
        info!("MCP index periodic refresh task started (every 30s)");
    }

    // Start cluster manager if in cluster mode
    if let Some(ref smm) = app_state.server_member_manager {
        let startup_mode = app_state.configuration.startup_mode();
        info!("Starting in {} mode", startup_mode);

        if !app_state.configuration.is_standalone() {
            // Wire the shared DistroProtocol to ServerMemberManager before starting
            // so it uses the same protocol instance as the gRPC handlers
            smm.set_distro_protocol(grpc_servers.distro_protocol.clone())
                .await;

            info!("Initializing cluster management...");
            if let Err(e) = smm.start().await {
                error!("Failed to start cluster manager: {}", e);
                return Err(e.to_string().into());
            }
            info!("Cluster management started successfully");

            // Initialize Raft cluster with discovered members (distributed embedded mode)
            if let Some(ref raft_node) = app_state.raft_node {
                info!(
                    "Initializing Raft cluster (self: node_id={}, addr={})",
                    raft_node.node_id(),
                    raft_node.addr()
                );

                // Build Raft member list directly from cluster.conf (not the
                // SMM server_list which may have duplicates when the local IP
                // differs from cluster.conf entries). All nodes read the same
                // cluster.conf so they agree on the exact same member set.
                let cluster_addrs = app_state.configuration.cluster_member_addresses();
                let default_port = app_state.configuration.server_main_port();
                let raft_offset = batata_api::model::Member::DEFAULT_RAFT_OFFSET_PORT;
                let mut members = std::collections::BTreeMap::new();

                for addr_str in &cluster_addrs {
                    // Parse ip:port (strip ?raft_port=xxx query params)
                    let addr_part = addr_str.split('?').next().unwrap_or(addr_str);
                    let (ip, main_port) = if let Some((ip, port_str)) = addr_part.rsplit_once(':') {
                        (
                            ip.to_string(),
                            port_str.parse::<u16>().unwrap_or(default_port),
                        )
                    } else {
                        (addr_part.to_string(), default_port)
                    };

                    // Check for explicit raft_port in query params
                    let member_raft_port = addr_str
                        .split('?')
                        .nth(1)
                        .and_then(|params| {
                            params.split('&').find_map(|kv| {
                                let (k, v) = kv.split_once('=')?;
                                if k.trim() == "raft_port" {
                                    v.trim().parse::<u16>().ok()
                                } else {
                                    None
                                }
                            })
                        })
                        .unwrap_or_else(|| main_port - raft_offset);

                    let raft_addr = format!("{}:{}", ip, member_raft_port);
                    let node_id = batata_consistency::calculate_node_id(&raft_addr);
                    info!("Raft member: node_id={}, addr={}", node_id, raft_addr);
                    members.insert(node_id, openraft::BasicNode { addr: raft_addr });
                }

                info!("Raft cluster: {} members from cluster.conf", members.len());
                if !members.is_empty() {
                    // Wait for all peer Raft gRPC servers to be reachable before
                    // initializing the cluster. This prevents premature leader
                    // election when some peers haven't bound their ports yet.
                    // Matches Nacos's approach of ensuring RPC server readiness
                    // before Raft group creation.
                    let self_raft_addr = raft_node.addr().to_string();
                    let peer_addrs: Vec<String> = members
                        .values()
                        .filter(|n| n.addr != self_raft_addr)
                        .map(|n| n.addr.clone())
                        .collect();

                    if !peer_addrs.is_empty() {
                        info!(
                            "Waiting for {} Raft peer(s) to become reachable...",
                            peer_addrs.len()
                        );
                        let deadline = std::time::Instant::now() + Duration::from_secs(30);

                        for addr in &peer_addrs {
                            loop {
                                match tokio::net::TcpStream::connect(addr).await {
                                    Ok(_) => {
                                        info!("Raft peer {} is reachable", addr);
                                        break;
                                    }
                                    Err(_) => {
                                        if std::time::Instant::now() >= deadline {
                                            tracing::warn!(
                                                "Timeout waiting for Raft peer {} - proceeding anyway",
                                                addr
                                            );
                                            break;
                                        }
                                        tokio::time::sleep(Duration::from_millis(500)).await;
                                    }
                                }
                            }
                        }
                        info!("All Raft peers checked, proceeding with initialization");
                    }

                    if let Err(e) = raft_node.initialize(members).await {
                        // Already initialized is OK (e.g. on restart)
                        info!(
                            "Raft cluster init result: {} (already initialized is OK)",
                            e
                        );
                    } else {
                        info!("Raft cluster initialized successfully");
                    }
                } else {
                    error!("No cluster members in cluster.conf for Raft initialization");
                }
            }
        }
    }

    // Start xDS service for service mesh support (if enabled)
    let xds_handle: Option<XdsServerHandle> = {
        let xds_config = app_state.configuration.xds_config();
        if xds_config.enabled {
            info!(
                port = xds_config.port,
                server_id = %xds_config.server_id,
                "Starting xDS service for service mesh support"
            );
            match start_xds_service(xds_config, grpc_servers.naming_service.clone()).await {
                Ok(handle) => {
                    info!("xDS service started successfully");
                    Some(handle)
                }
                Err(e) => {
                    error!("Failed to start xDS service: {}", e);
                    None
                }
            }
        } else {
            None
        }
    };

    // Create Consul service adapters (only if enabled)
    // In cluster mode, Consul uses Raft for data replication across nodes.
    // In standalone mode, Consul uses independent RocksDB for persistence.
    let consul_services = if consul_enabled {
        let is_cluster = !app_state.configuration.is_standalone() && !is_console_remote;
        // Create unified health check registry for Consul
        let consul_registry = Arc::new(InstanceCheckRegistry::new(
            grpc_servers.naming_service.clone(),
        ));

        let services = if is_cluster {
            // Cluster mode: create a dedicated Consul Raft group
            let consul_node_id = app_state
                .raft_node
                .as_ref()
                .map(|r| r.node_id())
                .unwrap_or(1);
            let consul_node_addr = app_state.configuration.server_address();

            match batata_plugin_consul::raft::ConsulRaftNode::new(
                consul_node_id,
                consul_node_addr.clone(),
                &consul_data_dir_for_init,
            )
            .await
            {
                Ok((consul_raft_node, consul_db, consul_table_index)) => {
                    let consul_raft = Arc::new(consul_raft_node);
                    info!("Consul Raft node created (id={}, dir={})", consul_node_id, consul_data_dir_for_init);

                    // Activate the gRPC service (already listening on port 9849)
                    grpc_servers.consul_raft_grpc.set_raft_node(consul_raft.clone()).await;

                    // Initialize Consul Raft in background — don't block main startup.
                    // Other nodes may not be ready yet; Raft will retry automatically.
                    if let Some(ref nacos_raft) = app_state.raft_node {
                        let nacos_metrics = nacos_raft.metrics();
                        let local_raft_port = app_state.configuration.raft_port();
                        let consul_raft_port = app_state.configuration.consul_raft_port();

                        let members: std::collections::BTreeMap<u64, openraft::BasicNode> =
                            nacos_metrics
                                .membership_config
                                .membership()
                                .voter_ids()
                                .map(|id| {
                                    let nacos_addr = nacos_metrics
                                        .membership_config
                                        .membership()
                                        .get_node(&id)
                                        .map(|n| n.addr.clone())
                                        .unwrap_or_default();
                                    // Derive consul raft addr from nacos raft addr:
                                    // nacos_raft_port = main_port - 1000
                                    // consul_raft_port = consul_port - 1000
                                    // For node with nacos_raft=7848 (main=8848):
                                    //   consul_raft = consul_port - 1000 = 8500 - 1000 = 7500
                                    // For node with nacos_raft=7858 (main=8858):
                                    //   consul_raft = 8510 - 1000 = 7510
                                    // Pattern: consul_raft = nacos_raft - (main - consul) = nacos_raft - 348
                                    // But we don't know remote consul_port. Use: same offset as local.
                                    // Local: nacos_raft=local_raft_port, consul_raft=consul_raft_port
                                    // Remote: nacos_raft=nacos_rp, consul_raft=nacos_rp + (consul_raft_port - local_raft_port)
                                    let consul_addr = if let Some((host, port_str)) = nacos_addr.rsplit_once(':') {
                                        if let Ok(nacos_rp) = port_str.parse::<i32>() {
                                            let diff = consul_raft_port as i32 - local_raft_port as i32;
                                            let consul_rp = (nacos_rp + diff) as u16;
                                            format!("{}:{}", host, consul_rp)
                                        } else {
                                            format!("{}:{}", host, consul_raft_port)
                                        }
                                    } else {
                                        nacos_addr
                                    };
                                    (id, openraft::BasicNode { addr: consul_addr })
                                })
                                .collect();

                        if !members.is_empty() {
                            let consul_raft_bg = consul_raft.clone();
                            info!("Consul Raft members: {:?}", members);
                            tokio::spawn(async move {
                                // Wait briefly for other nodes' Consul Raft ports to bind
                                tokio::time::sleep(Duration::from_secs(3)).await;
                                if let Err(e) = consul_raft_bg.initialize(members).await {
                                    tracing::debug!("Consul Raft init: {} (may already be initialized)", e);
                                } else {
                                    info!("Consul Raft cluster initialized");
                                }
                            });
                        }
                    }

                    ConsulServices::with_consul_raft(
                        grpc_servers.naming_service.clone(),
                        consul_registry.clone(),
                        consul_acl_enabled,
                        consul_db,
                        consul_raft,
                        consul_table_index,
                        consul_dc_config.clone(),
                    )
                }
                Err(e) => {
                    error!("Failed to create Consul Raft: {}. Falling back to standalone.", e);
                    let consul_rocks_db = open_consul_rocks_db(
                        &consul_data_dir_for_init,
                        &app_state.configuration.rocksdb_config(),
                    );
                    if let Some(db) = consul_rocks_db {
                        ConsulServices::with_persistence(
                            grpc_servers.naming_service.clone(),
                            consul_registry.clone(),
                            consul_acl_enabled,
                            db,
                            consul_dc_config.clone(),
                        )
                    } else {
                        ConsulServices::new(
                            grpc_servers.naming_service.clone(),
                            consul_registry.clone(),
                            consul_acl_enabled,
                            consul_dc_config.clone(),
                        )
                    }
                }
            }
        } else if !is_console_remote {
            // Standalone mode: use independent RocksDB for Consul persistence
            let consul_rocks_db = open_consul_rocks_db(
                &consul_data_dir_for_init,
                &app_state.configuration.rocksdb_config(),
            );
            if let Some(db) = consul_rocks_db {
                info!("Consul services using RocksDB persistence");
                ConsulServices::with_persistence(
                    grpc_servers.naming_service.clone(),
                    consul_registry.clone(),
                    consul_acl_enabled,
                    db,
                    consul_dc_config.clone(),
                )
            } else {
                info!("Consul services using in-memory storage (no persistence)");
                ConsulServices::new(
                    grpc_servers.naming_service.clone(),
                    consul_registry.clone(),
                    consul_acl_enabled,
                    consul_dc_config.clone(),
                )
            }
        } else {
            // Console remote mode: in-memory
            info!("Console remote mode: Consul services using in-memory storage");
            ConsulServices::new(
                grpc_servers.naming_service.clone(),
                consul_registry.clone(),
                consul_acl_enabled,
                consul_dc_config.clone(),
            )
        };

        // Auto-register Consul service if enabled (only in non-remote mode)
        if consul_register_self_for_init && !is_console_remote {
            info!("Auto-registering Consul service...");
            let agent = services.agent.clone();
            let dc = consul_dc_config.datacenter.clone();
            tokio::spawn(async move {
                // Wait a bit to ensure naming service is ready
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                if let Err(e) = agent.register_consul_service(consul_server_port, &dc).await {
                    error!("Failed to auto-register Consul service: {}", e);
                }
            });
        }

        // Start TTL monitor (checks for expired TTL-based health checks)
        info!("Starting Consul TTL monitor...");
        let ttl_monitor = TtlMonitor::new(consul_registry.clone());
        tokio::spawn(async move {
            ttl_monitor.start().await;
        });

        // Start DeregisterMonitor (auto-deregisters instances in Critical state past threshold)
        info!("Starting Consul deregister monitor...");
        let deregister_monitor = DeregisterMonitor::new(
            consul_registry.clone(),
            grpc_servers.naming_service.clone(),
            30,
        );
        tokio::spawn(async move {
            deregister_monitor.start().await;
        });

        // Start session TTL cleanup task
        {
            let session_svc = services.session.clone();
            let kv_svc = services.kv.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    // Session cleanup: scan and remove expired sessions,
                    // release any KV locks they held
                    let expired = session_svc.scan_expired_session_ids();
                    if !expired.is_empty() {
                        info!("Cleaning up {} expired Consul sessions", expired.len());
                        for id in &expired {
                            kv_svc.release_session(id).await;
                        }
                        session_svc.cleanup_expired();
                    }
                }
            });
        }

        Some(services)
    } else {
        info!("Consul compatibility server is disabled");
        None
    };

    // Prepare distro protocol for HTTP server (only in cluster mode for distro sync)
    let distro_for_http = if !app_state.configuration.is_standalone() {
        Some(grpc_servers.distro_protocol.clone())
    } else {
        None
    };

    // Start HTTP servers based on deployment type with graceful shutdown support
    match deployment_type.as_str() {
        model::common::NACOS_DEPLOYMENT_TYPE_CONSOLE => {
            let console_server = startup::console_server(
                app_state.clone(),
                Some(grpc_servers.naming_service.clone()),
                ai_services.clone(),
                console_context_path,
                console_server_address,
                console_server_port,
            )?;

            tokio::select! {
                result = console_server => {
                    if let Err(e) = result {
                        error!("Console server error: {}", e);
                    }
                }
                _ = graceful_shutdown.wait_for_shutdown() => {
                    info!("Console server shutting down gracefully");
                }
            }
        }
        model::common::NACOS_DEPLOYMENT_TYPE_SERVER
        | model::common::NACOS_DEPLOYMENT_TYPE_SERVER_WITH_MCP => {
            let naming_service = grpc_servers.naming_service.clone();

            info!(
                "Starting Nacos main server on {}:{}",
                server_address, server_main_port
            );
            let main = startup::main_server(
                app_state.clone(),
                grpc_servers.naming_service,
                grpc_servers.connection_manager,
                grpc_servers.config_change_notifier.clone(),
                ai_services.clone(),
                distro_for_http.clone(),
                encryption_service.clone(),
                server_context_path,
                server_address,
                server_main_port,
            )?;

            let consul_opt = consul_services
                .map(|svc| {
                    info!(
                        "Starting Consul compatibility server on {}:{}",
                        consul_server_address, consul_server_port
                    );
                    startup::consul_server(
                        app_state.clone(),
                        naming_service.clone(),
                        svc,
                        consul_server_address.clone(),
                        consul_server_port,
                    )
                })
                .transpose()?;

            let mcp_registry_opt = if mcp_registry_enabled {
                info!(
                    "Starting MCP Registry server on {}:{}",
                    mcp_registry_address, mcp_registry_port
                );
                Some(startup::mcp_registry_server(
                    ai_services.mcp_registry.clone(),
                    mcp_registry_address,
                    mcp_registry_port,
                    app_state.configuration.http_access_log_enabled(),
                )?)
            } else {
                None
            };

            tokio::select! {
                result = async {
                    tokio::try_join!(
                        main,
                        async { match consul_opt { Some(s) => s.await, None => std::future::pending().await } },
                        async { match mcp_registry_opt { Some(s) => s.await, None => std::future::pending().await } }
                    )
                } => {
                    if let Err(e) = result {
                        error!("Server error: {}", e);
                    }
                }
                _ = graceful_shutdown.wait_for_shutdown() => {
                    info!("All servers shutting down gracefully");
                }
            }
        }
        _ => {
            let naming_service = grpc_servers.naming_service.clone();
            let mcp_registry_for_server = ai_services.mcp_registry.clone();

            // Start console, main, and optionally Consul servers
            info!(
                "Starting Console server on {}:{}",
                console_server_address, console_server_port
            );
            let console = startup::console_server(
                app_state.clone(),
                Some(naming_service.clone()),
                ai_services.clone(),
                console_context_path,
                console_server_address,
                console_server_port,
            )?;

            info!(
                "Starting Nacos main server on {}:{}",
                server_address, server_main_port
            );
            let main = startup::main_server(
                app_state.clone(),
                grpc_servers.naming_service,
                grpc_servers.connection_manager,
                grpc_servers.config_change_notifier,
                ai_services,
                distro_for_http,
                encryption_service,
                server_context_path,
                server_address,
                server_main_port,
            )?;

            let consul_opt = consul_services
                .map(|svc| {
                    info!(
                        "Starting Consul compatibility server on {}:{}",
                        consul_server_address, consul_server_port
                    );
                    startup::consul_server(
                        app_state.clone(),
                        naming_service.clone(),
                        svc,
                        consul_server_address.clone(),
                        consul_server_port,
                    )
                })
                .transpose()?;

            let mcp_registry_opt = if mcp_registry_enabled {
                info!(
                    "Starting MCP Registry server on {}:{}",
                    mcp_registry_address, mcp_registry_port
                );
                Some(startup::mcp_registry_server(
                    mcp_registry_for_server,
                    mcp_registry_address,
                    mcp_registry_port,
                    app_state.configuration.http_access_log_enabled(),
                )?)
            } else {
                None
            };

            tokio::select! {
                result = async {
                    tokio::try_join!(
                        console,
                        main,
                        async { match consul_opt { Some(s) => s.await, None => std::future::pending().await } },
                        async { match mcp_registry_opt { Some(s) => s.await, None => std::future::pending().await } }
                    )
                } => {
                    if let Err(e) = result {
                        error!("Server error: {}", e);
                    }
                }
                _ = graceful_shutdown.wait_for_shutdown() => {
                    info!("All servers shutting down gracefully");
                }
            }
        }
    }

    // ====================================================================
    // Graceful shutdown sequence
    // ====================================================================

    // 1. Transition to DRAINING: the TrafficReviseFilter will start
    //    rejecting NEW requests with 503 while in-flight requests complete.
    server_status.set_draining();
    let drain_timeout = graceful_shutdown.drain_timeout();
    info!(
        "Server status: DRAINING (waiting up to {}s for in-flight requests)...",
        drain_timeout.as_secs()
    );

    // 2. Brief pause to let in-flight requests finish.
    //    We cap the actual sleep at the configured drain timeout.
    let drain_sleep = drain_timeout.min(Duration::from_secs(5));
    tokio::time::sleep(drain_sleep).await;

    // 3. Mark server as DOWN — no more requests will be accepted.
    server_status.set_down();
    info!("Server status: DOWN");

    // 4. Stop xDS service if running
    if let Some(handle) = xds_handle {
        info!("Stopping xDS service...");
        handle.shutdown().await;
        info!("xDS service stopped");
    }

    // 5. Stop health check manager
    if app_state.health_check_manager.is_some() {
        info!("Health check manager will be stopped (background tasks cancelled on drop)");
    }

    // 6. Stop cluster manager if running (deregisters from cluster)
    if let Some(ref smm) = app_state.server_member_manager
        && !app_state.configuration.is_standalone()
    {
        info!("Stopping cluster manager...");
        smm.stop().await;
        info!("Cluster manager stopped");
    }

    // 7. Shutdown Raft node (flushes logs, transfers leadership if leader)
    if let Some(ref raft_node) = app_state.raft_node {
        info!("Shutting down Raft node...");
        match tokio::time::timeout(Duration::from_secs(10), raft_node.shutdown()).await {
            Ok(Ok(())) => info!("Raft node shutdown complete"),
            Ok(Err(e)) => error!("Raft node shutdown error: {}", e),
            Err(_) => error!("Raft node shutdown timed out after 10s"),
        }
    }

    // 8. Flush and close database connection
    if let Some(db) = database_connection {
        info!("Closing database connections...");
        match tokio::time::timeout(Duration::from_secs(5), db.close()).await {
            Ok(Ok(())) => info!("Database connections closed"),
            Ok(Err(e)) => error!("Error closing database connections: {}", e),
            Err(_) => error!("Database close timed out after 5s"),
        }
    }

    info!("Batata server shutdown complete");
    Ok(())
}

/// Open an independent RocksDB for Consul KV/Session/ACL storage (standalone mode).
///
/// Uses the RocksDbConfig from application configuration for consistent tuning
/// across all RocksDB instances. Falls back to in-memory on failure.
fn open_consul_rocks_db(
    data_dir: &str,
    rocks_config: &batata_server_common::model::config::RocksDbConfig,
) -> Option<Arc<rocksdb::DB>> {
    info!("Initializing Consul RocksDB persistence at: {}", data_dir);

    let db_opts = rocks_config.to_db_options();
    let cf_opts = rocks_config.to_cf_options();

    let consul_cfs = vec![
        ColumnFamilyDescriptor::new(CF_CONSUL_KV, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_ACL, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_SESSIONS, cf_opts.clone()),
        ColumnFamilyDescriptor::new(CF_CONSUL_QUERIES, cf_opts),
    ];

    match rocksdb::DB::open_cf_descriptors(&db_opts, data_dir, consul_cfs) {
        Ok(db) => {
            info!("Consul RocksDB initialized successfully");
            Some(Arc::new(db))
        }
        Err(e) => {
            error!(
                "Failed to initialize Consul RocksDB: {}, falling back to in-memory",
                e
            );
            None
        }
    }
}
