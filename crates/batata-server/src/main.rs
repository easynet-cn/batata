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
use batata_naming::healthcheck::{HealthCheckConfig, HealthCheckManager};
use batata_persistence::{PersistenceService, StorageMode};
use batata_plugin_consul::HealthCheckExecutor;
use batata_server::{
    console::datasource,
    middleware::rate_limit,
    model::{self, common::AppState},
    startup::{
        self, AIServices, ApolloServices, ConsulServices, GracefulShutdown, OtelConfig,
        XdsServerHandle, start_xds_service,
    },
};
use rocksdb::{BlockBasedOptions, ColumnFamilyDescriptor, Options};
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

    let (database_connection, server_member_manager, persistence, _rocks_db): (
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
                let data_dir = configuration.embedded_data_dir();
                let node_addr = format!(
                    "{}:{}",
                    configuration.server_address(),
                    configuration.server_main_port()
                );
                let node_id = batata_consistency::calculate_node_id(&node_addr);
                info!(
                    "Initializing distributed embedded storage: node_id={}, addr={}, data_dir={}",
                    node_id, node_addr, data_dir
                );

                let raft_config = batata_consistency::RaftConfig {
                    data_dir: std::path::PathBuf::from(&data_dir),
                    ..Default::default()
                };

                // Create Raft node and get the underlying DB handle for reads
                let (raft_node, rdb) = batata_consistency::RaftNode::new_with_db(
                    node_id,
                    node_addr.clone(),
                    raft_config,
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
                (None, Some(smm), Some(persist), Some(rdb))
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

    // Create application state
    let app_state = Arc::new(AppState {
        configuration,
        server_member_manager,
        config_subscriber_manager,
        console_datasource,
        oauth_service,
        persistence,
        health_check_manager,
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

    // Start health check manager (unhealthy and expired instance checkers)
    if let Some(hc_manager) = app_state.health_check_manager.as_ref() {
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
    // Consul uses independent RocksDB for persistence, separate from Nacos storage
    let consul_services = if consul_enabled {
        let consul_rocks_db = if is_console_remote {
            info!("Console remote mode: Consul will use in-memory storage");
            None
        } else {
            // Create independent RocksDB for Consul KV storage
            info!(
                "Initializing Consul RocksDB persistence at: {}",
                consul_data_dir_for_init
            );

            let mut db_opts = Options::default();
            db_opts.create_if_missing(true);
            db_opts.create_missing_column_families(true);

            // Performance optimizations
            db_opts.set_write_buffer_size(64 * 1024 * 1024);
            db_opts.set_max_write_buffer_number(3);
            db_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

            // Block-based table options with block cache
            let mut block_opts = BlockBasedOptions::default();
            let cache = rocksdb::Cache::new_lru_cache(256 * 1024 * 1024);
            block_opts.set_block_cache(&cache);
            block_opts.set_bloom_filter(10.0, false);

            // Column family options
            let mut cf_opts = Options::default();
            cf_opts.set_write_buffer_size(64 * 1024 * 1024);
            cf_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
            cf_opts.set_block_based_table_factory(&block_opts);

            let consul_cfs = vec![
                ColumnFamilyDescriptor::new(CF_CONSUL_KV, cf_opts.clone()),
                ColumnFamilyDescriptor::new(CF_CONSUL_ACL, cf_opts.clone()),
                ColumnFamilyDescriptor::new(CF_CONSUL_SESSIONS, cf_opts.clone()),
                ColumnFamilyDescriptor::new(CF_CONSUL_QUERIES, cf_opts),
            ];

            match rocksdb::DB::open_cf_descriptors(&db_opts, &consul_data_dir_for_init, consul_cfs)
            {
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
        };

        // Always use persistent mode if RocksDB is available, otherwise in-memory
        let consul_check_reap_interval = app_state.configuration.consul_check_reap_interval();
        let services = if let Some(db) = consul_rocks_db {
            info!("Consul services using RocksDB persistence");
            ConsulServices::with_persistence(
                grpc_servers.naming_service.clone(),
                consul_acl_enabled,
                db,
                consul_check_reap_interval,
            )
        } else {
            info!("Consul services using in-memory storage (no persistence)");
            ConsulServices::new(
                grpc_servers.naming_service.clone(),
                consul_acl_enabled,
                consul_check_reap_interval,
            )
        };

        // Auto-register Consul service if enabled (only in non-remote mode)
        if consul_register_self_for_init && !is_console_remote {
            info!("Auto-registering Consul service...");
            let agent = services.agent.clone();
            tokio::spawn(async move {
                // Wait a bit to ensure naming service is ready
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                if let Err(e) = agent.register_consul_service(consul_server_port).await {
                    error!("Failed to auto-register Consul service: {}", e);
                }
            });
        }

        // Start health check executor
        info!("Starting Consul health check executor...");
        let health = Arc::new(services.health.clone());
        let health_actor = services.health.health_actor_handle();
        let naming_service_clone = grpc_servers.naming_service.clone();
        tokio::spawn(async move {
            let executor = HealthCheckExecutor::new(health, health_actor, naming_service_clone);
            executor.start().await;
        });

        Some(services)
    } else {
        info!("Consul compatibility server is disabled");
        None
    };

    // Create Apollo service adapters (only if enabled and external DB is available)
    let apollo_services = if apollo_enabled {
        if let Some(ref db_conn) = database_connection {
            Some(ApolloServices::new(Arc::new(db_conn.clone())))
        } else {
            tracing::warn!(
                "Apollo compatibility is enabled but no external database is available (embedded mode). Apollo server will be disabled."
            );
            None
        }
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
