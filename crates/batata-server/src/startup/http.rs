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
        AclService, agent::ConsulAgentService, catalog::ConsulCatalogService,
        health::ConsulHealthService, kv::ConsulKVService, route::consul_routes,
    },
    api::openapi::ApiDoc,
    api::v2::route::routes as v2_routes,
    auth, console,
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
}

impl ConsulServices {
    /// Creates Consul service adapters from a naming service.
    pub fn new(naming_service: Arc<NamingService>) -> Self {
        Self {
            agent: ConsulAgentService::new(naming_service.clone()),
            health: ConsulHealthService::new(naming_service.clone()),
            kv: ConsulKVService::new(),
            catalog: ConsulCatalogService::new(naming_service),
            acl: AclService::new(),
        }
    }
}

/// Creates and binds the console HTTP server.
///
/// The console server provides administrative endpoints for managing
/// the Batata cluster, including authentication and namespace management.
pub fn console_server(
    app_state: Arc<AppState>,
    context_path: String,
    address: String,
    port: u16,
) -> Result<Server, std::io::Error> {
    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(RateLimiter::with_defaults())
            .wrap(Authentication)
            .app_data(web::Data::from(app_state.clone()))
            .service(
                web::scope(&context_path)
                    .service(auth::v3::route::routes())
                    .service(console::v3::route::routes()),
            )
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
/// including config management, service discovery, Consul compatibility,
/// and Apollo Config client compatibility.
/// It also serves Swagger UI at `/swagger-ui/` for API documentation.
pub fn main_server(
    app_state: Arc<AppState>,
    naming_service: Arc<NamingService>,
    connection_manager: Arc<ConnectionManager>,
    consul_services: ConsulServices,
    apollo_services: ApolloServices,
    context_path: String,
    address: String,
    port: u16,
) -> Result<Server, std::io::Error> {
    // Create AI and Cloud services
    let ai_services = AIServices::new();
    let cloud_services = CloudServices::new();

    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(RateLimiter::with_defaults())
            .wrap(Authentication)
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(naming_service.clone()))
            .app_data(web::Data::new(connection_manager.clone()))
            .app_data(web::Data::new(consul_services.agent.clone()))
            .app_data(web::Data::new(consul_services.health.clone()))
            .app_data(web::Data::new(consul_services.kv.clone()))
            .app_data(web::Data::new(consul_services.catalog.clone()))
            .app_data(web::Data::new(consul_services.acl.clone()))
            // Apollo services (Notification, OpenAPI, Advanced)
            .app_data(web::Data::new(apollo_services.notification_service.clone()))
            .app_data(web::Data::new(apollo_services.openapi_service.clone()))
            .app_data(web::Data::new(apollo_services.advanced_service.clone()))
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
                    .service(auth::v1::route::routes())
                    .service(console::v3::route::routes()),
            )
            // V2 Open API routes under /nacos prefix
            .service(v2_routes())
            .service(consul_routes())
            // Apollo Config API routes
            .service(apollo_config_routes())
            // Apollo Open API and Advanced routes
            .service(apollo_openapi_routes())
            .service(apollo_advanced_routes())
            // AI Capabilities API routes (MCP, A2A)
            .configure(configure_mcp)
            .configure(configure_a2a)
            // Cloud Native Integration API routes (Prometheus SD, Kubernetes Sync)
            .configure(configure_prometheus)
            .configure(configure_kubernetes)
    })
    .bind((address, port))?
    .run())
}
