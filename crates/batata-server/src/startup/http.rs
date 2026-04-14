//! HTTP server setup module for main and console servers.

use std::sync::Arc;

use actix_web::{
    App, HttpResponse, HttpServer,
    dev::Server,
    middleware::{Compress, Condition, DefaultHeaders, Logger},
    web,
};
#[cfg(feature = "consul")]
use batata_plugin::ProtocolAdapterPlugin;

use batata_core::service::cluster_client::ClusterClientManager;
use batata_core::service::distro::DistroProtocol;
use batata_core::service::remote::ConnectionManager;

use crate::{
    api::ai::{AgentRegistry, McpServerRegistry, configure_mcp_registry},
    api::cloud::{
        K8sServiceSync, PrometheusServiceDiscovery, configure_kubernetes, configure_prometheus,
    },
    api::metrics::{PrometheusMetricsState, prometheus_metrics},
    api::v2::route::{cluster_routes, config_routes, naming_routes},
    api::v3::admin::route::admin_routes as v3_admin_routes,
    api::v3::client::route::client_routes as v3_client_routes,
    auth,
    console::v3::{
        ai_a2a as console_a2a, ai_agentspec as console_agentspec, ai_mcp as console_mcp,
        ai_pipeline as console_pipeline, ai_plugin as console_plugin, ai_skill as console_skill,
    },
    middleware::{
        auth::Authentication, distro_filter::DistroFilter, rate_limit::RateLimiter,
        tps_control::TpsControlMiddleware, traffic_revise::TrafficReviseFilter,
    },
    model::common::AppState,
};

use batata_api::naming::NamingServiceProvider;

/// Creates and binds the console HTTP server.
///
/// The console server provides administrative endpoints for managing
/// the Batata cluster, including authentication and namespace management.
pub fn console_server(
    app_state: Arc<AppState>,
    naming_service: Option<Arc<dyn NamingServiceProvider>>,
    ai_services: AIServices,
    context_path: String,
    address: String,
    port: u16,
) -> Result<Server, std::io::Error> {
    let rate_limit_config = app_state.configuration.rate_limit_config();
    let workers = app_state.configuration.http_workers();
    let keep_alive_secs = app_state.configuration.console_keep_alive_secs();
    let max_payload_size = app_state.configuration.max_payload_size();
    let max_json_size = app_state.configuration.max_json_size();
    let compression_enabled = app_state.configuration.http_compression_enabled();
    let access_log_enabled = app_state.configuration.http_access_log_enabled();

    Ok(HttpServer::new(move || {
        let mut app = App::new()
            // HTTP metrics (outermost: captures total request time including all middleware)
            .wrap(crate::middleware::http_metrics::HttpMetrics)
            .wrap(Condition::new(access_log_enabled, Logger::default()))
            .wrap(
                DefaultHeaders::new()
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("X-XSS-Protection", "1; mode=block"))
                    .add(("Cache-Control", "no-store")),
            )
            // HTTP response compression (gzip, brotli, zstd)
            .wrap(Condition::new(compression_enabled, Compress::default()))
            .wrap(RateLimiter::new(rate_limit_config.clone()))
            .wrap(Authentication)
            .app_data(web::PayloadConfig::new(max_payload_size))
            .app_data(web::JsonConfig::default().limit(max_json_size))
            .app_data(web::Data::from(app_state.clone()));

        // AI services: always provide trait objects (config-backed or registry fallback)
        {
            let mcp_svc: Arc<dyn batata_common::McpServerService> = ai_services
                .mcp_service
                .clone()
                .unwrap_or_else(|| ai_services.mcp_registry.clone());
            app = app.app_data(web::Data::new(mcp_svc));
            let a2a_svc: Arc<dyn batata_common::A2aAgentService> = ai_services
                .a2a_service
                .clone()
                .unwrap_or_else(|| ai_services.agent_registry.clone());
            app = app.app_data(web::Data::new(a2a_svc));
        }
        // Also provide concrete registries for direct API endpoints
        app = app
            .app_data(web::Data::new(ai_services.mcp_registry.clone()))
            .app_data(web::Data::new(ai_services.agent_registry.clone()));
        if let Some(ref svc) = ai_services.endpoint_service {
            app = app.app_data(web::Data::new(svc.clone()));
        }
        if let Some(ref svc) = ai_services.prompt_service {
            app = app.app_data(web::Data::new(svc.clone()));
        }
        if let Some(ref svc) = ai_services.skill_service {
            app = app.app_data(web::Data::new(svc.clone()));
        }
        if let Some(ref svc) = ai_services.agentspec_service {
            app = app.app_data(web::Data::new(svc.clone()));
        }
        if let Some(ref svc) = ai_services.pipeline_service {
            app = app.app_data(web::Data::new(svc.clone()));
        }
        if let Some(ref mgr) = ai_services.copilot_agent_manager {
            app = app.app_data(web::Data::new(mgr.clone()));
        }
        if let Some(ref svcs) = ai_services.copilot_services {
            app = app
                .app_data(web::Data::new(svcs.skill_generation.clone()))
                .app_data(web::Data::new(svcs.skill_optimization.clone()))
                .app_data(web::Data::new(svcs.prompt_optimization.clone()))
                .app_data(web::Data::new(svcs.prompt_debug.clone()));
        }

        // Inject NamingService if available (not available in console-remote mode)
        if let Some(ref ns) = naming_service {
            app = app.app_data(web::Data::new(ns.clone()));
        }
        app.service(
            web::scope(&context_path)
                .service(auth::v3::route::routes())
                .service(
                    web::scope("/v3/console")
                        .configure(batata_console::configure_v3_console_routes)
                        .service(console_mcp::routes())
                        .service(console_a2a::routes())
                        .service(console_plugin::routes())
                        .service(console_skill::routes())
                        .service(console_agentspec::routes())
                        .service(console_pipeline::routes())
                        .service(batata_copilot::copilot_console_routes())
                        .service(web::scope("/ai").service(batata_ai::prompt_admin_routes())),
                )
                .configure(batata_console::configure_v2_console_routes),
        )
    })
    .workers(workers)
    .keep_alive(std::time::Duration::from_secs(keep_alive_secs))
    .bind((address, port))?
    .run())
}

/// Creates and binds a protocol adapter HTTP server.
///
/// Generic server builder for protocol adapter plugins (e.g., Consul, Eureka).
/// Each plugin provides its own routes and app_data via `ProtocolAdapterPlugin::configure()`.
/// The server runs on a dedicated port without Batata authentication middleware,
/// since protocol adapters use their own auth systems (e.g., Consul ACL tokens).
#[cfg(feature = "consul")]
pub fn plugin_http_server(
    app_state: Arc<AppState>,
    naming_service: Arc<dyn NamingServiceProvider>,
    plugin: Arc<dyn ProtocolAdapterPlugin>,
    address: String,
    port: u16,
) -> Result<Server, std::io::Error> {
    let rate_limit_config = app_state.configuration.rate_limit_config();
    let plugin_workers = plugin.http_workers();
    let keep_alive_secs = app_state.configuration.http_keep_alive_secs();
    let max_payload_size = app_state.configuration.max_payload_size();
    let max_json_size = app_state.configuration.max_json_size();
    let compression_enabled = app_state.configuration.http_compression_enabled();
    let client_request_timeout_secs = app_state.configuration.http_client_request_timeout_secs();
    let access_log_enabled = app_state.configuration.http_access_log_enabled();

    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Condition::new(access_log_enabled, Logger::default()))
            .wrap(
                DefaultHeaders::new()
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("X-XSS-Protection", "1; mode=block"))
                    .add(("Cache-Control", "no-store")),
            )
            // HTTP response compression (gzip, brotli, zstd)
            .wrap(Condition::new(compression_enabled, Compress::default()))
            .wrap(RateLimiter::new(rate_limit_config.clone()))
            .app_data(web::PayloadConfig::new(max_payload_size))
            .app_data(web::JsonConfig::default().limit(max_json_size))
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(naming_service.clone()))
            .app_data(web::QueryConfig::default().error_handler(|err, _req| {
                let err_str = err.to_string();
                // For duplicate field errors (e.g., ?tag=a&tag=b), return empty result
                // Some protocols support multiple query params for AND-filtering
                if err_str.contains("duplicate field") {
                    actix_web::error::InternalError::from_response(
                        err,
                        actix_web::HttpResponse::Ok().json(Vec::<()>::new()),
                    )
                    .into()
                } else {
                    actix_web::error::InternalError::from_response(
                        err,
                        actix_web::HttpResponse::BadRequest()
                            .json(serde_json::json!({"error": err_str})),
                    )
                    .into()
                }
            }))
            // Delegate all app_data and routes to the protocol adapter plugin
            .configure(|cfg| plugin.configure(cfg))
    })
    .workers(plugin_workers)
    .keep_alive(std::time::Duration::from_secs(keep_alive_secs))
    .client_request_timeout(std::time::Duration::from_secs(client_request_timeout_secs))
    .bind((address, port))?
    .run())
}

/// Creates and binds the MCP Registry HTTP server.
///
/// The MCP Registry server implements the official MCP Registry OpenAPI spec
/// (`GET /v0/servers`, `GET /v0/servers/{id}`), exposing registered MCP servers
/// in the standardized format. It runs on a separate port (default 9080).
pub fn mcp_registry_server(
    mcp_registry: Arc<McpServerRegistry>,
    address: String,
    port: u16,
    access_log_enabled: bool,
) -> Result<Server, std::io::Error> {
    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Condition::new(access_log_enabled, Logger::default()))
            .app_data(web::Data::from(mcp_registry.clone()))
            .configure(configure_mcp_registry)
    })
    .bind((address, port))?
    .run())
}

/// AI services for MCP and A2A APIs.
#[derive(Clone)]
pub struct AIServices {
    pub mcp_registry: Arc<McpServerRegistry>,
    pub agent_registry: Arc<AgentRegistry>,
    pub mcp_service: Option<Arc<dyn batata_ai::McpServerService>>,
    pub a2a_service: Option<Arc<dyn batata_ai::A2aAgentService>>,
    pub endpoint_service: Option<Arc<crate::service::ai::AiEndpointService>>,
    pub mcp_index: Option<Arc<crate::service::ai::McpServerIndex>>,
    pub prompt_service: Option<Arc<batata_ai::PromptOperationService>>,
    pub skill_service: Option<Arc<dyn batata_ai::SkillService>>,
    pub agentspec_service: Option<Arc<dyn batata_ai::AgentSpecService>>,
    pub pipeline_service: Option<Arc<dyn batata_ai::PipelineService>>,
    pub copilot_agent_manager: Option<Arc<batata_copilot::CopilotAgentManager>>,
    pub copilot_services: Option<CopilotServices>,
}

/// Copilot service bundle (4 services)
#[derive(Clone)]
pub struct CopilotServices {
    pub skill_generation: Arc<batata_copilot::service::SkillGenerationService>,
    pub skill_optimization: Arc<batata_copilot::service::SkillOptimizationService>,
    pub prompt_optimization: Arc<batata_copilot::service::PromptOptimizationService>,
    pub prompt_debug: Arc<batata_copilot::service::PromptDebugService>,
}

impl AIServices {
    /// Creates AI service adapters with default in-memory registries (no persistence).
    pub fn new() -> Self {
        Self {
            mcp_registry: Arc::new(McpServerRegistry::new()),
            agent_registry: Arc::new(AgentRegistry::new()),
            mcp_service: None,
            a2a_service: None,
            endpoint_service: None,
            mcp_index: None,
            prompt_service: None,
            skill_service: None,
            agentspec_service: None,
            pipeline_service: None,
            copilot_agent_manager: None,
            copilot_services: None,
        }
    }

    /// Creates AI service adapters with config-backed persistence.
    pub fn with_persistence(
        persistence: Arc<dyn batata_persistence::PersistenceService>,
        naming_service: Arc<dyn NamingServiceProvider>,
    ) -> Self {
        let mcp_index = Arc::new(crate::service::ai::McpServerIndex::new());
        let mcp_service = Arc::new(crate::service::ai::McpServerOperationService::new(
            persistence.clone(),
            mcp_index.clone(),
        ));
        let prompt_service = Arc::new(batata_ai::PromptOperationService::new(persistence.clone()));
        let a2a_service = Arc::new(crate::service::ai::A2aServerOperationService::new(
            persistence,
        ));
        let endpoint_service = Arc::new(crate::service::ai::AiEndpointService::new(naming_service));

        Self {
            mcp_registry: Arc::new(McpServerRegistry::new()),
            agent_registry: Arc::new(AgentRegistry::new()),
            mcp_service: Some(mcp_service),
            a2a_service: Some(a2a_service),
            endpoint_service: Some(endpoint_service),
            mcp_index: Some(mcp_index),
            prompt_service: Some(prompt_service),
            skill_service: None,
            agentspec_service: None,
            pipeline_service: None,
            copilot_agent_manager: None,
            copilot_services: None,
        }
    }

    /// Set the skill, agentspec, and pipeline services (uses PersistenceService trait)
    pub fn with_ai_resource_services(
        mut self,
        persistence: Arc<dyn batata_persistence::PersistenceService>,
    ) -> Self {
        self.skill_service = Some(Arc::new(batata_ai::SkillOperationService::new(
            persistence.clone(),
        )));
        self.agentspec_service = Some(Arc::new(batata_ai::AgentSpecOperationService::new(
            persistence.clone(),
        )));
        self.pipeline_service = Some(Arc::new(batata_ai::PipelineQueryService::new(persistence)));
        self
    }

    /// Set copilot services (uses PersistenceService for config storage)
    pub fn with_copilot(
        mut self,
        persistence: Arc<dyn batata_persistence::PersistenceService>,
    ) -> Self {
        let config_storage = batata_copilot::CopilotConfigStorage::new(persistence);
        let agent_manager = Arc::new(batata_copilot::CopilotAgentManager::new(config_storage));
        let copilot_services = CopilotServices {
            skill_generation: Arc::new(batata_copilot::service::SkillGenerationService::new(
                agent_manager.clone(),
            )),
            skill_optimization: Arc::new(batata_copilot::service::SkillOptimizationService::new(
                agent_manager.clone(),
            )),
            prompt_optimization: Arc::new(batata_copilot::service::PromptOptimizationService::new(
                agent_manager.clone(),
            )),
            prompt_debug: Arc::new(batata_copilot::service::PromptDebugService::new(
                agent_manager.clone(),
            )),
        };
        self.copilot_agent_manager = Some(agent_manager);
        self.copilot_services = Some(copilot_services);
        self
    }
}

impl Default for AIServices {
    fn default() -> Self {
        Self::new()
    }
}

/// Cloud services for Prometheus SD and K8s integration.
#[derive(Clone)]
pub struct CloudServices {
    pub prometheus_sd: Arc<PrometheusServiceDiscovery>,
    pub k8s_sync: Arc<K8sServiceSync>,
}

impl CloudServices {
    /// Creates Cloud service adapters with default configuration.
    pub fn new() -> Self {
        Self {
            prometheus_sd: Arc::new(PrometheusServiceDiscovery::default()),
            k8s_sync: Arc::new(K8sServiceSync::default()),
        }
    }
}

impl Default for CloudServices {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates and binds the main HTTP server.
///
/// The main server provides core Batata API endpoints (Nacos-compatible)
/// including config management, service discovery, AI capabilities,
/// cloud native integrations, and metrics.
/// Consul compatibility is served on a dedicated port.
#[allow(clippy::too_many_arguments)]
pub fn main_server(
    app_state: Arc<AppState>,
    naming_service: Arc<dyn NamingServiceProvider>,
    connection_manager: Arc<ConnectionManager>,
    config_change_notifier: Arc<batata_config::ConfigChangeNotifier>,
    ai_services: AIServices,
    distro_protocol: Option<Arc<DistroProtocol>>,
    encryption_service: Arc<batata_config::service::encryption::ConfigEncryptionService>,
    context_path: String,
    address: String,
    port: u16,
    server_registry: Option<Arc<batata_core::ServerRegistry>>,
    cluster_client_manager: Option<Arc<ClusterClientManager>>,
) -> Result<Server, std::io::Error> {
    // Create Cloud services
    let cloud_services = CloudServices::new();
    let rate_limit_config = app_state.configuration.rate_limit_config();
    let workers = app_state.configuration.http_workers();
    let keep_alive_secs = app_state.configuration.http_keep_alive_secs();
    let max_payload_size = app_state.configuration.max_payload_size();
    let max_json_size = app_state.configuration.max_json_size();
    let compression_enabled = app_state.configuration.http_compression_enabled();
    let access_log_enabled = app_state.configuration.http_access_log_enabled();
    let control_plugin = app_state.control_plugin.clone();

    // Initialize Prometheus metrics recorder (metrics crate integration)
    let prometheus_state = web::Data::new(PrometheusMetricsState::new());

    let server_status = app_state.server_status.clone();

    Ok(HttpServer::new(move || {
        let mut app = App::new()
            // HTTP metrics (outermost: captures total request time including all middleware)
            .wrap(crate::middleware::http_metrics::HttpMetrics)
            .wrap(Condition::new(access_log_enabled, Logger::default()))
            .wrap(
                DefaultHeaders::new()
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("X-XSS-Protection", "1; mode=block"))
                    .add(("Cache-Control", "no-store")),
            )
            // HTTP response compression (gzip, brotli, zstd)
            .wrap(Condition::new(compression_enabled, Compress::default()))
            .wrap(TrafficReviseFilter::new(server_status.clone()))
            .wrap(DistroFilter::new(distro_protocol.clone()))
            .wrap(TpsControlMiddleware::new(control_plugin.clone()))
            .wrap(RateLimiter::new(rate_limit_config.clone()))
            .wrap(Authentication)
            .app_data(web::PayloadConfig::new(max_payload_size))
            .app_data(web::JsonConfig::default().limit(max_json_size))
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(naming_service.clone()));

        // Register both concrete ConnectionManager (for v2/v3 client handlers that need get_client)
        // and trait object (for handlers that only need trait methods like loader)
        let connection_manager_trait: Arc<dyn batata_core::ClientConnectionManager> =
            connection_manager.clone();
        app = app
            .app_data(web::Data::new(connection_manager.clone()))
            .app_data(web::Data::new(connection_manager_trait))
            // Config change notifier for long-polling listeners
            .app_data(web::Data::new(config_change_notifier.clone()))
            // AI services: concrete registries for direct API endpoints
            .app_data(web::Data::new(ai_services.mcp_registry.clone()))
            .app_data(web::Data::new(ai_services.agent_registry.clone()))
            // Cloud services (Prometheus SD, Kubernetes Sync)
            .app_data(web::Data::new(cloud_services.prometheus_sd.clone()))
            .app_data(web::Data::new(cloud_services.k8s_sync.clone()))
            // Prometheus metrics state (metrics crate integration)
            .app_data(prometheus_state.clone())
            // Config encryption service
            .app_data(web::Data::new(encryption_service.clone()));

        // Server registry for per-server health aggregation (optional)
        if let Some(ref reg) = server_registry {
            app = app.app_data(web::Data::new(reg.clone()));
        }

        // Cluster client manager for inter-node gRPC requests (cluster mode only).
        // Consumed by admin endpoints that need peer metrics, e.g. loader.rs
        // fetching `ServerLoaderInfoRequest` from every cluster member.
        if let Some(ref ccm) = cluster_client_manager {
            app = app.app_data(web::Data::new(ccm.clone()));
        }

        // AI services: always provide trait objects (config-backed or registry fallback)
        {
            let mcp_svc: Arc<dyn batata_common::McpServerService> = ai_services
                .mcp_service
                .clone()
                .unwrap_or_else(|| ai_services.mcp_registry.clone());
            app = app.app_data(web::Data::new(mcp_svc));
            let a2a_svc: Arc<dyn batata_common::A2aAgentService> = ai_services
                .a2a_service
                .clone()
                .unwrap_or_else(|| ai_services.agent_registry.clone());
            app = app.app_data(web::Data::new(a2a_svc));
        }

        // Inject distro protocol for cluster data synchronization (cluster mode only)
        if let Some(ref distro) = distro_protocol {
            app = app.app_data(web::Data::new(distro.clone()));
        }
        if let Some(ref svc) = ai_services.endpoint_service {
            app = app.app_data(web::Data::new(svc.clone()));
        }
        if let Some(ref svc) = ai_services.prompt_service {
            app = app.app_data(web::Data::new(svc.clone()));
        }
        if let Some(ref svc) = ai_services.skill_service {
            app = app.app_data(web::Data::new(svc.clone()));
        }
        if let Some(ref svc) = ai_services.agentspec_service {
            app = app.app_data(web::Data::new(svc.clone()));
        }
        if let Some(ref svc) = ai_services.pipeline_service {
            app = app.app_data(web::Data::new(svc.clone()));
        }

        app.service(
            web::scope(&context_path)
                // Auth routes (needed for SDK authentication)
                // Include both V1 and V3 auth routes for backward compatibility
                .service(auth::v3::route::routes())
                .service(auth::v3::route::v1_routes())
                // V2 Open API routes (config, naming, cluster only)
                .service(config_routes())
                .service(naming_routes())
                .service(cluster_routes())
                // V3 Console API routes (non-AI from batata-console + AI from server)
                .service(
                    web::scope("/v3/console")
                        .configure(batata_console::configure_v3_console_routes)
                        .service(console_mcp::routes())
                        .service(console_a2a::routes())
                        .service(console_plugin::routes())
                        .service(console_skill::routes())
                        .service(console_agentspec::routes())
                        .service(console_pipeline::routes())
                        .service(batata_copilot::copilot_console_routes()),
                )
                // V3 Admin API routes
                .service(v3_admin_routes())
                // V3 Client API routes
                .service(v3_client_routes()),
        )
        // Skills Registry (.well-known discovery protocol)
        .service(batata_ai::skills_registry_routes())
        // Cloud Native Integration API routes (Prometheus SD, Kubernetes Sync)
        .configure(configure_prometheus)
        .configure(configure_kubernetes)
        // Prometheus metrics endpoint at /metrics (standard path, hand-rolled)
        .service(batata_console::v3::metrics::routes())
        // Prometheus metrics endpoint at /prometheus (metrics crate integration)
        .route("/prometheus", web::get().to(prometheus_metrics))
        // Kubernetes-compatible health endpoints
        .route("/health/liveness", web::get().to(health_liveness))
        .route("/health/readiness", web::get().to(health_readiness))
    })
    .workers(workers)
    .keep_alive(std::time::Duration::from_secs(keep_alive_secs))
    .bind((address, port))?
    .run())
}

/// Kubernetes liveness probe endpoint.
/// Returns 200 if the process is alive and the HTTP server is serving.
async fn health_liveness() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status": "UP"}))
}

/// Kubernetes readiness probe endpoint.
/// Returns 200 if the server is ready to accept traffic (DB connected, services initialized).
async fn health_readiness(app_state: web::Data<Arc<AppState>>) -> HttpResponse {
    // Check if persistence service is available and healthy
    if let Some(ref persistence) = app_state.persistence {
        match persistence.health_check().await {
            Ok(_) => {}
            Err(e) => {
                return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                    "status": "DOWN",
                    "reason": format!("Database health check failed: {}", e),
                }));
            }
        }
    }

    HttpResponse::Ok().json(serde_json::json!({"status": "UP"}))
}
