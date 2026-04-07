//! Main entry point for Batata Nacos-compatible server.
//!
//! This file orchestrates server startup by delegating to specialized modules
//! for persistence, cluster, consul, and HTTP/gRPC server initialization.

use std::sync::Arc;
use std::time::Duration;

use batata_auth::service::oauth::OAuthService;
use batata_naming::healthcheck::{HealthCheckConfig, HealthCheckManager};
use batata_plugin::{Plugin as _, ProtocolAdapterPlugin as _};
use batata_server::{
    middleware::rate_limit,
    model::{self, common::AppState},
    startup::{self, AIServices, GracefulShutdown, OtelConfig, XdsServerHandle, start_xds_service},
};
use batata_server_common::ServerStatusManager;
use tracing::{error, info};

#[allow(clippy::type_complexity)]
#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ====================================================================
    // Phase 1: Configuration and logging
    // ====================================================================
    let configuration = model::common::Configuration::new()?;
    validate_config(&configuration);

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

    batata_server::metrics::init_metrics();
    let _system_stats_handle = batata_server::metrics::start_system_stats_reporter(None);
    let _rate_limit_cleanup_handle = rate_limit::start_cleanup_task();

    // Initialize auth caches with configuration values
    batata_auth::service::auth::init_auth_caches(batata_auth::service::auth::AuthCacheConfig {
        token_capacity: configuration.auth_token_cache_capacity(),
        token_ttl_secs: configuration.auth_token_cache_ttl_secs(),
        blacklist_capacity: configuration.auth_blacklist_capacity(),
        blacklist_ttl_secs: configuration.auth_blacklist_ttl_secs(),
    });
    batata_core::service::grpc_auth::init_grpc_auth_cache(
        configuration.grpc_auth_cache_capacity(),
        configuration.grpc_auth_cache_ttl_secs(),
    );

    // ====================================================================
    // Phase 2: Determine deployment mode
    // ====================================================================
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

    // ====================================================================
    // Phase 2.5: Register protocol adapter plugins (lightweight, config only)
    // ====================================================================
    let mut plugin_manager = batata_plugin::spi::PluginManager::new();
    let mut consul_plugin_ref: Option<Arc<batata_plugin_consul::ConsulPlugin>> = None;

    let consul_config = batata_plugin_consul::ConsulPluginConfig::from_config(&configuration.config);
    let consul_plugin = Arc::new(
        batata_plugin_consul::ConsulPlugin::from_plugin_config(consul_config),
    );
    if consul_plugin.is_enabled() {
        plugin_manager.register_protocol_adapter(consul_plugin.clone());
        consul_plugin_ref = Some(consul_plugin.clone());
    }

    // Collect plugin CFs before creating RocksDB
    let plugin_cf_names = plugin_manager.collect_plugin_column_families();

    // ====================================================================
    // Phase 3: Initialize persistence layer
    // ====================================================================
    let persistence_ctx = if is_console_remote {
        info!("Starting in console remote mode - connecting to remote server");
        startup::persistence::PersistenceContext::empty()
    } else {
        startup::persistence::init_persistence(&configuration, &plugin_cf_names).await?
    };

    // ====================================================================
    // Phase 4: Create shared services and application state
    // ====================================================================
    let config_subscriber_manager: Arc<dyn batata_common::ConfigSubscriptionService> =
        Arc::new(batata_core::ConfigSubscriberManager::new());

    let naming_service_concrete: Option<Arc<batata_naming::NamingService>> = if !is_console_remote {
        Some(Arc::new(batata_naming::NamingService::new()))
    } else {
        None
    };
    let naming_service: Option<Arc<dyn batata_api::naming::NamingServiceProvider>> =
        naming_service_concrete
            .clone()
            .map(|ns| ns as Arc<dyn batata_api::naming::NamingServiceProvider>);

    // Build plugin state providers for server state API.
    // Always include consul plugin so state API returns consul_enabled even when disabled.
    let plugin_state_providers: Vec<Arc<dyn batata_plugin::PluginStateProvider>> =
        vec![consul_plugin.clone() as Arc<dyn batata_plugin::PluginStateProvider>];

    let console_datasource = batata_console::create_datasource(
        &configuration,
        persistence_ctx.cluster_manager.clone(),
        config_subscriber_manager.clone(),
        naming_service.clone(),
        persistence_ctx.persistence.clone(),
        plugin_state_providers.clone(),
    )
    .await?;

    let oauth_service = init_oauth_service(&configuration)?;

    let health_check_manager: Option<Arc<HealthCheckManager>> =
        if let Some(ref ns) = naming_service_concrete {
            let health_check_config = Arc::new(HealthCheckConfig {
                heartbeat_interval_secs: configuration.naming_heartbeat_check_interval_secs(),
                ttl_monitor_interval_secs: configuration.naming_ttl_monitor_interval_secs(),
                deregister_monitor_interval_secs: configuration
                    .naming_deregister_monitor_interval_secs(),
                ..HealthCheckConfig::default()
            });
            let expire_enabled = configuration.expire_instance_enabled();
            let health_check_enabled = health_check_config.is_enabled();
            let manager = Arc::new(HealthCheckManager::new(
                ns.clone(),
                health_check_config,
                expire_enabled,
                None,
                None,
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

    let server_status = Arc::new(ServerStatusManager::new());

    let control_plugin = init_control_plugin(&configuration).await;

    let encryption_service = init_encryption_service(&configuration);

    let auth_plugin: Option<Arc<dyn batata_common::AuthPlugin>> =
        persistence_ctx.persistence.as_ref().map(|p| {
            let ldap_config = if configuration.auth_system_type() == "ldap" {
                Some(configuration.ldap_config())
            } else {
                None
            };
            batata_auth::plugin::create_auth_plugin(
                &configuration.auth_system_type(),
                configuration.token_secret_key(),
                configuration.auth_token_expire_seconds(),
                p.clone(),
                ldap_config,
            )
        });

    let app_state = Arc::new(AppState {
        configuration,
        cluster_manager: persistence_ctx.cluster_manager.clone(),
        config_subscriber_manager: config_subscriber_manager.clone(),
        console_datasource,
        oauth_service,
        auth_plugin,
        persistence: persistence_ctx.persistence.clone(),
        health_check_manager: health_check_manager
            .clone()
            .map(|m| m as Arc<dyn batata_common::HeartbeatService>),
        raft_node: persistence_ctx.raft_node.clone(),
        server_status: server_status.clone(),
        control_plugin,
        encryption_service: Some(
            encryption_service.clone() as Arc<dyn batata_common::ConfigEncryptionProvider>
        ),
        plugin_state_providers,
    });

    if !app_state.configuration.data_warmup() {
        server_status.set_up();
        info!("Server status: UP (data warmup disabled)");
    } else {
        info!("Server status: STARTING (data warmup enabled, waiting for subsystems)");
    }

    // ====================================================================
    // Phase 5: Initialize shutdown handler and AI services
    // ====================================================================
    let drain_timeout_secs = app_state.configuration.shutdown_drain_timeout_secs();
    let shutdown_signal = startup::wait_for_shutdown_signal().await;
    let graceful_shutdown = GracefulShutdown::new(
        shutdown_signal.clone(),
        Duration::from_secs(drain_timeout_secs),
    );

    let ai_services = match (&app_state.persistence, &naming_service) {
        (Some(persist), Some(ns)) => {
            info!("AI services using config-backed persistence");
            AIServices::with_persistence(persist.clone(), ns.clone())
                .with_ai_resource_services(persist.clone())
                .with_copilot(persist.clone())
        }
        _ => {
            info!("AI services using in-memory storage (no persistence)");
            AIServices::new()
        }
    };

    // ====================================================================
    // Phase 6: Console-remote early return
    // ====================================================================
    if is_console_remote {
        return run_console_remote(
            app_state,
            ai_services,
            graceful_shutdown,
            server_status,
            persistence_ctx.database_connection,
        )
        .await;
    }

    // ====================================================================
    // Phase 7: Start gRPC servers
    // ====================================================================
    let grpc_servers = startup::start_grpc_servers(
        app_state.clone(),
        naming_service_concrete.clone(),
        &ai_services,
        app_state.configuration.sdk_server_port(),
        app_state.configuration.cluster_server_port(),
        persistence_ctx.raft_node.clone(),
        persistence_ctx.server_member_manager.clone(),
        config_subscriber_manager.clone(),
    )?;

    // Server registry for per-server health aggregation
    let server_registry = Arc::new(batata_core::ServerRegistry::new());
    server_registry.register(
        "SDK gRPC Server",
        batata_core::ServerType::Grpc,
        grpc_servers.sdk_state.clone(),
    );
    server_registry.register(
        "Cluster gRPC Server",
        batata_core::ServerType::Grpc,
        grpc_servers.cluster_state.clone(),
    );
    if persistence_ctx.raft_node.is_some() {
        server_registry.register(
            "Raft Server",
            batata_core::ServerType::Raft,
            grpc_servers.raft_state.clone(),
        );
    }

    // ====================================================================
    // Phase 8: Start background tasks
    // ====================================================================

    // Upgrade health check to cluster mode if not standalone.
    // DistroProtocol (and its DistroMapper) is now available from gRPC servers.
    if !app_state.configuration.is_standalone()
        && let Some(ref hc_manager) = health_check_manager
    {
        let distro = grpc_servers.distro_protocol();
        hc_manager.upgrade_to_cluster(
            Arc::new(HealthCheckConfig {
                heartbeat_interval_secs: app_state
                    .configuration
                    .naming_heartbeat_check_interval_secs(),
                ttl_monitor_interval_secs: app_state
                    .configuration
                    .naming_ttl_monitor_interval_secs(),
                deregister_monitor_interval_secs: app_state
                    .configuration
                    .naming_deregister_monitor_interval_secs(),
                ..HealthCheckConfig::default()
            }),
            distro.mapper().clone(),
            distro.local_address().to_string(),
            distro.clone(),
        );
        info!(
            "Health check upgraded to cluster mode (local_address={})",
            distro.local_address()
        );
    }

    start_health_checkers(&health_check_manager);
    start_warmup_poller(
        &app_state,
        &server_status,
        persistence_ctx.server_member_manager.as_ref(),
    );
    start_mcp_index_refresh(&ai_services, &app_state);

    // ====================================================================
    // Phase 9: Cluster and Raft initialization
    // ====================================================================
    if let Some(ref smm) = persistence_ctx.server_member_manager {
        startup::cluster::init_cluster(
            &app_state.configuration,
            smm,
            persistence_ctx.raft_node.as_ref(),
            &grpc_servers,
        )
        .await?;
    }

    // ====================================================================
    // Phase 10: xDS service mesh support
    // ====================================================================
    let xds_handle = start_xds_if_enabled(&app_state, &grpc_servers).await;

    // ====================================================================
    // Phase 11: Initialize protocol adapter plugins
    // ====================================================================
    // (Plugins were registered in Phase 2.5 before persistence init)

    // Build plugin context
    let mut plugin_ctx = batata_plugin::PluginContext::new();
    let is_cluster = !app_state.configuration.is_standalone() && !is_console_remote;
    plugin_ctx.insert("is_cluster", Arc::new(is_cluster));
    if let Some(ref raft) = app_state.raft_node {
        plugin_ctx.insert("raft_node", raft.clone());
    }

    // Initialize all plugins
    plugin_manager.init_protocol_adapters(&plugin_ctx).await?;

    // Wire Consul event broadcast (cluster mode only)
    if let Some(ref consul_plugin) = consul_plugin_ref {
        // Register gRPC handler for receiving event broadcasts from peers
        let event_service = consul_plugin.event_service().clone();
        grpc_servers.handler_registry().register_handler(Arc::new(
            batata_server::service::consul_event_handler::ConsulEventBroadcastHandler {
                event_service: event_service.clone(),
            },
        ));

        // Set broadcaster for sending events to peers
        if let Some(ref client_manager) = *grpc_servers.cluster_client_manager()
            && let Some(ref smm) = persistence_ctx.server_member_manager
        {
            let broadcaster = Arc::new(
                batata_server::service::consul_event_handler::ConsulEventBroadcasterImpl::new(
                    client_manager.clone(),
                    smm.clone(),
                ),
            );
            event_service.set_broadcaster(broadcaster).await;
            info!("Consul event cluster broadcast enabled");
        }
    }

    // ====================================================================
    // Phase 12: Start HTTP servers and wait for shutdown
    // ====================================================================
    let distro_for_http = if !app_state.configuration.is_standalone() {
        Some(grpc_servers.distro_protocol().clone())
    } else {
        None
    };
    let naming_provider_for_http: Arc<dyn batata_api::naming::NamingServiceProvider> =
        grpc_servers.naming_provider();

    run_http_servers(
        &deployment_type,
        &app_state,
        naming_provider_for_http,
        &grpc_servers,
        ai_services,
        distro_for_http,
        encryption_service,
        &plugin_manager,
        &graceful_shutdown,
        &server_registry,
    )
    .await?;

    // ====================================================================
    // Phase 13: Graceful shutdown sequence
    // ====================================================================
    graceful_shutdown_sequence(
        &server_status,
        &graceful_shutdown,
        &app_state,
        &grpc_servers,
        xds_handle,
        persistence_ctx.server_member_manager.as_ref(),
        persistence_ctx.database_connection,
        &server_registry,
    )
    .await;

    info!("Batata server shutdown complete");
    Ok(())
}

// ============================================================================
// Phase helpers
// ============================================================================

/// Validate configuration and exit on fatal errors.
fn validate_config(configuration: &model::common::Configuration) {
    if configuration.auth_enabled() && configuration.token_secret_key().is_empty() {
        eprintln!(
            "FATAL: Authentication is enabled (batata.core.auth.enabled=true) but no JWT secret key is configured."
        );
        eprintln!(
            "Set 'batata.core.auth.plugin.nacos.token.secret.key' to a non-empty Base64-encoded secret."
        );
        std::process::exit(1);
    }

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
}

/// Initialize OAuth service if enabled.
fn init_oauth_service(
    configuration: &model::common::Configuration,
) -> Result<Option<Arc<dyn batata_common::OAuthProvider>>, Box<dyn std::error::Error>> {
    if configuration.is_oauth_enabled() {
        let oauth_config = configuration.oauth_config();
        let oauth_cache_config = batata_auth::service::oauth::OAuthCacheConfig {
            discovery_ttl_secs: configuration.oauth_discovery_cache_ttl_secs(),
            discovery_capacity: configuration.oauth_discovery_cache_capacity(),
            state_ttl_secs: configuration.oauth_state_cache_ttl_secs(),
            state_capacity: configuration.oauth_state_cache_capacity(),
            http_timeout_secs: configuration.oauth_http_timeout_secs(),
        };
        info!(
            "OAuth2/OIDC authentication enabled with {} providers",
            oauth_config.providers.len()
        );
        let service = Arc::new(OAuthService::with_cache_config(
            oauth_config,
            oauth_cache_config,
        )?);
        Ok(Some(service as Arc<dyn batata_common::OAuthProvider>))
    } else {
        Ok(None)
    }
}

/// Initialize control plugin for TPS rate limiting and connection control.
async fn init_control_plugin(
    configuration: &model::common::Configuration,
) -> Option<Arc<dyn batata_plugin::ControlPlugin>> {
    let control_config = configuration.control_plugin_config();
    if !control_config.enabled {
        info!("Control plugin disabled by configuration");
        return None;
    }

    let plugin = batata_plugin::DefaultControlPlugin::new(control_config);
    if let Err(e) = plugin.init().await {
        tracing::warn!("Failed to initialize control plugin: {}", e);
        return None;
    }

    info!(
        "Control plugin initialized (TPS limit={}, max connections={})",
        configuration.control_plugin_default_tps(),
        configuration.control_plugin_max_connections()
    );
    Some(Arc::new(plugin))
}

/// Initialize config encryption service.
fn init_encryption_service(
    configuration: &model::common::Configuration,
) -> Arc<batata_config::service::encryption::ConfigEncryptionService> {
    if configuration.encryption_enabled() {
        match configuration.encryption_key() {
            Some(key) => {
                match batata_config::service::encryption::ConfigEncryptionService::new(&key) {
                    Ok(svc) => {
                        info!("Config encryption enabled");
                        return Arc::new(svc);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create encryption service: {}", e);
                    }
                }
            }
            None => {
                tracing::warn!("Encryption enabled but no key configured");
            }
        }
    }
    Arc::new(batata_config::service::encryption::ConfigEncryptionService::disabled())
}

/// Start health check background tasks.
fn start_health_checkers(health_check_manager: &Option<Arc<HealthCheckManager>>) {
    if let Some(hc_manager) = health_check_manager {
        let hc1 = hc_manager.clone();
        tokio::spawn(async move { hc1.unhealthy_checker().start().await });

        let hc2 = hc_manager.clone();
        tokio::spawn(async move { hc2.expired_checker().start().await });

        info!("Health check manager started (unhealthy/expired instance checkers)");
    }
}

/// Start data warmup poller if enabled.
///
/// In cluster mode, the poller also gates readiness on the distro protocol
/// having completed its initial data load — preventing clients from connecting
/// to a node that has no naming data yet.
fn start_warmup_poller(
    app_state: &Arc<AppState>,
    server_status: &Arc<ServerStatusManager>,
    server_member_manager: Option<&Arc<batata_core::cluster::ServerMemberManager>>,
) {
    if !app_state.configuration.data_warmup() {
        return;
    }

    let status_mgr = server_status.clone();
    let console_ds = app_state.console_datasource.clone();
    let raft_ref = app_state.raft_node.clone();
    let is_standalone = app_state.configuration.is_standalone();
    let smm_clone = server_member_manager.cloned();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            let db_ready = console_ds.server_readiness().await;
            let (raft_ready, raft_reason) = match &raft_ref {
                Some(raft) => raft.is_ready(),
                None => (true, None),
            };

            // In cluster mode, also check that distro protocol has loaded initial data.
            // This prevents clients from seeing empty naming data on newly joined nodes.
            let distro_ready = if is_standalone {
                true
            } else if let Some(ref smm) = smm_clone {
                smm.is_distro_initialized().await
            } else {
                true
            };

            if db_ready && raft_ready && distro_ready {
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
                if !distro_ready {
                    reasons.push("distro initial data not loaded".to_string());
                }
                let msg = reasons.join(", ");
                status_mgr.set_down();
                status_mgr.set_error_msg(Some(msg)).await;
            }
        }
    });
    info!("Data warmup poller started (checking every 5s)");
}

/// Start periodic MCP index refresh if persistence is available.
fn start_mcp_index_refresh(ai_services: &AIServices, app_state: &Arc<AppState>) {
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
}

/// Start xDS service if enabled.
async fn start_xds_if_enabled(
    app_state: &Arc<AppState>,
    grpc_servers: &startup::GrpcServers,
) -> Option<XdsServerHandle> {
    let xds_config = app_state.configuration.xds_config();
    if !xds_config.enabled {
        return None;
    }

    info!(
        port = xds_config.port,
        server_id = %xds_config.server_id,
        "Starting xDS service for service mesh support"
    );
    match start_xds_service(xds_config, grpc_servers.naming_provider()).await {
        Ok(handle) => {
            info!("xDS service started successfully");
            Some(handle)
        }
        Err(e) => {
            error!("Failed to start xDS service: {}", e);
            None
        }
    }
}

/// Run in console-remote mode (only console HTTP server).
async fn run_console_remote(
    app_state: Arc<AppState>,
    ai_services: AIServices,
    graceful_shutdown: GracefulShutdown,
    server_status: Arc<ServerStatusManager>,
    database_connection: Option<sea_orm::DatabaseConnection>,
) -> Result<(), Box<dyn std::error::Error>> {
    let console_server_port = app_state.configuration.console_server_port();
    let console_context_path = app_state.configuration.console_server_context_path();
    let console_server_address = app_state.configuration.server_address();

    info!(
        "Starting console server in remote mode on port {}",
        console_server_port
    );

    let console_server = startup::console_server(
        app_state,
        None,
        ai_services,
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

    // Minimal shutdown for console-remote mode
    server_status.set_down();
    if let Some(db) = database_connection {
        let _ = tokio::time::timeout(Duration::from_secs(5), db.close()).await;
    }

    Ok(())
}

/// Start HTTP servers based on deployment type and wait for shutdown signal.
#[allow(clippy::too_many_arguments)]
async fn run_http_servers(
    deployment_type: &str,
    app_state: &Arc<AppState>,
    naming_provider: Arc<dyn batata_api::naming::NamingServiceProvider>,
    grpc_servers: &startup::GrpcServers,
    ai_services: AIServices,
    distro_for_http: Option<Arc<batata_core::service::distro::DistroProtocol>>,
    encryption_service: Arc<batata_config::service::encryption::ConfigEncryptionService>,
    plugin_manager: &batata_plugin::spi::PluginManager,
    graceful_shutdown: &GracefulShutdown,
    server_registry: &Arc<batata_core::ServerRegistry>,
) -> Result<(), Box<dyn std::error::Error>> {
    let server_address = app_state.configuration.server_address();
    let server_main_port = app_state.configuration.server_main_port();
    let server_context_path = app_state.configuration.server_context_path();
    let mcp_registry_enabled = app_state.configuration.mcp_registry_enabled()
        || deployment_type == model::common::NACOS_DEPLOYMENT_TYPE_SERVER_WITH_MCP;
    let mcp_registry_port = app_state.configuration.mcp_registry_port();

    // Register HTTP server state trackers
    let main_http_state = batata_core::ServerStateTracker::new();
    let console_http_state = batata_core::ServerStateTracker::new();

    match deployment_type {
        model::common::NACOS_DEPLOYMENT_TYPE_CONSOLE => {
            let console_server_port = app_state.configuration.console_server_port();
            let console_context_path = app_state.configuration.console_server_context_path();
            let console = startup::console_server(
                app_state.clone(),
                Some(naming_provider),
                ai_services,
                console_context_path,
                server_address,
                console_server_port,
            )?;

            tokio::select! {
                result = console => {
                    if let Err(e) = result { error!("Console server error: {}", e); }
                }
                _ = graceful_shutdown.wait_for_shutdown() => {
                    info!("Console server shutting down gracefully");
                }
            }
        }
        model::common::NACOS_DEPLOYMENT_TYPE_SERVER
        | model::common::NACOS_DEPLOYMENT_TYPE_SERVER_WITH_MCP => {
            info!(
                "Starting Nacos main server on {}:{}",
                server_address, server_main_port
            );
            let mcp_registry_for_server = ai_services.mcp_registry.clone();
            let main = startup::main_server(
                app_state.clone(),
                naming_provider,
                grpc_servers.connection_manager().clone(),
                grpc_servers.config_change_notifier().clone(),
                ai_services,
                distro_for_http,
                encryption_service,
                server_context_path,
                server_address.clone(),
                server_main_port,
                Some(server_registry.clone()),
            )?;
            main_http_state.set_running();
            server_registry.register(
                "Main HTTP Server",
                batata_core::ServerType::Http,
                main_http_state.clone(),
            );

            let plugin_servers = build_plugin_servers(app_state, grpc_servers, plugin_manager)?;
            let mcp_registry_opt = build_mcp_registry(
                app_state,
                &mcp_registry_for_server,
                mcp_registry_enabled,
                &server_address,
                mcp_registry_port,
            )?;

            // Start background tasks for all protocol adapter plugins
            plugin_manager.start_protocol_adapter_tasks().await?;

            // Spawn plugin servers as background tasks
            for server in plugin_servers {
                tokio::spawn(async move {
                    if let Err(e) = server.await {
                        error!("Plugin server error: {}", e);
                    }
                });
            }

            tokio::select! {
                result = async {
                    tokio::try_join!(
                        main,
                        async { match mcp_registry_opt { Some(s) => s.await, None => std::future::pending().await } }
                    )
                } => {
                    if let Err(e) = result { error!("Server error: {}", e); }
                }
                _ = graceful_shutdown.wait_for_shutdown() => {
                    info!("All servers shutting down gracefully");
                }
            }
        }
        _ => {
            // Merged mode: console + main + optional plugin servers + optional MCP
            let console_server_port = app_state.configuration.console_server_port();
            let console_context_path = app_state.configuration.console_server_context_path();

            info!(
                "Starting Console server on {}:{}",
                server_address, console_server_port
            );
            let console = startup::console_server(
                app_state.clone(),
                Some(naming_provider.clone()),
                ai_services.clone(),
                console_context_path,
                server_address.clone(),
                console_server_port,
            )?;
            console_http_state.set_running();
            server_registry.register(
                "Console HTTP Server",
                batata_core::ServerType::Http,
                console_http_state.clone(),
            );

            info!(
                "Starting Nacos main server on {}:{}",
                server_address, server_main_port
            );
            let main = startup::main_server(
                app_state.clone(),
                naming_provider,
                grpc_servers.connection_manager().clone(),
                grpc_servers.config_change_notifier().clone(),
                ai_services.clone(),
                distro_for_http,
                encryption_service,
                server_context_path,
                server_address.clone(),
                server_main_port,
                Some(server_registry.clone()),
            )?;
            main_http_state.set_running();
            server_registry.register(
                "Main HTTP Server",
                batata_core::ServerType::Http,
                main_http_state.clone(),
            );

            let plugin_servers = build_plugin_servers(app_state, grpc_servers, plugin_manager)?;
            let mcp_registry_opt = build_mcp_registry(
                app_state,
                &ai_services.mcp_registry,
                mcp_registry_enabled,
                &server_address,
                mcp_registry_port,
            )?;

            // Start background tasks for all protocol adapter plugins
            plugin_manager.start_protocol_adapter_tasks().await?;

            // Spawn plugin servers as background tasks
            for server in plugin_servers {
                tokio::spawn(async move {
                    if let Err(e) = server.await {
                        error!("Plugin server error: {}", e);
                    }
                });
            }

            tokio::select! {
                result = async {
                    tokio::try_join!(
                        console,
                        main,
                        async { match mcp_registry_opt { Some(s) => s.await, None => std::future::pending().await } }
                    )
                } => {
                    if let Err(e) = result { error!("Server error: {}", e); }
                }
                _ = graceful_shutdown.wait_for_shutdown() => {
                    info!("All servers shutting down gracefully");
                }
            }
        }
    }

    Ok(())
}

/// Build HTTP servers for all enabled protocol adapter plugins.
fn build_plugin_servers(
    app_state: &Arc<AppState>,
    grpc_servers: &startup::GrpcServers,
    plugin_manager: &batata_plugin::spi::PluginManager,
) -> Result<Vec<actix_web::dev::Server>, Box<dyn std::error::Error>> {
    let mut servers = Vec::new();
    let server_address = app_state.configuration.server_address();
    for plugin in plugin_manager.protocol_adapters() {
        if !plugin.is_enabled() {
            continue;
        }
        let port = plugin.default_port();
        info!(
            "Starting {} server on {}:{}",
            plugin.name(),
            server_address,
            port
        );
        let server = startup::plugin_http_server(
            app_state.clone(),
            grpc_servers.naming_provider(),
            plugin.clone(),
            server_address.clone(),
            port,
        )?;
        servers.push(server);
    }
    Ok(servers)
}

/// Build MCP registry server if enabled.
fn build_mcp_registry(
    app_state: &Arc<AppState>,
    mcp_registry: &Arc<batata_server::api::ai::McpServerRegistry>,
    mcp_registry_enabled: bool,
    address: &str,
    port: u16,
) -> Result<Option<actix_web::dev::Server>, Box<dyn std::error::Error>> {
    if !mcp_registry_enabled {
        return Ok(None);
    }
    info!("Starting MCP Registry server on {}:{}", address, port);
    Ok(Some(startup::mcp_registry_server(
        mcp_registry.clone(),
        address.to_string(),
        port,
        app_state.configuration.http_access_log_enabled(),
    )?))
}

/// Execute graceful shutdown sequence with per-server state tracking.
///
/// Shutdown order (reverse of startup priority):
/// 1. HTTP servers → DRAINING (reject new requests with 503)
/// 2. gRPC servers → DRAINING → STOPPED
/// 3. Drain pause for in-flight requests
/// 4. xDS, health check, cluster manager → STOPPED
/// 5. Raft node → STOPPED
/// 6. Database connection → closed
#[allow(clippy::too_many_arguments)]
async fn graceful_shutdown_sequence(
    server_status: &Arc<ServerStatusManager>,
    graceful_shutdown: &GracefulShutdown,
    app_state: &Arc<AppState>,
    grpc_servers: &startup::GrpcServers,
    xds_handle: Option<XdsServerHandle>,
    server_member_manager: Option<&Arc<batata_core::cluster::ServerMemberManager>>,
    database_connection: Option<sea_orm::DatabaseConnection>,
    server_registry: &Arc<batata_core::ServerRegistry>,
) {
    // Log per-server states before shutdown
    let states = server_registry.health();
    info!(
        "Starting graceful shutdown ({} servers registered): {:?}",
        states.len(),
        states
            .iter()
            .map(|s| format!("{}={}", s.name, s.state))
            .collect::<Vec<_>>()
    );

    // 1. DRAINING: reject new requests with 503
    server_status.set_draining();
    let drain_timeout = graceful_shutdown.drain_timeout();
    info!(
        "Server status: DRAINING (waiting up to {}s for in-flight requests)...",
        drain_timeout.as_secs()
    );

    // 2. Signal gRPC servers to stop accepting new connections
    grpc_servers.shutdown();
    info!("gRPC servers signaled to shut down");

    // 3. Brief pause for in-flight requests
    let drain_sleep = drain_timeout.min(Duration::from_secs(5));
    tokio::time::sleep(drain_sleep).await;

    // 4. DOWN
    server_status.set_down();
    info!("Server status: DOWN");

    // 5. Stop xDS
    if let Some(handle) = xds_handle {
        info!("Stopping xDS service...");
        handle.shutdown().await;
        info!("xDS service stopped");
    }

    // 6. Health check manager stopped on drop
    if app_state.health_check_manager.is_some() {
        info!("Health check manager will be stopped (background tasks cancelled on drop)");
    }

    // 7. Stop cluster manager
    if let Some(smm) = server_member_manager
        && !app_state.configuration.is_standalone()
    {
        info!("Stopping cluster manager...");
        smm.stop().await;
        info!("Cluster manager stopped");
    }

    // 8. Shutdown Raft node
    if let Some(ref raft_node) = app_state.raft_node {
        info!("Shutting down Raft node...");
        match tokio::time::timeout(Duration::from_secs(10), raft_node.shutdown()).await {
            Ok(Ok(())) => info!("Raft node shutdown complete"),
            Ok(Err(e)) => error!("Raft node shutdown error: {}", e),
            Err(_) => error!("Raft node shutdown timed out after 10s"),
        }
    }

    // 9. Close database connection
    if let Some(db) = database_connection {
        let db_timeout = app_state.configuration.shutdown_db_close_timeout_secs();
        info!("Closing database connections (timeout: {}s)...", db_timeout);
        match tokio::time::timeout(Duration::from_secs(db_timeout), db.close()).await {
            Ok(Ok(())) => info!("Database connections closed"),
            Ok(Err(e)) => error!("Error closing database connections: {}", e),
            Err(_) => error!("Database close timed out after {}s", db_timeout),
        }
    }

    // Log final per-server states
    let final_states = server_registry.health();
    info!(
        "Shutdown complete ({} servers): {:?}",
        final_states.len(),
        final_states
            .iter()
            .map(|s| format!("{}={}", s.name, s.state))
            .collect::<Vec<_>>()
    );
}
