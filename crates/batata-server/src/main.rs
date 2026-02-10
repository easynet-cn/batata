//! Main entry point for Batata Nacos-compatible server.
//!
//! This file sets up and starts the HTTP and gRPC servers with their respective services.

use std::sync::Arc;
use std::time::Duration;

use batata_auth::service::oauth::OAuthService;
use batata_core::cluster::ServerMemberManager;
use batata_persistence::{PersistenceService, StorageMode};
use batata_server::{
    console::datasource,
    middleware::rate_limit,
    model::{self, common::AppState},
    startup::{
        self, AIServices, ApolloServices, ConsulServices, GracefulShutdown, OtelConfig,
        XdsServerHandle, start_xds_service,
    },
};
use tracing::{error, info};

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize configuration and logging
    let configuration = model::common::Configuration::new();

    // Initialize multi-file logging with optional OpenTelemetry support
    let logging_config = configuration.logging_config();
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
    let is_console_remote = deployment_type == model::common::NACOS_DEPLOYMENT_TYPE_CONSOLE
        && configuration.is_console_remote_mode();

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
    let apollo_enabled = configuration.apollo_enabled();
    let apollo_server_port = configuration.apollo_server_port();
    let apollo_server_address = server_address.clone();

    // Initialize database, persistence service, and server member manager based on deployment mode
    let storage_mode = configuration.persistence_mode();
    info!("Persistence mode: {}", storage_mode);

    let (database_connection, server_member_manager, persistence, rocks_db): (
        Option<sea_orm::DatabaseConnection>,
        Option<Arc<ServerMemberManager>>,
        Option<Arc<dyn PersistenceService>>,
        Option<Arc<rocksdb::DB>>,
    ) = if is_console_remote {
        info!("Starting in console remote mode - connecting to remote server");
        (None, None, None, None)
    } else {
        match storage_mode {
            StorageMode::ExternalDb => {
                let db = configuration.database_connection().await?;
                let core_config = configuration.to_core_config();
                let smm = Arc::new(ServerMemberManager::new(&core_config));
                let persist: Arc<dyn PersistenceService> = Arc::new(
                    batata_persistence::ExternalDbPersistService::new(db.clone()),
                );
                (Some(db), Some(smm), Some(persist), None)
            }
            StorageMode::StandaloneEmbedded => {
                let data_dir = configuration.embedded_data_dir();
                info!("Initializing standalone embedded storage at: {}", data_dir);
                let sm = batata_consistency::RocksStateMachine::new(&data_dir)
                    .await
                    .map_err(|e| format!("Failed to initialize RocksDB state machine: {}", e))?;
                let rdb = sm.db();
                let persist: Arc<dyn PersistenceService> =
                    Arc::new(batata_persistence::EmbeddedPersistService::from_state_machine(&sm));
                let core_config = configuration.to_core_config();
                let smm = Arc::new(ServerMemberManager::new(&core_config));
                (None, Some(smm), Some(persist), Some(rdb))
            }
            StorageMode::DistributedEmbedded => {
                // Distributed mode requires Raft setup which is handled by the cluster manager.
                // For now, initialize the DB connection as fallback until full Raft integration is done.
                let db = configuration.database_connection().await?;
                let core_config = configuration.to_core_config();
                let smm = Arc::new(ServerMemberManager::new(&core_config));
                let persist: Arc<dyn PersistenceService> = Arc::new(
                    batata_persistence::ExternalDbPersistService::new(db.clone()),
                );
                info!(
                    "Distributed embedded mode: using external DB as fallback until Raft cluster is initialized"
                );
                (Some(db), Some(smm), Some(persist), None)
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

    let console_datasource = datasource::create_datasource(
        &configuration,
        database_connection.clone(),
        server_member_manager.clone(),
        config_subscriber_manager.clone(),
        naming_service.clone(),
    )
    .await?;

    // Initialize OAuth service if enabled
    let oauth_service = if configuration.is_oauth_enabled() {
        let oauth_config = configuration.oauth_config();
        info!(
            "OAuth2/OIDC authentication enabled with {} providers",
            oauth_config.providers.len()
        );
        Some(Arc::new(OAuthService::new(oauth_config)))
    } else {
        None
    };

    // Create application state
    let app_state = Arc::new(AppState {
        configuration,
        database_connection,
        server_member_manager,
        config_subscriber_manager,
        console_datasource,
        oauth_service,
        persistence,
    });

    // Initialize graceful shutdown handler
    let shutdown_signal = startup::wait_for_shutdown_signal().await;
    let graceful_shutdown = GracefulShutdown::new(shutdown_signal.clone(), Duration::from_secs(30));

    // Create shared AI services (used by both main and console servers)
    let ai_services = AIServices::new();

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

    // Start gRPC servers
    let grpc_servers = startup::start_grpc_servers(
        app_state.clone(),
        naming_service.clone(),
        sdk_server_port,
        cluster_server_port,
    )?;

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
    let consul_services = if consul_enabled {
        if let Some(ref db) = rocks_db {
            info!("Consul services using RocksDB persistence");
            Some(ConsulServices::with_persistence(
                grpc_servers.naming_service.clone(),
                consul_acl_enabled,
                db.clone(),
            ))
        } else {
            Some(ConsulServices::new(
                grpc_servers.naming_service.clone(),
                consul_acl_enabled,
            ))
        }
    } else {
        info!("Consul compatibility server is disabled");
        None
    };

    // Create Apollo service adapters (only if enabled)
    let apollo_services = if apollo_enabled {
        Some(ApolloServices::new(Arc::new(
            app_state
                .database_connection
                .clone()
                .expect("Database connection required for Apollo services"),
        )))
    } else {
        info!("Apollo compatibility server is disabled");
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
        model::common::NACOS_DEPLOYMENT_TYPE_SERVER => {
            let naming_service = grpc_servers.naming_service.clone();

            info!(
                "Starting Nacos main server on {}:{}",
                server_address, server_main_port
            );
            let main = startup::main_server(
                app_state.clone(),
                grpc_servers.naming_service,
                grpc_servers.connection_manager,
                ai_services.clone(),
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

            let apollo_opt = apollo_services
                .map(|svc| {
                    info!(
                        "Starting Apollo compatibility server on {}:{}",
                        apollo_server_address, apollo_server_port
                    );
                    startup::apollo_server(
                        app_state.clone(),
                        svc,
                        apollo_server_address.clone(),
                        apollo_server_port,
                    )
                })
                .transpose()?;

            tokio::select! {
                result = async {
                    tokio::try_join!(
                        main,
                        async { match consul_opt { Some(s) => s.await, None => std::future::pending().await } },
                        async { match apollo_opt { Some(s) => s.await, None => std::future::pending().await } }
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

            // Start console, main, and optionally Consul/Apollo servers
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
                ai_services,
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

            let apollo_opt = apollo_services
                .map(|svc| {
                    info!(
                        "Starting Apollo compatibility server on {}:{}",
                        apollo_server_address, apollo_server_port
                    );
                    startup::apollo_server(
                        app_state.clone(),
                        svc,
                        apollo_server_address.clone(),
                        apollo_server_port,
                    )
                })
                .transpose()?;

            tokio::select! {
                result = async {
                    tokio::try_join!(
                        console,
                        main,
                        async { match consul_opt { Some(s) => s.await, None => std::future::pending().await } },
                        async { match apollo_opt { Some(s) => s.await, None => std::future::pending().await } }
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

    // Cleanup: stop xDS service if running
    if let Some(handle) = xds_handle {
        info!("Stopping xDS service...");
        handle.shutdown().await;
        info!("xDS service stopped");
    }

    // Cleanup: stop cluster manager if running
    if let Some(ref smm) = app_state.server_member_manager {
        if !app_state.configuration.is_standalone() {
            info!("Stopping cluster manager...");
            smm.stop().await;
            info!("Cluster manager stopped");
        }
    }

    info!("Batata server shutdown complete");
    Ok(())
}
