//! Application builder - orchestrates the full initialization sequence
//!
//! This module provides the main AppBuilder that orchestrates
//! the entire application initialization sequence in a clear,
//! phase-based manner.
//!
//! Phase order:
//! 1. Configuration & logging
//! 2. Deployment mode & plugin registration
//! 2.5. Protocol adapter plugin registration
//! 3. Persistence layer
//! 4. Shared services & application state
//! 5. Shutdown handler & AI services
//! 6. Console-remote early return (if applicable)
//! 7. gRPC servers
//! 8. Background tasks
//! 9. Cluster & Raft initialization
//! 10. xDS service mesh
//! 11. Protocol adapter initialization
//! 12. HTTP servers
//! 13. Graceful shutdown

use std::sync::Arc;
use std::time::Duration;

use batata_auth::service::oauth::OAuthService;
use batata_naming::healthcheck::{HealthCheckConfig, HealthCheckManager};
use batata_plugin::{Plugin as _, ProtocolAdapterPlugin as _};
use batata_server_common::ServerStatusManager;
use tracing::{error, info, warn};

use crate::context::DeploymentMode;
use crate::model::common::{AppState, Configuration, DEPLOYMENT_TYPE_CONSOLE};
use crate::startup::{
    self, AIServices, GracefulShutdown, OtelConfig, XdsServerHandle, start_xds_service,
};

/// Application builder - orchestrates the full initialization sequence
///
/// This struct holds all intermediate state during server startup
/// and provides phase-based builder methods for clarity.
pub struct AppBuilder {
    // Phase 1: Configuration
    configuration: Option<Arc<Configuration>>,
    otel_config: Option<OtelConfig>,
    _logging_guard: Option<startup::LoggingGuard>,
    _system_stats_handle: Option<tokio::task::JoinHandle<()>>,
    _rate_limit_cleanup_handle: Option<tokio::task::JoinHandle<()>>,

    // Phase 2: Deployment mode
    deployment_mode: DeploymentMode,
    is_console_remote: bool,

    // Phase 2.5: Plugin manager
    plugin_manager: Option<batata_plugin::spi::PluginManager>,
    consul_plugin_ref: Option<Arc<batata_plugin_consul::ConsulPlugin>>,
    plugin_cf_names: Vec<String>,

    // Phase 3: Persistence
    persistence_ctx: Option<startup::persistence::PersistenceContext>,

    // Phase 4: Services and state
    config_subscriber_manager:
        Option<Arc<dyn batata_common::ConfigSubscriptionService>>,
    naming_service_concrete: Option<Arc<batata_naming::NamingService>>,
    naming_service: Option<Arc<dyn batata_api::naming::NamingServiceProvider>>,
    plugin_state_providers: Vec<Arc<dyn batata_plugin::PluginStateProvider>>,
    console_datasource: Option<Arc<dyn batata_console::ConsoleDataSource>>,
    oauth_service: Option<Arc<dyn batata_common::OAuthProvider>>,
    health_check_manager: Option<Arc<HealthCheckManager>>,
    server_status: Option<Arc<ServerStatusManager>>,
    control_plugin: Option<Arc<dyn batata_plugin::ControlPlugin>>,
    encryption_service: Option<Arc<batata_config::service::encryption::ConfigEncryptionService>>,
    auth_plugin: Option<Arc<dyn batata_common::AuthPlugin>>,
    app_state: Option<Arc<AppState>>,

    // Phase 5: Shutdown & AI
    shutdown_signal: Option<startup::ShutdownSignal>,
    graceful_shutdown: Option<GracefulShutdown>,
    ai_services: Option<AIServices>,

    // Phase 7: gRPC servers
    grpc_servers: Option<startup::GrpcServers>,
    server_registry: Option<Arc<batata_core::ServerRegistry>>,

    // Phase 10: xDS
    xds_handle: Option<XdsServerHandle>,
}

impl AppBuilder {
    /// Create a new application builder
    pub fn new() -> Self {
        Self {
            configuration: None,
            otel_config: None,
            _logging_guard: None,
            _system_stats_handle: None,
            _rate_limit_cleanup_handle: None,
            deployment_mode: DeploymentMode::Merged,
            is_console_remote: false,
            plugin_manager: None,
            consul_plugin_ref: None,
            plugin_cf_names: Vec::new(),
            persistence_ctx: None,
            config_subscriber_manager: None,
            naming_service_concrete: None,
            naming_service: None,
            plugin_state_providers: Vec::new(),
            console_datasource: None,
            oauth_service: None,
            health_check_manager: None,
            server_status: None,
            control_plugin: None,
            encryption_service: None,
            auth_plugin: None,
            app_state: None,
            shutdown_signal: None,
            graceful_shutdown: None,
            ai_services: None,
            grpc_servers: None,
            server_registry: None,
            xds_handle: None,
        }
    }

    // ========================================================================
    // Phase 1: Configuration and logging
    // ========================================================================

    /// Load configuration and initialize logging
    pub async fn with_config_and_logging(
        mut self,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 1] Loading configuration and initializing logging...");

        let configuration = Configuration::new()?;
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
        let logging_guard = startup::init_logging(&logging_config, Some(&otel_config))?;

        if otel_config.enabled {
            info!(
                "OpenTelemetry tracing enabled, exporting to {}",
                otel_config.otlp_endpoint
            );
        }

        crate::metrics::init_metrics();

        let system_stats_handle = if configuration.metrics_system_stats_enabled() {
            let interval_secs = configuration.metrics_system_stats_interval_secs();
            tracing::info!(
                "Starting system stats reporter with interval of {} seconds",
                interval_secs
            );
            Some(crate::metrics::start_system_stats_reporter(Some(interval_secs)))
        } else {
            tracing::info!("System stats reporter is disabled");
            None
        };

        let rate_limit_cleanup_handle = crate::middleware::rate_limit::start_cleanup_task();

        // Initialize auth caches
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

        self.configuration = Some(Arc::new(configuration));
        self.otel_config = Some(otel_config);
        self._logging_guard = Some(logging_guard);
        self._system_stats_handle = system_stats_handle;
        self._rate_limit_cleanup_handle = Some(rate_limit_cleanup_handle);

        info!("Phase 1 complete - configuration loaded, logging initialized");
        Ok(self)
    }

    // ========================================================================
    // Phase 2: Determine deployment mode & register plugins
    // ========================================================================

    /// Determine deployment mode and register plugins
    pub async fn with_plugins(
        mut self,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 2] Determining deployment mode and registering plugins...");

        let config = self.configuration.as_ref().unwrap();
        let deployment_type = config.deployment_type();
        let is_console_remote = deployment_type == DEPLOYMENT_TYPE_CONSOLE;

        if is_console_remote
            && config.console_remote_username() == "batata"
            && config.console_remote_password() == "batata"
        {
            tracing::warn!(
                "Console remote mode is using default credentials (batata/batata). \
                 This is insecure for production environments. \
                 Set 'batata.console.remote.username' and 'batata.console.remote.password'."
            );
        }

        // Register the default trace subscriber
        batata_plugin::global_trace_registry()
            .register(Arc::new(batata_plugin::LoggingTraceSubscriber::new()));

        self.deployment_mode = DeploymentMode::from_str(&deployment_type);
        self.is_console_remote = is_console_remote;

        info!(
            "Deployment mode: {} (console_remote: {})",
            deployment_type, is_console_remote
        );

        // ====================================================================
        // Phase 2.5: Register protocol adapter plugins
        // ====================================================================
        info!("[Phase 2.5] Registering protocol adapter plugins...");

        let mut plugin_manager = batata_plugin::spi::PluginManager::new();
        let mut consul_plugin_ref: Option<Arc<batata_plugin_consul::ConsulPlugin>> = None;

        let consul_config =
            batata_plugin_consul::ConsulPluginConfig::from_config(&config.config);
        let consul_plugin = Arc::new(batata_plugin_consul::ConsulPlugin::from_plugin_config(
            consul_config,
        ));

        if consul_plugin.is_enabled() {
            plugin_manager.register_protocol_adapter(consul_plugin.clone());
            consul_plugin_ref = Some(consul_plugin.clone());
            info!("Consul plugin registered and enabled");
        } else {
            info!("Consul plugin disabled by configuration");
        }

        // Collect plugin CFs before creating RocksDB
        let plugin_cf_names = plugin_manager.collect_plugin_column_families();

        self.plugin_manager = Some(plugin_manager);
        self.consul_plugin_ref = consul_plugin_ref;
        self.plugin_cf_names = plugin_cf_names;

        info!("Phase 2 complete - plugins registered");
        Ok(self)
    }

    // ========================================================================
    // Phase 3: Initialize persistence layer
    // ========================================================================

    /// Initialize persistence layer
    pub async fn with_persistence(
        mut self,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 3] Initializing persistence layer...");

        let config = self.configuration.as_ref().unwrap();
        let plugin_cf_names = self.plugin_cf_names.clone();

        let persistence_ctx = if self.is_console_remote {
            info!("Starting in console remote mode - connecting to remote server");
            startup::persistence::PersistenceContext::empty()
        } else {
            startup::persistence::init_persistence(config.as_ref(), &plugin_cf_names).await?
        };

        self.persistence_ctx = Some(persistence_ctx);
        info!("Phase 3 complete - persistence layer initialized");
        Ok(self)
    }

    // ========================================================================
    // Phase 4: Create shared services and application state
    // ========================================================================

    /// Create shared services and application state
    pub async fn with_services(
        mut self,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 4] Creating shared services and application state...");

        let config = self.configuration.as_ref().unwrap();
        let persistence_ctx = self.persistence_ctx.as_ref().unwrap();

        // Config subscriber
        let config_subscriber_manager: Arc<dyn batata_common::ConfigSubscriptionService> =
            Arc::new(batata_core::ConfigSubscriberManager::new());

        // Naming service
        let naming_service_concrete: Option<Arc<batata_naming::NamingService>> =
            if !self.is_console_remote {
                Some(Arc::new(batata_naming::NamingService::new()))
            } else {
                None
            };
        let naming_service: Option<Arc<dyn batata_api::naming::NamingServiceProvider>> =
            naming_service_concrete
                .clone()
                .map(|ns| ns as Arc<dyn batata_api::naming::NamingServiceProvider>);

        // Plugin state providers
        let consul_plugin_clone = self.consul_plugin_ref.clone();
        let plugin_state_providers: Vec<Arc<dyn batata_plugin::PluginStateProvider>> =
            match consul_plugin_clone {
                Some(plugin) => vec![plugin as Arc<dyn batata_plugin::PluginStateProvider>],
                None => {
                    // Create a stub so state API still works
                    vec![]
                }
            };

        // Console datasource
        let consul_for_ds = self.consul_plugin_ref.clone();
        let ds_plugins: Vec<Arc<dyn batata_plugin::PluginStateProvider>> =
            if let Some(ref c) = consul_for_ds {
                vec![c.clone() as Arc<dyn batata_plugin::PluginStateProvider>]
            } else {
                Vec::new()
            };

        let console_datasource = batata_console::create_datasource(
            config,
            persistence_ctx.cluster_manager.clone(),
            config_subscriber_manager.clone(),
            naming_service.clone(),
            persistence_ctx.persistence.clone(),
            ds_plugins.clone(),
        )
        .await?;

        // OAuth service
        let oauth_service = init_oauth_service(config)?;

        // Health check manager
        let health_check_manager: Option<Arc<HealthCheckManager>> =
            if let Some(ref ns) = naming_service_concrete {
                let health_check_config = Arc::new(HealthCheckConfig {
                    heartbeat_interval_secs: config.naming_heartbeat_check_interval_secs(),
                    ttl_monitor_interval_secs: config.naming_ttl_monitor_interval_secs(),
                    deregister_monitor_interval_secs: config
                        .naming_deregister_monitor_interval_secs(),
                    ..HealthCheckConfig::default()
                });
                let expire_enabled = config.expire_instance_enabled();
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

        // Control plugin
        let control_plugin = init_control_plugin(config).await;

        // Encryption service
        let encryption_service = init_encryption_service(config);

        // Auth plugin
        let auth_plugin: Option<Arc<dyn batata_common::AuthPlugin>> =
            persistence_ctx.persistence.as_ref().map(|p| {
                let ldap_config = if config.auth_system_type() == "ldap" {
                    Some(config.ldap_config())
                } else {
                    None
                };
                batata_auth::plugin::create_auth_plugin(
                    &config.auth_system_type(),
                    config.token_secret_key(),
                    config.auth_token_expire_seconds(),
                    p.clone(),
                    ldap_config,
                )
            });

        // Clone Arc fields before moving into AppState
        let console_datasource_for_self = console_datasource.clone();
        let oauth_service_for_self = oauth_service.clone();
        let control_plugin_for_self = control_plugin.clone();
        let auth_plugin_for_self = auth_plugin.clone();

        // Build app state
        let app_state = Arc::new(AppState {
            configuration: config.as_ref().clone(),
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
            plugin_state_providers: plugin_state_providers.clone(),
            log_level_setter: Some(Arc::new(|filter: &str| {
                crate::startup::logging::set_log_level(filter)
            })),
        });

        if !app_state.configuration.data_warmup() {
            server_status.set_up();
            info!("Server status: UP (data warmup disabled)");
        } else {
            info!("Server status: STARTING (data warmup enabled, waiting for subsystems)");
        }

        self.config_subscriber_manager = Some(config_subscriber_manager);
        self.naming_service_concrete = naming_service_concrete;
        self.naming_service = naming_service;
        self.plugin_state_providers = plugin_state_providers;
        self.console_datasource = Some(console_datasource_for_self);
        self.oauth_service = oauth_service_for_self;
        self.health_check_manager = health_check_manager;
        self.server_status = Some(server_status);
        self.control_plugin = control_plugin_for_self;
        self.encryption_service = Some(encryption_service);
        self.auth_plugin = auth_plugin_for_self;
        self.app_state = Some(app_state);

        info!("Phase 4 complete - shared services and application state created");
        Ok(self)
    }

    // ========================================================================
    // Phase 5: Initialize shutdown handler and AI services
    // ========================================================================

    /// Initialize shutdown handler and AI services
    pub async fn with_shutdown_and_ai(
        mut self,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 5] Initializing shutdown handler and AI services...");

        let app_state = self.app_state.as_ref().unwrap();
        let naming_service = self.naming_service.clone();

        let drain_timeout_secs = app_state.configuration.shutdown_drain_timeout_secs();
        let shutdown_signal = startup::wait_for_shutdown_signal().await;
        let graceful_shutdown =
            GracefulShutdown::new(shutdown_signal.clone(), Duration::from_secs(drain_timeout_secs));

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

        self.shutdown_signal = Some(shutdown_signal);
        self.graceful_shutdown = Some(graceful_shutdown);
        self.ai_services = Some(ai_services);

        info!("Phase 5 complete - shutdown handler and AI services initialized");
        Ok(self)
    }

    // ========================================================================
    // Phase 6: Console-remote early return check
    // ========================================================================

    /// Check if running in console-remote mode and return early
    ///
    /// Returns `Ok(None)` if not console-remote (continue normal startup),
    /// or `Ok(Some(()))` if console-remote and already handled.
    pub async fn check_console_remote(&self) -> Result<Option<()>, Box<dyn std::error::Error>> {
        if !self.is_console_remote {
            return Ok(None);
        }

        info!("[Phase 6] Console-remote mode - starting console server only...");

        let app_state = self.app_state.as_ref().unwrap();
        let ai_services = self.ai_services.as_ref().unwrap().clone();
        let graceful_shutdown = self.graceful_shutdown.as_ref().unwrap().clone();
        let server_status = self.server_status.as_ref().unwrap().clone();
        let database_connection = self.persistence_ctx.as_ref().unwrap().database_connection.clone();

        run_console_remote(
            app_state.clone(),
            ai_services,
            graceful_shutdown,
            server_status,
            database_connection,
        )
        .await?;

        Ok(Some(()))
    }

    // ========================================================================
    // Phase 7: Start gRPC servers
    // ========================================================================

    /// Start gRPC servers
    pub async fn with_grpc_servers(
        mut self,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 7] Starting gRPC servers...");

        let app_state = self.app_state.as_ref().unwrap();
        let ai_services = self.ai_services.as_ref().unwrap();
        let persistence_ctx = self.persistence_ctx.as_ref().unwrap();
        let config = self.configuration.as_ref().unwrap();

        let grpc_servers = startup::start_grpc_servers(
            app_state.clone(),
            self.naming_service_concrete.clone(),
            ai_services,
            config.sdk_server_port(),
            config.cluster_server_port(),
            persistence_ctx.raft_node.clone(),
            persistence_ctx.server_member_manager.clone(),
            self.config_subscriber_manager.as_ref().unwrap().clone(),
        )?;

        // Wire naming apply hook for Raft
        if let (Some(raft), Some(ns)) = (&persistence_ctx.raft_node, &self.naming_service_concrete) {
            let hook = std::sync::Arc::new(
                batata_naming::handler::raft_hook::NamingApplyHookImpl::new(ns.clone()),
            );
            raft.register_naming_hook(hook).await;
            match raft.replay_persistent_instances().await {
                Ok(n) => info!(
                    "Persistent instance replay complete: {} instances restored",
                    n
                ),
                Err(e) => warn!("Persistent instance replay failed: {}", e),
            }
        }

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

        self.grpc_servers = Some(grpc_servers);
        self.server_registry = Some(server_registry);

        info!("Phase 7 complete - gRPC servers started");
        Ok(self)
    }

    // ========================================================================
    // Phase 8: Start background tasks
    // ========================================================================

    /// Start background tasks (health checks, warmup, MCP index refresh)
    pub async fn with_background_tasks(
        self,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 8] Starting background tasks...");

        let app_state = self.app_state.as_ref().unwrap();
        let grpc_servers = self.grpc_servers.as_ref().unwrap();
        let persistence_ctx = self.persistence_ctx.as_ref().unwrap();
        let ai_services = self.ai_services.as_ref().unwrap();
        let server_status = self.server_status.as_ref().unwrap();

        // Upgrade health check to cluster mode if not standalone
        if !app_state.configuration.is_standalone() && self.health_check_manager.is_some() {
            let hc_manager = self.health_check_manager.as_ref().unwrap();
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

        // Start health checkers
        start_health_checkers(&self.health_check_manager);

        // Start warmup poller
        start_warmup_poller(
            app_state,
            server_status,
            persistence_ctx.server_member_manager.as_ref(),
        );

        // Start MCP index refresh
        start_mcp_index_refresh(ai_services, app_state);

        info!("Phase 8 complete - background tasks started");
        Ok(self)
    }

    // ========================================================================
    // Phase 9: Cluster and Raft initialization
    // ========================================================================

    /// Initialize cluster and Raft
    pub async fn with_cluster_init(
        self,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 9] Initializing cluster and Raft...");

        let app_state = self.app_state.as_ref().unwrap();
        let persistence_ctx = self.persistence_ctx.as_ref().unwrap();
        let grpc_servers = self.grpc_servers.as_ref().unwrap();

        if let Some(ref smm) = persistence_ctx.server_member_manager {
            startup::cluster::init_cluster(
                &app_state.configuration,
                smm,
                persistence_ctx.raft_node.as_ref(),
                grpc_servers,
            )
            .await?;
        }

        info!("Phase 9 complete - cluster and Raft initialized");
        Ok(self)
    }

    // ========================================================================
    // Phase 10: xDS service mesh support
    // ========================================================================

    /// Start xDS service if enabled
    pub async fn with_xds(
        mut self,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 10] Starting xDS service mesh support...");

        let app_state = self.app_state.as_ref().unwrap();
        let grpc_servers = self.grpc_servers.as_ref().unwrap();

        let xds_handle = start_xds_if_enabled(app_state, grpc_servers).await;
        self.xds_handle = xds_handle;

        info!("Phase 10 complete - xDS service initialized");
        Ok(self)
    }

    // ========================================================================
    // Phase 11: Initialize protocol adapter plugins
    // ========================================================================

    /// Initialize protocol adapter plugins
    pub async fn with_plugin_init(
        self,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 11] Initializing protocol adapter plugins...");

        let app_state = self.app_state.as_ref().unwrap();
        let grpc_servers = self.grpc_servers.as_ref().unwrap();
        let persistence_ctx = self.persistence_ctx.as_ref().unwrap();
        let plugin_manager = self.plugin_manager.as_ref().unwrap();
        let is_cluster = !app_state.configuration.is_standalone() && !self.is_console_remote;

        // Build plugin context
        let mut plugin_ctx = batata_plugin::PluginContext::new();
        plugin_ctx.insert("is_cluster", Arc::new(is_cluster));

        if let Some(ref raft) = app_state.raft_node {
            plugin_ctx.insert("raft_node", raft.clone());
        }
        if let Some(ref cm) = persistence_ctx.cluster_manager {
            plugin_ctx.insert("cluster_manager", Arc::new(cm.clone()));
        }
        if is_cluster {
            plugin_ctx.insert("distro_protocol", grpc_servers.distro_protocol().clone());
        }

        // Initialize all plugins
        plugin_manager.init_protocol_adapters(&plugin_ctx).await?;

        // Wire Consul event broadcast (cluster mode only)
        if let Some(ref consul_plugin) = self.consul_plugin_ref {
            let event_service = consul_plugin.event_service().clone();

            grpc_servers.handler_registry().register_handler(Arc::new(
                crate::service::consul_event_handler::ConsulEventBroadcastHandler {
                    event_service: event_service.clone(),
                },
            ));

            if let Some(ref client_manager) = *grpc_servers.cluster_client_manager()
                && let Some(ref smm) = persistence_ctx.server_member_manager
            {
                let broadcaster = Arc::new(
                    crate::service::consul_event_handler::ConsulEventBroadcasterImpl::new(
                        client_manager.clone(),
                        smm.clone(),
                    ),
                );
                event_service.set_broadcaster(broadcaster).await;
                info!("Consul event cluster broadcast enabled");
            }
        }

        info!("Phase 11 complete - protocol adapter plugins initialized");
        Ok(self)
    }

    // ========================================================================
    // Phase 12: Start HTTP servers and wait for shutdown
    // ========================================================================

    /// Start HTTP servers and wait for shutdown signal
    pub async fn run_http_servers(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("[Phase 12] Starting HTTP servers...");

        let app_state = self.app_state.as_ref().unwrap();
        let grpc_servers = self.grpc_servers.as_ref().unwrap();
        let ai_services = self.ai_services.as_ref().unwrap().clone();
        let plugin_manager = self.plugin_manager.as_ref().unwrap();
        let graceful_shutdown = self.graceful_shutdown.as_ref().unwrap();
        let server_registry = self.server_registry.as_ref().unwrap();
        let encryption_service = self.encryption_service.as_ref().unwrap().clone();

        let distro_for_http = if !app_state.configuration.is_standalone() {
            Some(grpc_servers.distro_protocol().clone())
        } else {
            None
        };
        let naming_provider_for_http: Arc<dyn batata_api::naming::NamingServiceProvider> =
            grpc_servers.naming_provider();

        run_http_servers(
            &self.deployment_mode,
            app_state,
            naming_provider_for_http,
            grpc_servers,
            ai_services,
            distro_for_http,
            encryption_service,
            plugin_manager,
            graceful_shutdown,
            server_registry,
        )
        .await?;

        info!("Phase 12 complete - HTTP servers stopped");
        Ok(())
    }

    // ========================================================================
    // Phase 13: Graceful shutdown sequence
    // ========================================================================

    /// Execute graceful shutdown sequence
    pub async fn graceful_shutdown(&mut self) {
        info!("[Phase 13] Executing graceful shutdown sequence...");

        let server_status = self.server_status.as_ref().unwrap();
        let graceful_shutdown = self.graceful_shutdown.as_ref().unwrap();
        let app_state = self.app_state.as_ref().unwrap();
        let grpc_servers = self.grpc_servers.as_ref().unwrap();
        let xds_handle = self.xds_handle.take();
        let persistence_ctx = self.persistence_ctx.as_ref().unwrap();
        let server_registry = self.server_registry.as_ref().unwrap();

        graceful_shutdown_sequence(
            server_status,
            graceful_shutdown,
            app_state,
            grpc_servers,
            xds_handle,
            persistence_ctx.server_member_manager.as_ref(),
            persistence_ctx.database_connection.clone(),
            server_registry,
        )
        .await;

        info!("Phase 13 complete - graceful shutdown finished");
    }

    // ========================================================================
    // Run the complete server lifecycle
    // ========================================================================

    /// Run the complete server lifecycle (all phases)
    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        let builder = AppBuilder::new()
            .with_config_and_logging()
            .await?
            .with_plugins()
            .await?
            .with_persistence()
            .await?
            .with_services()
            .await?
            .with_shutdown_and_ai()
            .await?;

        // Check for console-remote early return
        if builder.check_console_remote().await?.is_some() {
            info!("Batata server shutdown complete (console-remote mode)");
            return Ok(());
        }

        let mut builder = builder
            .with_grpc_servers()
            .await?
            .with_background_tasks()
            .await?
            .with_cluster_init()
            .await?
            .with_xds()
            .await?
            .with_plugin_init()
            .await?;

        builder.run_http_servers().await?;
        builder.graceful_shutdown().await;

        info!("Batata server shutdown complete");
        Ok(())
    }
}

impl Default for AppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helper functions (moved from main.rs)
// ============================================================================

/// Validate configuration and exit on fatal errors.
fn validate_config(configuration: &Configuration) {
    if configuration.auth_enabled() && configuration.token_secret_key().is_empty() {
        eprintln!(
            "FATAL: Authentication is enabled (batata.core.auth.enabled=true) but no JWT secret key is configured."
        );
        eprintln!(
            "Set 'batata.core.auth.plugin.default.token.secret.key' to a non-empty Base64-encoded secret."
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
        eprintln!("Set it via: batata.core.auth.plugin.default.token.secret.key=<your-key>");
    }
}

/// Initialize OAuth service if enabled.
fn init_oauth_service(
    configuration: &Configuration,
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
    configuration: &Configuration,
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
    configuration: &Configuration,
) -> Arc<batata_config::service::encryption::ConfigEncryptionService> {
    use batata_config::service::encryption::{ConfigEncryptionService, EncryptionPattern};

    let registry = batata_plugin::global_encryption_registry();
    registry.register(Arc::new(batata_plugin::NoopEncryptionPlugin::new()));

    if configuration.encryption_enabled() {
        match configuration.encryption_key() {
            Some(key) if !key.is_empty() => {
                match batata_common::crypto::AesGcmEncryptionPlugin::new(&key) {
                    Ok(plugin) => {
                        registry.register(Arc::new(plugin));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to build AES-GCM encryption plugin: {}", e);
                    }
                }
            }
            _ => {
                tracing::warn!("Encryption enabled but no key configured");
            }
        }
    }

    let default_patterns = vec![EncryptionPattern::Prefix("cipher-".to_string())];
    let plugin_name = configuration.encryption_plugin_type();

    if let Some(plugin) = registry.find(&plugin_name) {
        if plugin.is_enabled() {
            info!("Config encryption enabled via plugin '{}'", plugin.name());
        } else {
            info!("Config encryption plugin '{}' is disabled", plugin.name());
        }
        return Arc::new(ConfigEncryptionService::from_plugin(
            plugin,
            default_patterns,
        ));
    }

    tracing::warn!(
        "Encryption plugin '{}' not found in registry; disabling encryption",
        plugin_name
    );

    Arc::new(ConfigEncryptionService::disabled())
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

    server_status.set_down();

    if let Some(db) = database_connection {
        let _ = tokio::time::timeout(Duration::from_secs(5), db.close()).await;
    }

    Ok(())
}

/// Start HTTP servers based on deployment type and wait for shutdown signal.
#[allow(clippy::too_many_arguments)]
async fn run_http_servers(
    deployment_mode: &DeploymentMode,
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
        || matches!(deployment_mode, DeploymentMode::ServerWithMcp);
    let mcp_registry_port = app_state.configuration.mcp_registry_port();

    let main_http_state = batata_core::ServerStateTracker::new();
    let console_http_state = batata_core::ServerStateTracker::new();

    match deployment_mode {
        DeploymentMode::Console => {
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
        DeploymentMode::Server | DeploymentMode::ServerWithMcp => {
            info!(
                "Starting Batata main server on {}:{}",
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
                grpc_servers.cluster_client_manager().clone(),
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

            plugin_manager.start_protocol_adapter_tasks().await?;

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
        DeploymentMode::Merged => {
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
                "Starting Batata main server on {}:{}",
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
                grpc_servers.cluster_client_manager().clone(),
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

            plugin_manager.start_protocol_adapter_tasks().await?;

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
    mcp_registry: &Arc<crate::api::ai::McpServerRegistry>,
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
    let states = server_registry.health();

    info!(
        "Starting graceful shutdown ({} servers registered): {:?}",
        states.len(),
        states
            .iter()
            .map(|s| format!("{}={}", s.name, s.state))
            .collect::<Vec<_>>()
    );

    server_status.set_draining();

    let drain_timeout = graceful_shutdown.drain_timeout();

    info!(
        "Server status: DRAINING (waiting up to {}s for in-flight requests)...",
        drain_timeout.as_secs()
    );

    grpc_servers.shutdown();
    info!("gRPC servers signaled to shut down");

    let drain_sleep = drain_timeout.min(Duration::from_secs(5));
    tokio::time::sleep(drain_sleep).await;

    server_status.set_down();
    info!("Server status: DOWN");

    if let Some(handle) = xds_handle {
        info!("Stopping xDS service...");
        handle.shutdown().await;
        info!("xDS service stopped");
    }

    if app_state.health_check_manager.is_some() {
        info!("Health check manager will be stopped (background tasks cancelled on drop)");
    }

    if let Some(smm) = server_member_manager
        && !app_state.configuration.is_standalone()
    {
        info!("Stopping cluster manager...");
        smm.stop().await;
        info!("Cluster manager stopped");
    }

    if let Some(ref raft_node) = app_state.raft_node {
        info!("Shutting down Raft node...");
        match tokio::time::timeout(Duration::from_secs(10), raft_node.shutdown()).await {
            Ok(Ok(())) => info!("Raft node shutdown complete"),
            Ok(Err(e)) => error!("Raft node shutdown error: {}", e),
            Err(_) => error!("Raft node shutdown timed out after 10s"),
        }
    }

    if let Some(db) = database_connection {
        let db_timeout = app_state.configuration.shutdown_db_close_timeout_secs();
        info!("Closing database connections (timeout: {}s)...", db_timeout);
        match tokio::time::timeout(Duration::from_secs(db_timeout), db.close()).await {
            Ok(Ok(())) => info!("Database connections closed"),
            Ok(Err(e)) => error!("Error closing database connections: {}", e),
            Err(_) => error!("Database close timed out after {}s", db_timeout),
        }
    }

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
