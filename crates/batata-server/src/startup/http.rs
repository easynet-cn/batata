//! HTTP server setup module for main and console servers.

use std::sync::Arc;

use actix_web::{App, HttpServer, dev::Server, middleware::Logger, web};

use crate::{
    api::consul::{
        AclService, agent::ConsulAgentService, catalog::ConsulCatalogService,
        health::ConsulHealthService, kv::ConsulKVService, route::consul_routes,
    },
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

/// Creates and binds the main HTTP server.
///
/// The main server provides the core Nacos-compatible API endpoints
/// including config management, service discovery, and Consul compatibility.
pub fn main_server(
    app_state: Arc<AppState>,
    naming_service: Arc<NamingService>,
    consul_services: ConsulServices,
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
            .app_data(web::Data::new(naming_service.clone()))
            .app_data(web::Data::new(consul_services.agent.clone()))
            .app_data(web::Data::new(consul_services.health.clone()))
            .app_data(web::Data::new(consul_services.kv.clone()))
            .app_data(web::Data::new(consul_services.catalog.clone()))
            .app_data(web::Data::new(consul_services.acl.clone()))
            .service(
                web::scope(&context_path)
                    .service(auth::v1::route::routes())
                    .service(console::v3::route::routes()),
            )
            .service(consul_routes())
    })
    .bind((address, port))?
    .run())
}
