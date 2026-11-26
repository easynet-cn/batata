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
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use batata_auth::service::oauth::OAuthService;
use batata_naming::healthcheck::{HealthCheckConfig, HealthCheckManager};
use batata_plugin::Plugin as _;
#[cfg(any(feature = "consul", feature = "apollo"))]
use batata_plugin::ProtocolAdapterPlugin as _;
use batata_plugin::context_keys;
use batata_plugin::spi::PluginManager;
use batata_server_common::ServerStatusManager;
use tracing::{debug, error, info, warn};

use super::lifecycle_timing::{
    LifecycleKind, LifecycleTimeline, finish_startup_phase, mode_label, phases, shutdown_summary,
    startup_summary, storage_label,
};
use crate::context::DeploymentMode;
use crate::model::common::{AppState, Configuration, DEPLOYMENT_TYPE_CONSOLE};
use crate::startup::{
    self, AIServices, GracefulShutdown, IstioMcpServerHandle, OtelConfig,
    PersistenceIstioMcpConfigProvider, XdsServerHandle, start_istio_mcp_service, start_xds_service,
};

type TrackedTasks = Vec<(&'static str, tokio::task::JoinHandle<()>)>;

/// Inner task registry. Aborting here (rather than in `BackgroundTasks::drop`)
/// means cleanup fires only when the last clone is dropped — a short-lived
/// clone must not kill tasks still owned by the server.
struct TaskRegistry(std::sync::Mutex<TrackedTasks>);

impl Default for TaskRegistry {
    fn default() -> Self {
        Self(std::sync::Mutex::new(Vec::new()))
    }
}

impl TaskRegistry {
    fn lock(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, TrackedTasks>> {
        self.0.lock()
    }
}

impl Drop for TaskRegistry {
    fn drop(&mut self) {
        // Last-owner cleanup for a failed startup: `?` unwinds AppBuilder, the
        // final BackgroundTasks clone drops, and this aborts any task that
        // graceful shutdown never got to (JoinHandle drop alone only detaches).
        // On a successful shutdown `shutdown()` already mem::take'd the vec, so
        // this is a no-op.
        if let Ok(tasks) = self.0.get_mut() {
            for (name, handle) in tasks.drain(..) {
                if !handle.is_finished() {
                    handle.abort();
                    debug!(task = name, "aborted background task on startup cleanup");
                }
            }
        }
    }
}

/// Tracks tokio tasks owned by the server startup path so shutdown can cancel
/// and join them instead of relying on runtime teardown.
#[derive(Clone, Default)]
struct BackgroundTasks {
    tasks: Arc<TaskRegistry>,
}

impl BackgroundTasks {
    fn push(&self, name: &'static str, handle: tokio::task::JoinHandle<()>) {
        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.push((name, handle));
        }
    }

    fn spawn<F>(&self, name: &'static str, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.push(name, tokio::spawn(future));
    }

    fn len(&self) -> usize {
        self.tasks
            .lock()
            .map(|tasks| tasks.len())
            .unwrap_or_default()
    }

    async fn shutdown(&self, timeout: Duration) {
        let owned = self
            .tasks
            .lock()
            .map(|mut tasks| std::mem::take(&mut *tasks))
            .unwrap_or_default();
        let deadline = std::time::Instant::now() + timeout;
        for (name, handle) in owned {
            handle.abort();
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match tokio::time::timeout(remaining, handle).await {
                Ok(Ok(())) => info!(task = name, "background task stopped"),
                Ok(Err(error)) if error.is_cancelled() => {
                    info!(task = name, "background task aborted")
                }
                Ok(Err(error)) => warn!(task = name, error = %error, "background task panicked"),
                Err(_) => warn!(task = name, "background task join timed out"),
            }
        }
    }
}

/// Application builder - orchestrates the full initialization sequence
///
/// This struct holds all intermediate state during server startup
/// and provides phase-based builder methods for clarity.
pub struct AppBuilder {
    // Phase 1: Configuration
    configuration: Option<Arc<Configuration>>,
    otel_config: Option<OtelConfig>,
    _logging_guard: Option<startup::LoggingGuard>,
    background_tasks: BackgroundTasks,
    startup_complete: Arc<AtomicBool>,
    /// Shared phase timeline; cloned into HTTP/shutdown helpers that outlive a
    /// mutable `self` phase chain.
    timeline: LifecycleTimeline,

    // Phase 2: Deployment mode
    deployment_mode: DeploymentMode,
    is_console_remote: bool,

    // Phase 2.5: Plugin manager. One shared instance is used by protocol
    // adapters, config-change hooks, state reporting and shutdown.
    plugin_manager: Option<Arc<batata_plugin::spi::PluginManager>>,
    #[cfg(feature = "consul")]
    consul_plugin_ref: Option<Arc<batata_plugin_consul::ConsulPlugin>>,
    #[cfg(feature = "apollo")]
    apollo_plugin_ref: Option<Arc<batata_plugin_apollo::ApolloPlugin>>,
    plugin_cf_names: Vec<String>,

    // Phase 3: Persistence
    persistence_ctx: Option<startup::persistence::PersistenceContext>,

    // Phase 4: Services and state
    config_subscriber_manager: Option<Arc<dyn batata_common::ConfigSubscriptionService>>,
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

    // Phase 10: xDS and Istio MCP
    xds_handle: Option<XdsServerHandle>,
    istio_mcp_handle: Option<IstioMcpServerHandle>,
}

impl AppBuilder {
    /// Create a new application builder
    pub fn new() -> Self {
        Self {
            configuration: None,
            otel_config: None,
            _logging_guard: None,
            background_tasks: BackgroundTasks::default(),
            startup_complete: Arc::new(AtomicBool::new(false)),
            timeline: LifecycleTimeline::new(),
            deployment_mode: DeploymentMode::Merged,
            is_console_remote: false,
            plugin_manager: None,
            #[cfg(feature = "consul")]
            consul_plugin_ref: None,
            #[cfg(feature = "apollo")]
            apollo_plugin_ref: None,
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
            istio_mcp_handle: None,
        }
    }

    // ========================================================================
    // Phase 1: Configuration and logging
    // ========================================================================

    /// Load configuration and initialize logging
    pub async fn with_config_and_logging(mut self) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 1] Loading configuration and initializing logging...");

        let configuration = Configuration::new()?;
        validate_config(&configuration)?;

        let logging_config = startup::LoggingConfig::from_config(
            configuration.log_dir(),
            configuration.log_console_enabled(),
            configuration.log_file_enabled(),
            configuration.log_level(),
        )
        .with_env_overrides();
        let otel_config = OtelConfig::from_config(
            configuration.otel_enabled(),
            configuration.otel_endpoint(),
            configuration.otel_service_name(),
            configuration.otel_sampling_ratio(),
            configuration.otel_export_timeout_secs(),
        );
        let logging_guard = startup::init_logging(&logging_config, Some(&otel_config))?;

        #[cfg(feature = "http-tracing")]
        if otel_config.enabled {
            info!(
                "OpenTelemetry tracing enabled, exporting to {}",
                otel_config.otlp_endpoint
            );
        }

        crate::metrics::init_metrics();

        if configuration.metrics_system_stats_enabled() {
            let interval_secs = configuration.metrics_system_stats_interval_secs();
            tracing::info!(
                "Starting system stats reporter with interval of {} seconds",
                interval_secs
            );
            self.background_tasks.push(
                "system-stats-reporter",
                crate::metrics::start_system_stats_reporter(Some(interval_secs)),
            );
        } else {
            tracing::info!("System stats reporter is disabled");
        }

        self.background_tasks.push(
            "rate-limit-cleanup",
            crate::middleware::rate_limit::start_cleanup_task(),
        );

        // Initialize auth caches. Every auth cache in the process (token,
        // blacklist, roles, permissions, RBAC) is sized and switched from the
        // same `batata.core.auth.caching.*` / `batata.core.auth.cache.*` keys.
        batata_auth::service::auth::init_auth_caches(batata_auth::service::auth::AuthCacheConfig {
            token_capacity: configuration.auth_token_cache_capacity(),
            token_ttl_secs: configuration.auth_token_cache_ttl_secs(),
            blacklist_capacity: configuration.auth_blacklist_capacity(),
            blacklist_ttl_secs: configuration.auth_blacklist_ttl_secs(),
            enabled: configuration.auth_caching_enabled(),
            roles_capacity: configuration.auth_roles_cache_capacity(),
            roles_ttl_secs: configuration.auth_roles_cache_ttl_secs(),
            permissions_capacity: configuration.auth_permissions_cache_capacity(),
            permissions_ttl_secs: configuration.auth_permissions_cache_ttl_secs(),
        });
        batata_core::service::grpc_auth::init_grpc_auth_cache(
            configuration.grpc_auth_cache_capacity(),
            configuration.grpc_auth_cache_ttl_secs(),
        );

        info!(
            auth_caching_enabled = configuration.auth_caching_enabled(),
            roles_capacity = configuration.auth_roles_cache_capacity(),
            roles_ttl_secs = configuration.auth_roles_cache_ttl_secs(),
            permissions_capacity = configuration.auth_permissions_cache_capacity(),
            permissions_ttl_secs = configuration.auth_permissions_cache_ttl_secs(),
            "Auth caches initialized from batata.core.auth.caching.* / batata.core.auth.cache.*"
        );

        self.configuration = Some(Arc::new(configuration));
        self.otel_config = Some(otel_config);
        self._logging_guard = Some(logging_guard);

        info!("Phase 1 complete - configuration loaded, logging initialized");
        Ok(self)
    }

    // ========================================================================
    // Phase 2: Determine deployment mode & register plugins
    // ========================================================================

    /// Determine deployment mode and register plugins
    pub async fn with_plugins(mut self) -> Result<Self, Box<dyn std::error::Error>> {
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

        #[cfg(any(feature = "consul", feature = "apollo"))]
        let mut plugin_manager = batata_plugin::spi::PluginManager::new();
        #[cfg(not(any(feature = "consul", feature = "apollo")))]
        let plugin_manager = batata_plugin::spi::PluginManager::new();

        #[cfg(feature = "consul")]
        {
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

            self.consul_plugin_ref = consul_plugin_ref;
        }

        #[cfg(feature = "apollo")]
        {
            let apollo_config =
                batata_plugin_apollo::ApolloPluginConfig::from_config(&config.config);
            let apollo_plugin = Arc::new(batata_plugin_apollo::ApolloPlugin::from_plugin_config(
                apollo_config,
            ));

            if apollo_plugin.is_enabled() {
                plugin_manager.register_protocol_adapter(apollo_plugin.clone());
                info!("Apollo plugin registered and enabled");
            } else {
                info!("Apollo plugin disabled by configuration");
            }

            self.apollo_plugin_ref = Some(apollo_plugin);
        }

        // Collect plugin CFs before creating RocksDB
        let plugin_cf_names = plugin_manager.collect_plugin_column_families();

        // Wrap once here. Later phases register built-in plugins through
        // `Arc::get_mut` before this Arc is shared with AppState.
        self.plugin_manager = Some(Arc::new(plugin_manager));
        self.plugin_cf_names = plugin_cf_names;

        info!("Phase 2 complete - plugins registered");
        Ok(self)
    }

    // ========================================================================
    // Phase 3: Initialize persistence layer
    // ========================================================================

    /// Initialize persistence layer
    pub async fn with_persistence(mut self) -> Result<Self, Box<dyn std::error::Error>> {
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
    pub async fn with_services(mut self) -> Result<Self, Box<dyn std::error::Error>> {
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

        // Plugin state providers (consul + apollo adapters expose their
        // runtime state through the `/v3/admin/core/state` endpoint).
        // `mut` is only needed when a consul/apollo feature pushes a provider;
        // keep this attribute so --no-default-features builds stay warning-free.
        #[allow(unused_mut)]
        let mut plugin_state_providers: Vec<Arc<dyn batata_plugin::PluginStateProvider>> =
            Vec::new();
        #[cfg(feature = "consul")]
        if let Some(plugin) = self.consul_plugin_ref.clone() {
            plugin_state_providers.push(plugin as Arc<dyn batata_plugin::PluginStateProvider>);
        }
        #[cfg(feature = "apollo")]
        if let Some(plugin) = self.apollo_plugin_ref.clone() {
            plugin_state_providers.push(plugin as Arc<dyn batata_plugin::PluginStateProvider>);
        }

        // Console datasource
        #[cfg(feature = "consul")]
        let ds_plugins: Vec<Arc<dyn batata_plugin::PluginStateProvider>> = {
            let consul_for_ds = self.consul_plugin_ref.clone();
            if let Some(ref c) = consul_for_ds {
                vec![c.clone() as Arc<dyn batata_plugin::PluginStateProvider>]
            } else {
                Vec::new()
            }
        };
        #[cfg(not(feature = "consul"))]
        let ds_plugins: Vec<Arc<dyn batata_plugin::PluginStateProvider>> = Vec::new();

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
        let health_check_manager: Option<Arc<HealthCheckManager>> = if let Some(ref ns) =
            naming_service_concrete
        {
            let health_check_config = Arc::new(HealthCheckConfig {
                heartbeat_interval_secs: config.naming_heartbeat_check_interval_secs(),
                ttl_monitor_interval_secs: config.naming_ttl_monitor_interval_secs(),
                deregister_monitor_interval_secs: config.naming_deregister_monitor_interval_secs(),
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

        // Auth data service
        //
        // Backs both the local auth plugin and the `/v3/auth/**` management
        // handlers (user, role, permission CRUD) that the console serves to its
        // own UI — in a separated deployment those handlers run in the console
        // process, so they need the remote backend just as much as the plugin
        // does.
        //
        // The plugin always runs in this process, so token validation and the
        // RBAC decision stay local and are made with the real end-user identity.
        // Only the user/role/permission *data* it reads switches backend with
        // the deployment mode, mirroring the split Nacos makes between
        // `NacosUserServiceDirectImpl` and `NacosUserServiceRemoteImpl`:
        //
        //   - merged / server -> the local persistence service
        //   - console         -> HTTP to the core this console fronts
        //
        // The console must never depend on the database, raft or rocksdb, and it
        // must not delegate the authorization decision itself: proxied requests
        // carry the server identity, so the core cannot re-check the real user.
        let ldap_config = if config.auth_system_type() == "ldap" {
            Some(config.ldap_config())
        } else {
            None
        };

        let auth_data: Option<Arc<dyn batata_auth::service::AuthDataService>> = if self
            .is_console_remote
        {
            let base = batata_auth::service::RemoteAuthDataService::base_from_addrs(
                &config.resolve_remote_server_addrs(),
                &config.server_context_path(),
            );

            match batata_auth::service::RemoteAuthDataService::new(
                &base,
                config.server_identity_key(),
                config.server_identity_value(),
                config.console_remote_connect_timeout_ms(),
                config.console_remote_read_timeout_ms(),
            ) {
                Ok(remote) => {
                    if config.server_identity_key().is_empty() {
                        tracing::warn!(
                            "Console-remote auth data reads need a configured server identity \
                                 (batata.core.auth.server.identity.key/value); the core will reject \
                                 them and guarded console APIs will fail closed."
                        );
                    }

                    Some(Arc::new(remote))
                }
                Err(e) => {
                    error!(
                        "Console-remote mode requires a working remote auth data service, but \
                             it failed to initialize: {}. Guarded console APIs will return 403 \
                             until resolved (check batata.console.remote.server_addr).",
                        e
                    );
                    None
                }
            }
        } else {
            persistence_ctx.persistence.as_ref().map(|p| {
                Arc::new(batata_auth::service::PersistenceAuthDataService::new(
                    p.clone(),
                )) as Arc<dyn batata_auth::service::AuthDataService>
            })
        };

        let auth_plugin: Option<Arc<dyn batata_common::AuthPlugin>> =
            auth_data.as_ref().map(|data| {
                batata_auth::plugin::create_auth_plugin(
                    &config.auth_system_type(),
                    config.token_secret_key(),
                    config.auth_token_expire_seconds(),
                    data.clone(),
                    ldap_config.clone(),
                )
            });

        // Clone Arc fields before moving into AppState
        let console_datasource_for_self = console_datasource.clone();
        let oauth_service_for_self = oauth_service.clone();
        let control_plugin_for_self = control_plugin.clone();
        let auth_plugin_for_self = auth_plugin.clone();

        // Register built-in plugins on the same manager created in Phase 2.
        // `Arc::get_mut` is valid here because AppState has not taken its
        // reference yet; after this point the manager is immutable and shared.
        {
            let shared = self
                .plugin_manager
                .as_mut()
                .expect("plugin manager must be created before services");
            let plugin_manager = Arc::get_mut(shared)
                .expect("plugin manager must not be shared before built-in registration");

            // Register audit plugin
            plugin_manager.register_config_change_v2(Arc::new(batata_plugin::AuditPlugin::new(
                Arc::new(batata_plugin::InMemoryAuditLogStore::new(10000)),
            )));
            // Register webhook config change plugin (bridges config changes to webhook delivery)
            let webhook_plugin: Arc<dyn batata_plugin::WebhookPlugin> =
                Arc::new(batata_plugin::DefaultWebhookPlugin::new());
            plugin_manager.register_config_change_v2(Arc::new(
                batata_plugin::WebhookConfigChangePlugin::new(webhook_plugin),
            ));
        }
        let plugin_manager = self
            .plugin_manager
            .clone()
            .expect("plugin manager must be created before services");

        // Build app state
        let app_state = Arc::new(AppState {
            configuration: config.as_ref().clone(),
            cluster_manager: persistence_ctx.cluster_manager.clone(),
            config_subscriber_manager: config_subscriber_manager.clone(),
            console_datasource,
            oauth_service,
            auth_plugin,
            persistence: persistence_ctx.persistence.clone(),
            auth_data,
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
            plugin_manager: Some(plugin_manager),
            log_level_setter: Some(Arc::new(|filter: &str| {
                crate::startup::logging::set_log_level(filter)
            })),
            connection_manager: Arc::new(std::sync::OnceLock::new()),
        });

        // Stay STARTING until the final startup barrier in Phase 12 (or the
        // console-remote barrier in Phase 6). The gRPC status gate relies on
        // this: UP must not be published before Raft, plugins and listeners
        // are ready to serve traffic.
        info!("Server status: STARTING (waiting for startup completion)");

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
    pub async fn with_shutdown_and_ai(mut self) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 5] Initializing shutdown handler and AI services...");

        let app_state = self.app_state.as_ref().unwrap();
        let naming_service = self.naming_service.clone();

        let drain_timeout_secs = app_state.configuration.shutdown_drain_timeout_secs();
        let shutdown_signal = startup::wait_for_shutdown_signal().await;
        let graceful_shutdown = GracefulShutdown::new(
            shutdown_signal.clone(),
            Duration::from_secs(drain_timeout_secs),
        );

        let ai_services = match (&app_state.persistence, &naming_service) {
            (Some(persist), Some(ns)) => {
                info!("AI services using config-backed persistence");
                AIServices::with_persistence(
                    persist.clone(),
                    ns.clone(),
                    app_state.configuration.typed.ai.rad.publication_limit(),
                )
                .with_ai_resource_services(
                    persist.clone(),
                    app_state.auth_plugin.clone(),
                    app_state.configuration.auth_enabled(),
                )
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
        let database_connection = self
            .persistence_ctx
            .as_ref()
            .unwrap()
            .database_connection
            .clone();

        run_console_remote(
            app_state.clone(),
            ai_services,
            graceful_shutdown,
            server_status,
            database_connection,
            &self.startup_complete,
            &self.timeline,
        )
        .await?;

        Ok(Some(()))
    }

    // ========================================================================
    // Phase 7: Start gRPC servers
    // ========================================================================

    /// Start gRPC servers
    pub async fn with_grpc_servers(mut self) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 7] Starting gRPC servers...");

        let app_state = self.app_state.as_ref().unwrap();
        let ai_services = self.ai_services.as_mut().unwrap();
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
        if let (Some(raft), Some(ns)) = (&persistence_ctx.raft_node, &self.naming_service_concrete)
        {
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

        // Expose the gRPC connection manager to AppState so admin naming
        // handlers can push instance-change notifications to SDK subscribers.
        if let Some(app_state) = &self.app_state {
            let _ = app_state
                .connection_manager
                .set(grpc_servers.connection_manager().clone());
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
    pub async fn with_background_tasks(self) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 8] Starting background tasks...");

        let app_state = self.app_state.as_ref().unwrap();
        let grpc_servers = self.grpc_servers.as_ref().unwrap();
        let persistence_ctx = self.persistence_ctx.as_ref().unwrap();
        let ai_services = self.ai_services.as_ref().unwrap();
        let server_status = self.server_status.as_ref().unwrap();

        // Upgrade health check to cluster mode if not standalone
        if !app_state.configuration.is_standalone() {
            if let Some(hc_manager) = self.health_check_manager.as_ref() {
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
        }

        // Start health checkers
        start_health_checkers(
            &self.health_check_manager,
            &self.naming_service_concrete,
            self.server_status.as_ref(),
            &self.background_tasks,
        );

        // Start warmup poller
        start_warmup_poller(
            app_state,
            server_status,
            persistence_ctx.server_member_manager.as_ref(),
            self.startup_complete.clone(),
            &self.background_tasks,
        );

        // Start MCP index refresh
        start_mcp_index_refresh(ai_services, app_state, &self.background_tasks);

        // Start the AI resource search index consumer
        start_ai_search_index_consumer(app_state, &self.background_tasks);

        info!("Phase 8 complete - background tasks started");
        Ok(self)
    }

    // ========================================================================
    // Phase 9: Cluster and Raft initialization
    // ========================================================================

    /// Initialize cluster and Raft
    pub async fn with_cluster_init(self) -> Result<Self, Box<dyn std::error::Error>> {
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
    pub async fn with_xds(mut self) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 10] Starting xDS service mesh support...");

        let app_state = self.app_state.as_ref().unwrap();
        let grpc_servers = self.grpc_servers.as_ref().unwrap();

        let xds_handle = start_xds_if_enabled(app_state, grpc_servers).await;
        self.xds_handle = xds_handle;

        let istio_mcp_handle = start_istio_mcp_if_enabled(app_state, grpc_servers).await?;
        self.istio_mcp_handle = istio_mcp_handle;

        info!("Phase 10 complete - xDS and Istio MCP services initialized");
        Ok(self)
    }

    // ========================================================================
    // Phase 11: Initialize protocol adapter plugins
    // ========================================================================

    /// Validate the typed dependencies required by built-in protocol adapters.
    ///
    /// This is a producer-side gate: a missing or wrong-typed dependency is a
    /// startup error rather than a silent runtime downgrade in an adapter.
    fn validate_plugin_context(
        ctx: &batata_plugin::PluginContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            dependencies = ?ctx.describe(),
            "Validating plugin context dependencies"
        );

        let is_cluster = *ctx.get_required::<bool>(context_keys::IS_CLUSTER)?;
        let storage_mode = *ctx
            .get_required::<batata_persistence::model::StorageMode>(context_keys::STORAGE_MODE)?;

        if storage_mode == batata_persistence::model::StorageMode::ExternalDb {
            ctx.get_required::<sea_orm::DatabaseConnection>(context_keys::DATABASE)?;
        } else {
            ctx.get_required::<rocksdb::DB>(context_keys::ROCKS_DB)?;
        }

        if is_cluster {
            ctx.get_required::<batata_consistency::RaftNode>(context_keys::RAFT_NODE)?;
            ctx.get_shared_required::<dyn batata_common::ClusterManager>(
                context_keys::CLUSTER_MANAGER_TYPED,
            )?;
            ctx.get_required::<batata_core::service::distro::DistroProtocol>(
                context_keys::DISTRO_PROTOCOL,
            )?;
        }

        info!("Plugin context dependencies validated");
        Ok(())
    }

    /// Initialize protocol adapter plugins
    pub async fn with_plugin_init(self) -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Phase 11] Initializing protocol adapter plugins...");

        let app_state = self.app_state.as_ref().unwrap();
        let grpc_servers = self.grpc_servers.as_ref().unwrap();
        let persistence_ctx = self.persistence_ctx.as_ref().unwrap();
        let plugin_manager = self.plugin_manager.as_ref().unwrap();
        let is_cluster = !app_state.configuration.is_standalone() && !self.is_console_remote;

        // Build plugin context. All known keys come from `context_keys` so the
        // producer and the adapters cannot drift on string literals.
        let mut plugin_ctx = batata_plugin::PluginContext::new();
        plugin_ctx.insert(context_keys::IS_CLUSTER, Arc::new(is_cluster));

        // Inject storage mode and database connection for plugins that need them (e.g. Apollo)
        let storage_mode = app_state.configuration.persistence_mode();
        plugin_ctx.insert(context_keys::STORAGE_MODE, Arc::new(storage_mode));
        if let Some(ref db) = persistence_ctx.database_connection {
            plugin_ctx.insert(context_keys::DATABASE, Arc::new(db.clone()));
        }
        if let Some(ref rdb) = persistence_ctx.rocks_db {
            plugin_ctx.insert(context_keys::ROCKS_DB, rdb.clone());
        }
        if let Some(ref raft) = app_state.raft_node {
            plugin_ctx.insert(context_keys::RAFT_NODE, raft.clone());
        }
        if let Some(ref cm) = persistence_ctx.cluster_manager {
            let cm = cm.clone();
            // Keep the legacy double-Arc key for older readers while exposing
            // a normalized single-Arc value to typed consumers. Any downcasts
            // need a sized type, hence the generic SharedDependency wrapper.
            plugin_ctx.insert(context_keys::CLUSTER_MANAGER, Arc::new(cm.clone()));
            plugin_ctx.insert_shared(context_keys::CLUSTER_MANAGER_TYPED, cm);
        }
        if is_cluster {
            plugin_ctx.insert(
                context_keys::DISTRO_PROTOCOL,
                grpc_servers.distro_protocol().clone(),
            );
        }

        Self::validate_plugin_context(&plugin_ctx)?;

        // Initialize all plugins
        plugin_manager.init_protocol_adapters(&plugin_ctx).await?;

        // Wire Consul event broadcast (cluster mode only)
        #[cfg(feature = "consul")]
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

        // Wire cross-node auth cache invalidation (cluster mode only).
        //
        // Role/permission writes evict the local RBAC caches; without this the
        // peers would keep serving stale authorizations until their entries hit
        // the TTL. Broadcasts are fire-and-forget: an unreachable peer simply
        // falls back to TTL expiry.
        if let Some(ref client_manager) = *grpc_servers.cluster_client_manager()
            && let Some(ref smm) = persistence_ctx.server_member_manager
        {
            let broadcaster = Arc::new(
                batata_core::service::auth_cache_broadcast::ClusterAuthCacheBroadcaster::new(
                    Arc::new(
                        batata_core::service::cluster_client::ClusterRequestSender::new(
                            client_manager.clone(),
                        ),
                    ),
                    smm.clone(),
                ),
            );

            let broadcast_tasks = self.background_tasks.clone();
            batata_auth::service::rbac_cache::set_peer_invalidator(Arc::new(
                move |kind: &str, target: &str| {
                    let broadcaster = broadcaster.clone();
                    let kind = kind.to_owned();
                    let target = target.to_owned();
                    broadcast_tasks.spawn("auth-cache-broadcast", async move {
                        broadcaster.broadcast(&kind, &target).await
                    });
                },
            ));

            info!("Auth cache cluster invalidation enabled");
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
        let plugin_manager = app_state
            .plugin_manager
            .as_ref()
            .expect("shared plugin manager must be present in AppState");
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
            &self.startup_complete,
            self.persistence_ctx
                .as_ref()
                .and_then(|ctx| ctx.server_member_manager.as_ref()),
            &self.timeline,
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
        let grpc_servers = self.grpc_servers.as_mut().unwrap();
        let xds_handle = self.xds_handle.take();
        let istio_mcp_handle = self.istio_mcp_handle.take();
        let persistence_ctx = self.persistence_ctx.as_ref().unwrap();
        let server_registry = self.server_registry.as_ref().unwrap();

        graceful_shutdown_sequence(
            server_status,
            graceful_shutdown,
            app_state,
            grpc_servers,
            xds_handle,
            istio_mcp_handle,
            persistence_ctx.server_member_manager.as_ref(),
            persistence_ctx.database_connection.clone(),
            server_registry,
            &self.background_tasks,
            self.health_check_manager.as_ref(),
            &self.timeline,
        )
        .await;

        info!("Phase 13 complete - graceful shutdown finished");
        let total_ms = self.timeline.record_total(LifecycleKind::Shutdown);
        let phases_recorded = self.timeline.snapshot().len();
        info!(
            phase = "shutdown.total",
            duration_ms = total_ms,
            phases = phases_recorded,
            "{}",
            shutdown_summary(total_ms)
        );
    }

    // ========================================================================
    // Run the complete server lifecycle
    // ========================================================================

    /// Run the complete server lifecycle (all phases)
    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        // The builder owns the timeline so helpers started inside a phase can
        // keep recording after `self` is moved into the next phase.
        let seed = AppBuilder::new();
        let timeline = seed.timeline.clone();

        macro_rules! startup_phase {
            ($phase:expr, $future:expr) => {{
                let mut timer = timeline.begin($phase, LifecycleKind::Startup);
                finish_startup_phase(&mut timer, $future.await)?
            }};
        }

        let builder = startup_phase!(phases::CONFIG, seed.with_config_and_logging());
        let builder = startup_phase!(phases::PLUGINS_REGISTER, builder.with_plugins());
        let builder = startup_phase!(phases::PERSISTENCE, builder.with_persistence());
        let builder = startup_phase!(phases::SERVICES, builder.with_services());
        let builder = startup_phase!(phases::SHUTDOWN_AI, builder.with_shutdown_and_ai());

        // Check for console-remote early return
        if startup_phase!(phases::CONSOLE_REMOTE, builder.check_console_remote()).is_some() {
            info!("Batata server shutdown complete (console-remote mode)");
            return Ok(());
        }

        let builder = startup_phase!(phases::GRPC, builder.with_grpc_servers());
        let builder = startup_phase!(phases::BACKGROUND_TASKS, builder.with_background_tasks());
        let builder = startup_phase!(phases::CLUSTER_INIT, builder.with_cluster_init());
        let builder = startup_phase!(phases::XDS, builder.with_xds());
        let mut builder = startup_phase!(phases::PLUGINS_INIT, builder.with_plugin_init());

        // `run_http_servers` spans the whole serving lifetime, so its own
        // internal timers separate bind, drain and shutdown from uptime.
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

/// Validate mandatory configuration.
///
/// Startup failures unwind through `AppBuilder::run()` like every other phase
/// instead of calling `process::exit()` from library code. The process still
/// exits nonzero via `main`, but guards and buffered state can unwind normally.
fn validate_config(configuration: &Configuration) -> Result<(), Box<dyn std::error::Error>> {
    if configuration.auth_enabled() && configuration.token_secret_key().is_empty() {
        eprintln!(
            "FATAL: Authentication is enabled (batata.core.auth.enabled=true) but no JWT secret key is configured."
        );
        eprintln!(
            "Set 'batata.core.auth.plugin.default.token.secret.key' to a non-empty Base64-encoded secret."
        );
        return Err(concat!(
            "authentication enabled without JWT secret key: ",
            "batata.core.auth.plugin.default.token.secret.key"
        )
        .into());
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

    Ok(())
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

    let rule_file = configuration.control_rule_file();
    let rule_store = match batata_plugin::control::create_rule_store(
        &control_config.storage_type,
        Some(&rule_file),
    )
    .await
    {
        Ok(store) => store,
        Err(e) => {
            tracing::warn!("Failed to load control rules from {}: {}", rule_file, e);
            return None;
        }
    };

    let plugin =
        batata_plugin::DefaultControlPlugin::new(control_config).with_rule_store(rule_store);

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
fn start_health_checkers(
    health_check_manager: &Option<Arc<HealthCheckManager>>,
    naming_service: &Option<Arc<batata_naming::NamingService>>,
    server_status: Option<&Arc<ServerStatusManager>>,
    tasks: &BackgroundTasks,
) {
    if let Some(hc_manager) = health_check_manager {
        let hc1 = hc_manager.clone();
        tasks.spawn("unhealthy-instance-checker", async move {
            hc1.unhealthy_checker().start().await
        });

        let hc2 = hc_manager.clone();
        tasks.spawn("expired-instance-checker", async move {
            hc2.expired_checker().start().await
        });

        info!("Health check manager started (unhealthy/expired instance checkers)");
    }

    // Reclaim services that hold no instances — the batata counterpart of Nacos
    // `EmptyServiceAutoCleanerV2`. Without it every service ever created stays resident
    // and Distro keeps re-syncing empty ones indefinitely.
    if let Some(ns) = naming_service {
        let cleaner = batata_naming::service::EmptyServiceCleaner::new(ns.clone());
        // Stay idle until the node is UP so warmup / Distro loading is not reaped.
        let cleaner = match server_status {
            Some(status) => cleaner.with_status(status.clone()),
            None => cleaner,
        };
        tasks.spawn(
            "empty-service-cleaner",
            async move { cleaner.start().await },
        );
        info!("Empty service cleaner started");
    }
}

/// Start the server-status refresher.
///
/// Runs every 5s and derives the live `ServerStatus` from subsystem readiness
/// (db + raft + distro), mirroring Nacos `ServerStatusManager.refreshServerStatus()`.
///
/// Nacos parity for the operator override: if `overriddenServerStatus` is set, the
/// refresher adopts that value verbatim and returns *before* the readiness computation —
/// `if (isNotBlank(overriddenServerStatus)) { status = value; return; }`. Clearing the
/// override (via `PUT /v3/admin/core/state` with `overriddenServerStatus=null`) makes the
/// refresher revert to computing UP/DOWN again.
///
/// When data warmup is disabled the readiness computation short-circuits to UP, matching
/// Nacos `isReady()` which returns `true` when `!isDataWarmup()`.
#[derive(Debug, PartialEq, Eq)]
enum ReadinessDecision {
    Up,
    Down(Vec<String>),
}

/// Pure readiness computation shared by the poller and the final startup
/// barrier. Keeping it pure makes the DB/Raft/Distro composition testable
/// without binding ports or opening RocksDB.
fn readiness_decision(
    data_warmup: bool,
    db_ready: bool,
    raft_ready: bool,
    raft_reason: Option<String>,
    distro_ready: bool,
) -> ReadinessDecision {
    if !data_warmup {
        return ReadinessDecision::Up;
    }

    let mut reasons = Vec::new();
    if !db_ready {
        reasons.push("database not ready".to_string());
    }
    if !raft_ready {
        reasons.push(raft_reason.unwrap_or_else(|| "raft not ready".to_string()));
    }
    if !distro_ready {
        reasons.push("distro initial data not loaded".to_string());
    }

    if reasons.is_empty() {
        ReadinessDecision::Up
    } else {
        ReadinessDecision::Down(reasons)
    }
}

/// Recompute server status from the live subsystem state.
///
/// Returns the resulting `is_up()` so the final startup barrier can log the
/// same signal the poller will continue to maintain.
#[allow(clippy::too_many_arguments)]
async fn refresh_server_status(
    status_mgr: &Arc<ServerStatusManager>,
    console_ds: &Arc<dyn batata_console::ConsoleDataSource>,
    raft_ref: &Option<Arc<batata_consistency::RaftNode>>,
    is_standalone: bool,
    server_member_manager: Option<&Arc<batata_core::cluster::ServerMemberManager>>,
    data_warmup: bool,
    startup_complete: &AtomicBool,
) -> bool {
    // Operator override wins over everything (Nacos parity).
    if let Some(overridden) = status_mgr.overridden() {
        status_mgr.set_overridden(overridden);
        status_mgr.set_error_msg(None).await;
        return status_mgr.is_up();
    }

    // Before the final barrier the process is intentionally STARTING. The
    // gRPC gate uses this to reject traffic while Raft/plugins/listeners are
    // still coming up, matching Nacos `ApplicationUtils.isStarted()`.
    if !startup_complete.load(Ordering::Acquire) {
        return status_mgr.is_up();
    }

    let db_ready = console_ds.server_readiness().await;
    let (raft_ready, raft_reason) = match raft_ref {
        Some(raft) => raft.is_ready(),
        None => (true, None),
    };
    let distro_ready = if is_standalone {
        true
    } else if let Some(smm) = server_member_manager {
        smm.is_distro_initialized().await
    } else {
        true
    };

    match readiness_decision(data_warmup, db_ready, raft_ready, raft_reason, distro_ready) {
        ReadinessDecision::Up => {
            if !status_mgr.is_up() {
                status_mgr.set_up();
                info!("Server status: UP (all subsystems ready)");
            }
            status_mgr.set_error_msg(None).await;
        }
        ReadinessDecision::Down(reasons) => {
            status_mgr.set_down();
            status_mgr.set_error_msg(Some(reasons.join(", "))).await;
        }
    }

    status_mgr.is_up()
}

/// Publish the final startup barrier and immediately evaluate status once so
/// readiness does not wait for the next 5s poller tick.
async fn complete_startup(
    app_state: &Arc<AppState>,
    server_member_manager: Option<&Arc<batata_core::cluster::ServerMemberManager>>,
    startup_complete: &AtomicBool,
    timeline: &LifecycleTimeline,
) {
    let mut barrier = timeline.begin(phases::STARTUP_BARRIER, LifecycleKind::Startup);
    startup_complete.store(true, Ordering::Release);
    let up = refresh_server_status(
        &app_state.server_status,
        &app_state.console_datasource,
        &app_state.raft_node,
        app_state.configuration.is_standalone(),
        server_member_manager,
        app_state.configuration.data_warmup(),
        startup_complete,
    )
    .await;
    barrier.ok();
    info!(up, "Final startup barrier complete");

    // Nacos parity: log one total cost with deployment mode and storage after
    // the readiness barrier, not when the listener bind merely returns.
    let total_ms = timeline.record_total(LifecycleKind::Startup);
    let mode = mode_label(app_state.configuration.is_standalone());
    let storage = storage_label(app_state.configuration.persistence_mode());
    info!(
        phase = "startup.total",
        duration_ms = total_ms,
        mode = mode,
        storage = storage,
        up = up,
        "{}",
        startup_summary(mode, storage, total_ms)
    );
}

fn start_warmup_poller(
    app_state: &Arc<AppState>,
    server_status: &Arc<ServerStatusManager>,
    server_member_manager: Option<&Arc<batata_core::cluster::ServerMemberManager>>,
    startup_complete: Arc<AtomicBool>,
    tasks: &BackgroundTasks,
) {
    let data_warmup = app_state.configuration.data_warmup();

    let status_mgr = server_status.clone();
    let console_ds = app_state.console_datasource.clone();
    let raft_ref = app_state.raft_node.clone();
    let is_standalone = app_state.configuration.is_standalone();
    let smm_clone = server_member_manager.cloned();

    tasks.spawn("warmup-status-poller", async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;
            refresh_server_status(
                &status_mgr,
                &console_ds,
                &raft_ref,
                is_standalone,
                smm_clone.as_ref(),
                data_warmup,
                &startup_complete,
            )
            .await;
        }
    });

    info!("Data warmup poller started (checking every 5s)");
}

/// Start periodic MCP index refresh if persistence is available.
fn start_mcp_index_refresh(
    ai_services: &AIServices,
    app_state: &Arc<AppState>,
    tasks: &BackgroundTasks,
) {
    if let Some(ref mcp_index) = ai_services.mcp_index
        && let Some(ref persist) = app_state.persistence
    {
        let index = mcp_index.clone();
        let persist = persist.clone();

        tasks.spawn("mcp-index-refresh", async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                index.refresh(persist.as_ref()).await;
            }
        });

        info!("MCP index periodic refresh task started (every 30s)");
    }
}

/// Start the AI resource search-index consumer.
///
/// Polls durable `search_index` tasks and converges the relational index.
/// Skipped when there is no local persistence (remote console deployments).
fn start_ai_search_index_consumer(app_state: &Arc<AppState>, tasks: &BackgroundTasks) {
    let Some(ref persist) = app_state.persistence else {
        return;
    };

    let consumer = batata_ai::search::consumer::AiResourceIndexConsumer::new(persist.clone());
    let interval_secs = batata_ai::search::consumer::DEFAULT_INTERVAL_SECONDS;

    tasks.spawn("ai-search-index-consumer", async move {
        let mut timer = tokio::time::interval(Duration::from_secs(interval_secs));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            timer.tick().await;
            // A failed poll must not kill the loop; the next tick retries.
            if let Err(e) = consumer.consume_once().await {
                warn!(error = %e, "AI resource search index poll failed");
            }
        }
    });

    info!(
        "AI resource search index consumer started (every {}s)",
        interval_secs
    );
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

/// Start the Istio MCP server if enabled.
async fn start_istio_mcp_if_enabled(
    app_state: &Arc<AppState>,
    grpc_servers: &startup::GrpcServers,
) -> Result<Option<IstioMcpServerHandle>, Box<dyn std::error::Error>> {
    let configuration = &app_state.configuration;
    if !configuration.istio_mcp_server_enabled() {
        return Ok(None);
    }

    let port = configuration.istio_mcp_server_port();
    let service_entry_port = configuration.istio_mcp_service_entry_port();
    let domain_suffix = configuration.istio_domain_suffix().to_string();
    info!(
        port = port,
        service_entry_port = service_entry_port,
        domain_suffix = %domain_suffix,
        "Starting Istio MCP server"
    );
    let Some(persistence) = app_state.persistence.clone() else {
        return Err("Istio MCP server requires a persistence service".into());
    };
    let config_provider = Arc::new(PersistenceIstioMcpConfigProvider::new(persistence));
    match start_istio_mcp_service(
        true,
        port,
        service_entry_port,
        domain_suffix,
        grpc_servers.naming_provider(),
        config_provider,
        grpc_servers.config_change_notifier().clone(),
    )
    .await
    {
        Ok(handle) => Ok(handle),
        Err(error) => Err(Box::new(std::io::Error::other(error.to_string()))),
    }
}

/// Run in console-remote mode (only console HTTP server).
async fn run_console_remote(
    app_state: Arc<AppState>,
    ai_services: AIServices,
    graceful_shutdown: GracefulShutdown,
    server_status: Arc<ServerStatusManager>,
    database_connection: Option<sea_orm::DatabaseConnection>,
    startup_complete: &AtomicBool,
    timeline: &LifecycleTimeline,
) -> Result<(), Box<dyn std::error::Error>> {
    let console_server_port = app_state.configuration.console_server_port();
    let console_context_path = app_state.configuration.console_server_context_path();
    let console_server_address = app_state.configuration.server_address();

    info!(
        "Starting console server in remote mode on port {}",
        console_server_port
    );

    let console_server = startup::console_server(
        app_state.clone(),
        None,
        ai_services,
        console_context_path,
        console_server_address,
        console_server_port,
    )?;

    // Console-remote has no Raft/plugins, so the listener bind is the final
    // barrier. This also fixes the historical mode that never set UP at all.
    complete_startup(&app_state, None, startup_complete, timeline).await;

    tokio::select! {
        result = console_server => {
            timeline.mark_shutdown();
            if let Err(e) = result {
                error!("Console server error: {}", e);
            }
        }
        _ = graceful_shutdown.wait_for_shutdown() => {
            timeline.mark_shutdown();
            info!("Console server shutting down gracefully");
        }
    }

    server_status.set_down();

    if let Some(db) = database_connection {
        let _ = tokio::time::timeout(Duration::from_secs(5), db.close()).await;
    }

    let total_ms = timeline.record_total(LifecycleKind::Shutdown);
    info!(
        phase = "shutdown.total",
        duration_ms = total_ms,
        "{}",
        shutdown_summary(total_ms)
    );

    Ok(())
}

struct ManagedHttpServer {
    name: &'static str,
    state: batata_core::ServerStateTracker,
    handle: actix_web::dev::ServerHandle,
}

/// Spawn an actix server as a tracked task and register its state so shutdown
/// can observe the transition Running → Draining → Stopped.
fn register_http_server(
    name: &'static str,
    server: actix_web::dev::Server,
    state: batata_core::ServerStateTracker,
    server_registry: &Arc<batata_core::ServerRegistry>,
    tasks: &mut tokio::task::JoinSet<()>,
) -> ManagedHttpServer {
    let handle = server.handle();
    state.set_running();
    server_registry.register(name, batata_core::ServerType::Http, state.clone());
    tasks.spawn(async move {
        if let Err(error) = server.await {
            error!(name, error = %error, "HTTP server stopped with error");
        }
    });
    ManagedHttpServer {
        name,
        state,
        handle,
    }
}

/// Stop every HTTP listener gracefully and wait for their tasks to exit.
///
/// This replaces the old fixed sleep: `ServerHandle::stop(true)` stops
/// accepting connections and waits for in-flight requests, and the task join
/// is bounded by the configured drain timeout.
async fn drain_http_servers(
    servers: Vec<ManagedHttpServer>,
    tasks: &mut tokio::task::JoinSet<()>,
    timeout: Duration,
) {
    let deadline = tokio::time::Instant::now() + timeout;

    for server in &servers {
        server.state.set_draining();
    }

    let states: Vec<_> = servers
        .iter()
        .map(|server| (server.name, server.state.clone()))
        .collect();

    let mut stop_tasks = tokio::task::JoinSet::new();
    for server in servers {
        stop_tasks.spawn(async move {
            server.handle.stop(true).await;
            server.state.set_stopped();
            info!(server = server.name, "HTTP server stopped gracefully");
        });
    }

    let stop_drain = async { while stop_tasks.join_next().await.is_some() {} };
    if tokio::time::timeout_at(deadline, stop_drain).await.is_err() {
        warn!("HTTP graceful stop did not complete before the drain deadline");
        stop_tasks.abort_all();
        for (name, state) in states {
            state.set_stopped();
            warn!(server = name, "HTTP server forced to Stopped after timeout");
        }
    }

    let task_drain = async { while tasks.join_next().await.is_some() {} };
    if tokio::time::timeout_at(deadline, task_drain).await.is_err() {
        warn!("HTTP server tasks did not exit before the drain deadline");
        tasks.abort_all();
    }
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
    plugin_manager: &Arc<PluginManager>,
    graceful_shutdown: &GracefulShutdown,
    server_registry: &Arc<batata_core::ServerRegistry>,
    startup_complete: &AtomicBool,
    server_member_manager: Option<&Arc<batata_core::cluster::ServerMemberManager>>,
    timeline: &LifecycleTimeline,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bind_timer = timeline.begin(phases::HTTP_BIND, LifecycleKind::Startup);
    let server_address = app_state.configuration.server_address();
    let server_main_port = app_state.configuration.server_main_port();
    let server_context_path = app_state.configuration.server_context_path();
    let mcp_registry_enabled = app_state.configuration.mcp_registry_enabled()
        || matches!(deployment_mode, DeploymentMode::ServerWithMcp);
    let mcp_registry_port = app_state.configuration.mcp_registry_port();

    let mut server_tasks = tokio::task::JoinSet::new();
    let mut http_servers = Vec::new();

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

            http_servers.push(register_http_server(
                "Console HTTP Server",
                console,
                batata_core::ServerStateTracker::new(),
                server_registry,
                &mut server_tasks,
            ));
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

            http_servers.push(register_http_server(
                "Main HTTP Server",
                main,
                batata_core::ServerStateTracker::new(),
                server_registry,
                &mut server_tasks,
            ));

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
                http_servers.push(register_http_server(
                    "Plugin HTTP Server",
                    server,
                    batata_core::ServerStateTracker::new(),
                    server_registry,
                    &mut server_tasks,
                ));
            }
            if let Some(server) = mcp_registry_opt {
                http_servers.push(register_http_server(
                    "MCP Registry Server",
                    server,
                    batata_core::ServerStateTracker::new(),
                    server_registry,
                    &mut server_tasks,
                ));
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

            http_servers.push(register_http_server(
                "Console HTTP Server",
                console,
                batata_core::ServerStateTracker::new(),
                server_registry,
                &mut server_tasks,
            ));

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
                encryption_service.clone(),
                server_context_path,
                server_address.clone(),
                server_main_port,
                Some(server_registry.clone()),
                grpc_servers.cluster_client_manager().clone(),
            )?;

            http_servers.push(register_http_server(
                "Main HTTP Server",
                main,
                batata_core::ServerStateTracker::new(),
                server_registry,
                &mut server_tasks,
            ));

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
                http_servers.push(register_http_server(
                    "Plugin HTTP Server",
                    server,
                    batata_core::ServerStateTracker::new(),
                    server_registry,
                    &mut server_tasks,
                ));
            }
            if let Some(server) = mcp_registry_opt {
                http_servers.push(register_http_server(
                    "MCP Registry Server",
                    server,
                    batata_core::ServerStateTracker::new(),
                    server_registry,
                    &mut server_tasks,
                ));
            }
        }
    }

    complete_startup(app_state, server_member_manager, startup_complete, timeline).await;
    bind_timer.ok();

    let trigger = tokio::select! {
        Some(result) = server_tasks.join_next() => {
            timeline.mark_shutdown();
            if let Err(error) = result {
                error!(error = %error, "HTTP server task failed");
            }
            "server-exit"
        }
        _ = graceful_shutdown.wait_for_shutdown() => {
            timeline.mark_shutdown();
            info!("All servers shutting down gracefully");
            "shutdown-signal"
        }
    };

    info!(trigger, "Stopping HTTP server group");
    let mut drain_timer = timeline.begin(phases::HTTP_DRAIN, LifecycleKind::Shutdown);
    drain_http_servers(
        http_servers,
        &mut server_tasks,
        graceful_shutdown.drain_timeout(),
    )
    .await;
    drain_timer.ok();

    Ok(())
}

/// Build HTTP servers for all enabled protocol adapter plugins.
#[cfg(feature = "consul")]
fn build_plugin_servers(
    app_state: &Arc<AppState>,
    grpc_servers: &startup::GrpcServers,
    plugin_manager: &Arc<PluginManager>,
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

/// No protocol adapter needs a dedicated HTTP server when consul is disabled.
#[cfg(not(feature = "consul"))]
fn build_plugin_servers(
    _app_state: &Arc<AppState>,
    _grpc_servers: &startup::GrpcServers,
    _plugin_manager: &batata_plugin::spi::PluginManager,
) -> Result<Vec<actix_web::dev::Server>, Box<dyn std::error::Error>> {
    Ok(Vec::new())
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
#[allow(clippy::too_many_arguments)]
async fn graceful_shutdown_sequence(
    server_status: &Arc<ServerStatusManager>,
    graceful_shutdown: &GracefulShutdown,
    app_state: &Arc<AppState>,
    grpc_servers: &mut startup::GrpcServers,
    xds_handle: Option<XdsServerHandle>,
    istio_mcp_handle: Option<IstioMcpServerHandle>,
    server_member_manager: Option<&Arc<batata_core::cluster::ServerMemberManager>>,
    database_connection: Option<sea_orm::DatabaseConnection>,
    server_registry: &Arc<batata_core::ServerRegistry>,
    background_tasks: &BackgroundTasks,
    health_check_manager: Option<&Arc<HealthCheckManager>>,
    timeline: &LifecycleTimeline,
) {
    macro_rules! shutdown_phase {
        ($phase:expr, $body:block) => {{
            let mut timer = timeline.begin($phase, LifecycleKind::Shutdown);
            let result = $body;
            timer.ok();
            result
        }};
    }

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

    // The watch signal and task joins replace the old fixed sleep. Stopped
    // state is published only after each gRPC listener task has exited (or the
    // bounded deadline elapsed), which is the contract in
    // `batata_core::ManagedServer::shutdown`.
    shutdown_phase!(phases::GRPC, {
        grpc_servers.shutdown(drain_timeout).await;
        info!("gRPC servers stopped");
    });

    server_status.set_down();
    info!("Server status: DOWN");

    shutdown_phase!(phases::XDS, {
        if let Some(handle) = xds_handle {
            info!("Stopping xDS service...");
            handle.shutdown().await;
            info!("xDS service stopped");
        }
    });

    shutdown_phase!(phases::ISTIO_MCP, {
        if let Some(handle) = istio_mcp_handle {
            info!("Stopping Istio MCP service...");
            handle.shutdown().await;
            info!("Istio MCP service stopped");
        }
    });

    shutdown_phase!(phases::HEALTH_CHECK, {
        if let Some(manager) = health_check_manager {
            info!("Stopping health check manager...");
            manager.shutdown().await;
            info!("Health check manager stopped");
        }
    });

    shutdown_phase!(phases::PLUGINS_SHUTDOWN, {
        if let Some(plugin_manager) = &app_state.plugin_manager {
            info!("Shutting down protocol adapters...");
            if let Err(error) = plugin_manager.shutdown_protocol_adapters().await {
                warn!(error = %error, "Protocol adapter shutdown reported an error");
            }
            info!("Protocol adapters stopped");
        }
    });

    shutdown_phase!(phases::BACKGROUND_TASKS, {
        info!(
            tracked = background_tasks.len(),
            "Stopping background tasks"
        );
        background_tasks.shutdown(drain_timeout).await;
    });

    shutdown_phase!(phases::CLUSTER_MANAGER, {
        if let Some(smm) = server_member_manager
            && !app_state.configuration.is_standalone()
        {
            info!("Stopping cluster manager...");
            smm.stop().await;
            info!("Cluster manager stopped");
        }
    });

    shutdown_phase!(phases::RAFT, {
        if let Some(ref raft_node) = app_state.raft_node {
            info!("Shutting down Raft node...");
            match tokio::time::timeout(Duration::from_secs(10), raft_node.shutdown()).await {
                Ok(Ok(())) => info!("Raft node shutdown complete"),
                Ok(Err(e)) => error!("Raft node shutdown error: {}", e),
                Err(_) => error!("Raft node shutdown timed out after 10s"),
            }
        }
    });

    shutdown_phase!(phases::DATABASE, {
        if let Some(db) = database_connection {
            let db_timeout = app_state.configuration.shutdown_db_close_timeout_secs();
            info!("Closing database connections (timeout: {}s)...", db_timeout);
            match tokio::time::timeout(Duration::from_secs(db_timeout), db.close()).await {
                Ok(Ok(())) => info!("Database connections closed"),
                Ok(Err(e)) => error!("Error closing database connections: {}", e),
                Err(_) => error!("Database close timed out after {}s", db_timeout),
            }
        }
    });

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

#[cfg(test)]
mod phase_tests {
    use super::*;
    use config::Config;

    const SOURCE: &str = include_str!("app_builder.rs");

    fn config_with_auth(secret: &str) -> Configuration {
        config_with_auth_settings(true, secret)
    }

    fn config_with_auth_settings(enabled: bool, secret: &str) -> Configuration {
        let config = Config::builder()
            .set_override("batata.core.auth.enabled", enabled)
            .unwrap()
            .set_override(
                "batata.core.auth.plugin.default.token.secret.key",
                secret.to_string(),
            )
            .unwrap()
            .build()
            .unwrap();
        let typed = config.get("batata").unwrap_or_default();
        Configuration { config, typed }
    }

    #[test]
    fn invalid_config_returns_error_instead_of_exiting_process() {
        let configuration = config_with_auth("");
        let error = validate_config(&configuration).expect_err("empty JWT secret must fail");
        assert!(
            error.to_string().contains("JWT secret key"),
            "error should stay actionable: {error}"
        );
    }

    #[test]
    fn configured_secret_passes_validation() {
        let configuration = config_with_auth("a-valid-test-secret");
        validate_config(&configuration).expect("configured secret must pass");
    }

    #[test]
    fn empty_jwt_secret_error_names_exact_configuration_key() {
        let error = validate_config(&config_with_auth(""))
            .expect_err("empty JWT secret must remain a startup error");
        let message = error.to_string();
        assert!(
            message.contains("authentication enabled"),
            "error must explain the failing condition: {message}"
        );
        assert!(
            message.contains("batata.core.auth.plugin.default.token.secret.key"),
            "error must name the exact setting operators need to change: {message}"
        );
    }

    #[test]
    fn auth_disabled_accepts_empty_jwt_secret() {
        let configuration = config_with_auth_settings(false, "");
        validate_config(&configuration).expect("auth-disabled startup must not require a secret");
    }

    #[test]
    fn known_default_jwt_secret_is_advisory() {
        let configuration =
            config_with_auth_settings(true, "NzViOWFlNjYtMWM3MC00ZDYwLTg4OWUtMjYxYTdhMzA1Y2Jm");
        validate_config(&configuration).expect("default secret must warn, not fail startup");
    }

    #[test]
    fn config_validation_returns_error_without_exiting_process() {
        let start = SOURCE.find("fn validate_config(").unwrap();
        let end = SOURCE.find("/// Initialize OAuth service").unwrap();
        let body = &SOURCE[start..end];

        assert!(
            body.contains("Result<(), Box<dyn std::error::Error>>"),
            "config validation must unwind errors through AppBuilder::run()"
        );
        assert!(
            !body.contains(concat!("std::process::", "exit(")),
            "library code must never terminate the process directly"
        );
    }

    #[test]
    fn console_remote_persistence_context_has_no_local_resources() {
        let ctx = crate::startup::persistence::PersistenceContext::empty();
        assert!(ctx.database_connection.is_none());
        assert!(ctx.server_member_manager.is_none());
        assert!(ctx.cluster_manager.is_none());
        assert!(ctx.persistence.is_none());
        assert!(ctx.rocks_db.is_none());
        assert!(ctx.raft_node.is_none());

        const PERSISTENCE_SOURCE: &str = include_str!("../startup/persistence.rs");
        let start = PERSISTENCE_SOURCE
            .find("pub fn empty() -> Self")
            .expect("empty persistence context must exist");
        let end = PERSISTENCE_SOURCE[start..]
            .find("\n    }")
            .expect("empty persistence context boundary must exist")
            + start;
        assert_eq!(
            PERSISTENCE_SOURCE[start..end].matches(": None,").count(),
            6,
            "console-remote mode must declare every local resource as absent"
        );
    }

    #[test]
    fn persistence_initialization_branches_on_console_remote_mode() {
        let runtime_source = &SOURCE[..SOURCE.find("mod phase_tests").unwrap()];
        let start = runtime_source
            .find("pub async fn with_persistence")
            .expect("production persistence phase must exist");
        let end = runtime_source[start..]
            .find("// Phase 4:")
            .expect("persistence phase boundary must exist")
            + start;
        let body = &runtime_source[start..end];

        assert!(body.contains("if self.is_console_remote"));
        assert!(body.contains("PersistenceContext::empty()"));
        assert!(body.contains("init_persistence("));
    }

    #[test]
    fn readiness_decomposition_is_testable_without_runtime_resources() {
        assert_eq!(
            readiness_decision(false, false, false, None, false),
            ReadinessDecision::Up,
            "data warmup disabled matches Nacos isReady() short-circuit"
        );
        assert_eq!(
            readiness_decision(true, true, true, None, true),
            ReadinessDecision::Up
        );
        assert_eq!(
            readiness_decision(
                true,
                false,
                false,
                Some("raft leader not elected".into()),
                false
            ),
            ReadinessDecision::Down(vec![
                "database not ready".into(),
                "raft leader not elected".into(),
                "distro initial data not loaded".into(),
            ])
        );
        assert_eq!(
            readiness_decision(true, true, false, None, true),
            ReadinessDecision::Down(vec!["raft not ready".into()])
        );
    }

    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    #[tokio::test]
    async fn background_task_tracker_aborts_and_joins_tasks() {
        let tasks = BackgroundTasks::default();
        let started = Arc::new(tokio::sync::Notify::new());
        let started_for_task = started.clone();
        let dropped = Arc::new(AtomicBool::new(false));
        let drop_flag = DropFlag(dropped.clone());

        tasks.spawn("loop", async move {
            let _drop_flag = drop_flag;
            started_for_task.notify_one();
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        started.notified().await;
        assert_eq!(tasks.len(), 1);
        tasks.shutdown(Duration::from_secs(1)).await;
        assert_eq!(tasks.len(), 0);
        assert!(
            dropped.load(Ordering::Acquire),
            "tracked task future must actually be dropped during shutdown"
        );
    }

    #[tokio::test]
    async fn last_owner_drop_aborts_tasks_left_by_failed_startup() {
        // `?` on any phase after background tasks start drops AppBuilder. The
        // final BackgroundTasks clone must abort tasks graceful shutdown never
        // reached (JoinHandle drop alone would only detach them).
        let started = Arc::new(tokio::sync::Notify::new());
        let started_for_task = started.clone();
        let dropped = Arc::new(AtomicBool::new(false));
        let drop_flag = DropFlag(dropped.clone());

        {
            let tasks = BackgroundTasks::default();
            tasks.spawn("orphaned-by-startup-failure", async move {
                let _drop_flag = drop_flag;
                started_for_task.notify_one();
                loop {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            });
            started.notified().await;
            assert_eq!(tasks.len(), 1);
        } // last (and only) owner drops here

        // Abort is scheduled synchronously; yield so the runtime processes it.
        for _ in 0..16 {
            if dropped.load(Ordering::Acquire) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(
            dropped.load(Ordering::Acquire),
            "dropping the last BackgroundTasks owner must abort leftover tasks"
        );
    }

    #[tokio::test]
    async fn dropping_a_non_final_clone_keeps_server_tasks_alive() {
        // with_plugin_init clones BackgroundTasks into a process-lifetime
        // global. A short-lived clone dropping must not abort tasks the server
        // still owns — hence abort lives in TaskRegistry::drop (last owner).
        let started = Arc::new(tokio::sync::Notify::new());
        let started_for_task = started.clone();
        let dropped = Arc::new(AtomicBool::new(false));
        let drop_flag = DropFlag(dropped.clone());

        let tasks = BackgroundTasks::default();
        tasks.spawn("server-owned", async move {
            let _drop_flag = drop_flag;
            started_for_task.notify_one();
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
        started.notified().await;

        let short_lived = tasks.clone();
        drop(short_lived);
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        assert!(
            !dropped.load(Ordering::Acquire),
            "dropping a non-final BackgroundTasks clone must not abort server tasks"
        );

        // Explicit shutdown still tears the task down.
        tasks.shutdown(Duration::from_secs(1)).await;
        assert_eq!(tasks.len(), 0);
        assert!(
            dropped.load(Ordering::Acquire),
            "explicit shutdown must abort the task"
        );
    }

    #[test]
    fn startup_failure_cleanup_is_last_owner_abort_in_task_registry() {
        // Source guard: abort belongs in TaskRegistry::drop, not in
        // BackgroundTasks (which is Clone), so a stray clone cannot kill tasks.
        assert!(
            SOURCE.contains("impl Drop for TaskRegistry"),
            "last-owner abort must live in TaskRegistry::drop"
        );
        assert!(
            SOURCE.contains("handle.abort()"),
            "cleanup must abort leftover task handles"
        );
        assert!(
            SOURCE.contains("tasks: Arc<TaskRegistry>"),
            "BackgroundTasks must share the registry via Arc for last-owner semantics"
        );
        let drop_start = SOURCE.find("impl Drop for TaskRegistry").unwrap();
        let clone_boundary = SOURCE.find("impl BackgroundTasks").unwrap();
        assert!(
            SOURCE[drop_start..clone_boundary].contains("handle.abort()"),
            "the abort must be inside TaskRegistry::drop"
        );
    }

    #[test]
    fn startup_builds_exactly_one_shared_plugin_manager() {
        // Split literals so the guard itself does not inflate the count.
        let constructor = concat!("PluginManager", "::new()");
        // Two feature-gated constructor arms exist in source, but exactly one
        // compiles. The important guard is that `with_services` never creates
        // a second manager.
        assert_eq!(SOURCE.matches(constructor).count(), 2);
        let services_start = SOURCE.find("pub async fn with_services").unwrap();
        let services_end = SOURCE.find("pub async fn with_shutdown_and_ai").unwrap();
        assert!(
            !SOURCE[services_start..services_end].contains(constructor),
            "protocol adapters and config-change plugins must share one manager"
        );
        assert!(
            !SOURCE[services_start..services_end].contains("server_status.set_up()"),
            "with_services must not publish UP before the final startup barrier"
        );
        assert!(SOURCE.contains(concat!("plugin_manager: Some(", "plugin_manager)")));
        assert!(SOURCE.contains(concat!("plugin_manager.start_protocol_adapter", "_tasks()")));
    }

    #[test]
    fn shutdown_uses_state_based_drain_instead_of_fixed_sleep() {
        // Split literals so the guards do not inflate their own source scan.
        assert!(!SOURCE.contains(concat!("drain", "_sleep")));
        assert!(SOURCE.contains(concat!("grpc_servers.shutdown(", "drain_timeout).await")));
        assert!(SOURCE.contains(concat!("server.handle.stop(", "true).await")));
        assert!(SOURCE.contains(concat!("shutdown_", "protocol_adapters()")));
        assert!(SOURCE.contains(concat!(
            "background_tasks.shutdown(",
            "drain_timeout).await"
        )));

        let barrier = concat!(
            "complete_startup(app_state, server_member_manager, ",
            "startup_complete, timeline).await;"
        );
        assert_eq!(
            SOURCE.matches(barrier).count(),
            1,
            "all HTTP deployment modes must pass the common final barrier"
        );
    }

    #[test]
    fn plugin_context_has_one_producer_and_typed_validation_gate() {
        let constructor = concat!("batata_plugin::PluginContext::", "new()");
        assert_eq!(
            SOURCE.matches(constructor).count(),
            1,
            "only AppBuilder may produce the protocol-adapter PluginContext"
        );
        assert!(
            SOURCE.contains(concat!("Self::validate_plugin_context(&", "plugin_ctx)?;")),
            "the producer must validate typed dependencies before adapter init"
        );
        let runtime_source = &SOURCE[..SOURCE.find("mod phase_tests").unwrap()];
        assert_eq!(
            runtime_source
                .matches("context_keys::CLUSTER_MANAGER_TYPED")
                .count(),
            2,
            "the typed cluster manager key must be both validated and inserted"
        );
        for raw_key in [
            concat!("\"is_", "cluster\""),
            concat!("\"storage_", "mode\""),
            concat!("\"raft_", "node\""),
            concat!("\"cluster_", "manager\""),
            concat!("\"distro_", "protocol\""),
        ] {
            assert!(
                !SOURCE.contains(raw_key),
                "known context keys must come from context_keys, not string literals: {raw_key}"
            );
        }
    }

    #[test]
    fn legacy_parallel_framework_is_test_only() {
        const LIB_SOURCE: &str = include_str!("../lib.rs");
        const BUILDER_SOURCE: &str = include_str!("mod.rs");
        const CONTEXT_SOURCE: &str = include_str!("../context/mod.rs");

        for module in ["initializer", "lifecycle"] {
            assert!(
                !LIB_SOURCE.contains(&format!("pub mod {module};")),
                "legacy startup module must stay deleted: {module}"
            );
            let path =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("src/{module}"));
            assert!(
                !path.exists(),
                "legacy startup source must stay deleted: {module}"
            );
        }

        for module in [
            "config_builder",
            "persistence_builder",
            "server_builder",
            "service_builder",
        ] {
            assert!(
                !BUILDER_SOURCE.contains(&format!("pub mod {module};")),
                "legacy builder must not be declared: {module}"
            );
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("src/builder/{module}.rs"));
            assert!(
                !path.exists(),
                "legacy builder source must stay deleted: {module}"
            );
        }

        assert!(
            !CONTEXT_SOURCE.contains("pub mod app_context;"),
            "legacy AppContext must stay deleted"
        );
        assert!(
            !std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/context/app_context.rs")
                .exists(),
            "legacy AppContext source must stay deleted"
        );

        for module in ["server_registry", "service_registry"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("src/registry/{module}.rs"));
            assert!(
                !path.exists(),
                "legacy registry source must stay deleted: {module}"
            );
        }
        assert!(
            !std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/registry")
                .exists(),
            "legacy registry module placeholder must stay deleted"
        );

        let orphan =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lifecycle/server_trait.rs");
        assert!(
            !orphan.exists(),
            "the undeclared lifecycle/server_trait.rs orphan must stay deleted"
        );
    }

    #[test]
    fn grpc_shutdown_publishes_draining_before_stopped() {
        const SOURCE: &str = include_str!("../startup/grpc.rs");
        let shutdown = SOURCE
            .split("pub async fn shutdown(&mut self, timeout: Duration)")
            .nth(1)
            .expect("GrpcServers::shutdown must exist")
            .split("/// Get naming service")
            .next()
            .expect("shutdown method boundary must exist");

        let first_draining = shutdown
            .find("set_draining()")
            .expect("shutdown must publish draining state");
        let first_stopped = shutdown
            .find("set_stopped()")
            .expect("shutdown must publish stopped state");
        assert!(
            first_draining < first_stopped,
            "gRPC state must be DRAINING before STOPPED so health checks drain first"
        );
    }

    #[test]
    fn grpc_shutdown_joins_each_listener_before_marking_stopped() {
        const SOURCE: &str = include_str!("../startup/grpc.rs");
        for listener in ["sdk_server", "cluster_server", "raft_server"] {
            let handle = format!("self.{listener}.take()");
            let stopped = match listener {
                "sdk_server" => "self.sdk_state.set_stopped()",
                "cluster_server" => "self.cluster_state.set_stopped()",
                _ => "self.raft_state.set_stopped()",
            };
            let take = SOURCE
                .find(&handle)
                .unwrap_or_else(|| panic!("{listener} handle must be taken on shutdown"));
            let state = SOURCE
                .find(stopped)
                .unwrap_or_else(|| panic!("{listener} state must be published stopped"));
            assert!(
                take < state,
                "{listener} must be joined before its state is STOPPED"
            );
        }
    }

    #[test]
    fn startup_phases_are_timed_and_total_is_reported_with_mode_and_storage() {
        let run_start = SOURCE.find("pub async fn run()").unwrap();
        let run_end = SOURCE.find("impl Default for AppBuilder").unwrap();
        let run = &SOURCE[run_start..run_end];

        assert_eq!(
            run.matches("startup_phase!(").count(),
            11,
            "every fallible startup phase must go through the timed wrapper"
        );
        assert!(
            SOURCE.contains("finish_startup_phase(&mut timer, $future.await)?"),
            "startup wrapper must record the real Ok/Err outcome"
        );
        assert!(
            SOURCE.contains("startup_summary(mode, storage, total_ms)"),
            "total startup log must mirror Nacos started-successfully shape"
        );
        assert!(
            SOURCE.contains("mode_label(app_state.configuration.is_standalone())"),
            "startup summary must carry the deployment mode label"
        );
        assert!(
            SOURCE.contains(concat!(
                "storage_label(app_state.configuration.",
                "persistence_mode())"
            )),
            "startup summary must carry the embedded/external storage label"
        );
        assert!(
            SOURCE.contains("let mut barrier = timeline.begin(phases::STARTUP_BARRIER"),
            "the readiness barrier must be a measured startup phase"
        );
    }

    #[test]
    fn shutdown_stages_have_individual_timers_and_one_total() {
        let seq_start = SOURCE
            .find("async fn graceful_shutdown_sequence(")
            .expect("shutdown sequence must exist");
        let seq_end = SOURCE.find("#[cfg(test)]").unwrap();
        let sequence = &SOURCE[seq_start..seq_end];

        for phase in [
            "phases::GRPC",
            "phases::XDS",
            "phases::ISTIO_MCP",
            "phases::HEALTH_CHECK",
            "phases::PLUGINS_SHUTDOWN",
            "phases::BACKGROUND_TASKS",
            "phases::CLUSTER_MANAGER",
            "phases::RAFT",
            "phases::DATABASE",
        ] {
            assert!(
                sequence.contains(&format!("shutdown_phase!({phase},")),
                "shutdown stage must be timed: {phase}"
            );
        }
        assert_eq!(
            sequence.matches("shutdown_phase!(").count(),
            9,
            "each shutdown stage must be timed exactly once"
        );
        let mark = concat!("timeline.mark_", "shutdown()");
        assert_eq!(
            SOURCE.matches(mark).count(),
            4,
            "console/server signal and server-exit triggers must start the shutdown clock once"
        );
        assert!(
            SOURCE.contains("shutdown_summary(total_ms)"),
            "shutdown total must be logged with elapsed milliseconds"
        );
    }

    #[test]
    fn lifecycle_metrics_are_described_and_emitted() {
        const METRICS: &str = include_str!("../metrics.rs");
        const TIMING: &str = include_str!("lifecycle_timing.rs");

        for name in [
            "batata_server_lifecycle_phase_duration_seconds",
            "batata_server_lifecycle_total_duration_seconds",
            "batata_server_lifecycle_phase_failures_total",
        ] {
            assert!(TIMING.contains(name), "metric must exist: {name}");
        }
        assert!(
            METRICS.contains("LifecycleTimeline::describe_metrics()"),
            "metric descriptions must be registered by init_metrics()"
        );
        assert!(
            TIMING.contains(".record(ms as f64 / 1000.0)"),
            "phase histogram must receive the measured duration"
        );
        assert!(
            TIMING.contains(".increment(1)"),
            "failed phases must increment the failure counter"
        );
    }
}
