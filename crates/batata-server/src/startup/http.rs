//! HTTP server setup module for main and console servers.

use std::sync::Arc;

use actix_web::{App, HttpServer, dev::Server, middleware::Logger, web};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use batata_core::service::remote::ConnectionManager;

use crate::{
    api::ai::{AgentRegistry, McpServerRegistry, configure_a2a, configure_mcp},
    api::apollo::{
        ApolloAdvancedService, ApolloNotificationService, ApolloOpenApiService,
        apollo_advanced_routes, apollo_config_routes, apollo_openapi_routes,
    },
    api::cloud::{
        K8sServiceSync, PrometheusServiceDiscovery, configure_kubernetes, configure_prometheus,
    },
    api::consul::{
        AclService, ConsulEventService, ConsulLockService, ConsulQueryService,
        ConsulSemaphoreService, ConsulSessionService, agent::ConsulAgentService,
        catalog::ConsulCatalogService, health::ConsulHealthService, kv::ConsulKVService,
        route::consul_routes,
    },
    api::openapi::ApiDoc,
    api::v2::route::{
        cluster_routes, config_routes, console_routes as v2_console_routes, naming_routes,
    },
    api::v3::admin::route::admin_routes as v3_admin_routes,
    api::v3::client::route::client_routes as v3_client_routes,
    auth, console,
    console::v3::metrics::routes as metrics_routes,
    middleware::{auth::Authentication, rate_limit::RateLimiter},
    model::common::AppState,
    service::naming::NamingService,
};

/// Consul service adapters for HTTP endpoints.
#[derive(Clone)]
pub struct ConsulServices {
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
}

impl ConsulServices {
    /// Creates Consul service adapters from a naming service.
    pub fn new(naming_service: Arc<NamingService>, acl_enabled: bool) -> Self {
        let session = ConsulSessionService::new();
        let kv = ConsulKVService::new();
        let kv_arc = Arc::new(kv.clone());
        let session_arc = Arc::new(session.clone());
        let lock = ConsulLockService::new(kv_arc.clone(), session_arc.clone());
        let semaphore = ConsulSemaphoreService::new(kv_arc, session_arc);
        Self {
            agent: ConsulAgentService::new(naming_service.clone()),
            health: ConsulHealthService::new(naming_service.clone()),
            kv,
            catalog: ConsulCatalogService::new(naming_service),
            acl: if acl_enabled {
                AclService::new()
            } else {
                AclService::disabled()
            },
            session,
            event: ConsulEventService::new(),
            query: ConsulQueryService::new(),
            lock,
            semaphore,
        }
    }
}

/// Creates and binds the console HTTP server.
///
/// The console server provides administrative endpoints for managing
/// the Batata cluster, including authentication and namespace management.
pub fn console_server(
    app_state: Arc<AppState>,
    naming_service: Option<Arc<NamingService>>,
    ai_services: AIServices,
    context_path: String,
    address: String,
    port: u16,
) -> Result<Server, std::io::Error> {
    let rate_limit_config = app_state.configuration.rate_limit_config();

    Ok(HttpServer::new(move || {
        let mut app = App::new()
            .wrap(Logger::default())
            .wrap(RateLimiter::new(rate_limit_config.clone()))
            .wrap(Authentication)
            .app_data(web::Data::from(app_state.clone()))
            // AI services (MCP Server Registry, A2A Agent Registry)
            .app_data(web::Data::new(ai_services.mcp_registry.clone()))
            .app_data(web::Data::new(ai_services.agent_registry.clone()));

        // Inject NamingService if available (not available in console-remote mode)
        if let Some(ref ns) = naming_service {
            app = app.app_data(web::Data::new(ns.clone()));
        }

        app.service(
            web::scope(&context_path)
                .service(auth::v3::route::routes())
                .service(console::v3::route::routes())
                .service(v2_console_routes()),
        )
    })
    .bind((address, port))?
    .run())
}

/// Creates and binds the Consul compatibility HTTP server.
///
/// The Consul server provides Consul-compatible API endpoints for service
/// discovery clients that speak the Consul protocol. It runs on a dedicated
/// port (default 8500) without Nacos authentication middleware, since Consul
/// uses its own ACL token system.
pub fn consul_server(
    app_state: Arc<AppState>,
    naming_service: Arc<NamingService>,
    consul_services: ConsulServices,
    address: String,
    port: u16,
) -> Result<Server, std::io::Error> {
    let rate_limit_config = app_state.configuration.rate_limit_config();

    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(RateLimiter::new(rate_limit_config.clone()))
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(naming_service.clone()))
            .app_data(web::Data::new(consul_services.agent.clone()))
            .app_data(web::Data::new(consul_services.health.clone()))
            .app_data(web::Data::new(consul_services.kv.clone()))
            .app_data(web::Data::new(consul_services.catalog.clone()))
            .app_data(web::Data::new(consul_services.acl.clone()))
            .app_data(web::Data::new(consul_services.session.clone()))
            .app_data(web::Data::new(consul_services.event.clone()))
            .app_data(web::Data::new(consul_services.query.clone()))
            .app_data(web::Data::new(consul_services.lock.clone()))
            .app_data(web::Data::new(consul_services.semaphore.clone()))
            .app_data(web::QueryConfig::default().error_handler(|err, _req| {
                let err_str = err.to_string();
                // For duplicate field errors (e.g., ?tag=a&tag=b), return empty result
                // Consul supports multiple tag params for AND-filtering
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
            .service(consul_routes())
    })
    .bind((address, port))?
    .run())
}

/// Creates and binds the Apollo compatibility HTTP server.
///
/// The Apollo server provides Apollo Config-compatible API endpoints for
/// configuration management clients that speak the Apollo protocol. It runs
/// on a dedicated port (default 8080) without Nacos authentication middleware,
/// since Apollo uses its own access key system.
pub fn apollo_server(
    app_state: Arc<AppState>,
    apollo_services: ApolloServices,
    address: String,
    port: u16,
) -> Result<Server, std::io::Error> {
    let rate_limit_config = app_state.configuration.rate_limit_config();

    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(RateLimiter::new(rate_limit_config.clone()))
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(apollo_services.notification_service.clone()))
            .app_data(web::Data::new(apollo_services.openapi_service.clone()))
            .app_data(web::Data::new(apollo_services.advanced_service.clone()))
            .service(apollo_config_routes())
            .service(apollo_openapi_routes())
            .service(apollo_advanced_routes())
    })
    .bind((address, port))?
    .run())
}

/// AI services for MCP and A2A APIs.
#[derive(Clone)]
pub struct AIServices {
    pub mcp_registry: Arc<McpServerRegistry>,
    pub agent_registry: Arc<AgentRegistry>,
}

impl AIServices {
    /// Creates AI service adapters with default registries.
    pub fn new() -> Self {
        Self {
            mcp_registry: Arc::new(McpServerRegistry::new()),
            agent_registry: Arc::new(AgentRegistry::new()),
        }
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

/// Apollo services for Apollo Config client compatibility.
#[derive(Clone)]
pub struct ApolloServices {
    pub notification_service: Arc<ApolloNotificationService>,
    pub openapi_service: Arc<ApolloOpenApiService>,
    pub advanced_service: Arc<ApolloAdvancedService>,
}

impl ApolloServices {
    /// Creates Apollo service adapters from a database connection.
    pub fn new(db: Arc<sea_orm::DatabaseConnection>) -> Self {
        Self {
            notification_service: Arc::new(ApolloNotificationService::new(db.clone())),
            openapi_service: Arc::new(ApolloOpenApiService::new(db.clone())),
            advanced_service: Arc::new(ApolloAdvancedService::new(db)),
        }
    }
}

/// Creates and binds the main HTTP server.
///
/// The main server provides the core Nacos-compatible API endpoints
/// including config management, service discovery, AI capabilities,
/// cloud native integrations, metrics, and Swagger UI documentation.
/// Consul and Apollo compatibility are served on dedicated ports.
pub fn main_server(
    app_state: Arc<AppState>,
    naming_service: Arc<NamingService>,
    connection_manager: Arc<ConnectionManager>,
    ai_services: AIServices,
    context_path: String,
    address: String,
    port: u16,
) -> Result<Server, std::io::Error> {
    // Create Cloud services
    let cloud_services = CloudServices::new();
    let rate_limit_config = app_state.configuration.rate_limit_config();

    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(RateLimiter::new(rate_limit_config.clone()))
            .wrap(Authentication)
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(naming_service.clone()))
            .app_data(web::Data::new(connection_manager.clone()))
            // AI services (MCP Server Registry, A2A Agent Registry)
            .app_data(web::Data::new(ai_services.mcp_registry.clone()))
            .app_data(web::Data::new(ai_services.agent_registry.clone()))
            // Cloud services (Prometheus SD, Kubernetes Sync)
            .app_data(web::Data::new(cloud_services.prometheus_sd.clone()))
            .app_data(web::Data::new(cloud_services.k8s_sync.clone()))
            // Swagger UI for API documentation
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
            .service(
                web::scope(&context_path)
                    // Auth routes (needed for SDK authentication)
                    .service(auth::v3::route::routes())
                    // V2 Open API routes (config, naming, cluster only)
                    .service(config_routes())
                    .service(naming_routes())
                    .service(cluster_routes())
                    // V2 Console API routes
                    .service(v2_console_routes())
                    // V3 Console API routes
                    .service(console::v3::route::routes())
                    // V3 Admin API routes
                    .service(v3_admin_routes())
                    // V3 Client API routes
                    .service(v3_client_routes()),
            )
            // AI Capabilities API routes (MCP, A2A)
            .configure(configure_mcp)
            .configure(configure_a2a)
            // Cloud Native Integration API routes (Prometheus SD, Kubernetes Sync)
            .configure(configure_prometheus)
            .configure(configure_kubernetes)
            // Prometheus metrics endpoint at /metrics (standard path)
            .service(metrics_routes())
    })
    .bind((address, port))?
    .run())
}
