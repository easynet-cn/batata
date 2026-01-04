// Consul API route configuration
// Maps HTTP routes to handler functions

use actix_web::web;

use super::{acl, agent, catalog, health, kv};

/// Configure Consul Agent API routes
/// Returns a scope configured with all agent service endpoints
pub fn consul_agent_routes() -> actix_web::Scope {
    web::scope("/v1/agent")
        // Service registration
        .route("/service/register", web::put().to(agent::register_service))
        // Service deregistration
        .route(
            "/service/deregister/{service_id}",
            web::put().to(agent::deregister_service),
        )
        // List all services
        .route("/services", web::get().to(agent::list_services))
        // Get single service
        .route("/service/{service_id}", web::get().to(agent::get_service))
        // Service maintenance mode
        .route(
            "/service/maintenance/{service_id}",
            web::put().to(agent::set_service_maintenance),
        )
        // Check endpoints
        .route("/check/register", web::put().to(health::register_check))
        .route(
            "/check/deregister/{check_id}",
            web::put().to(health::deregister_check),
        )
        .route("/check/pass/{check_id}", web::put().to(health::pass_check))
        .route("/check/warn/{check_id}", web::put().to(health::warn_check))
        .route("/check/fail/{check_id}", web::put().to(health::fail_check))
        .route(
            "/check/update/{check_id}",
            web::put().to(health::update_check),
        )
        .route("/checks", web::get().to(health::list_agent_checks))
}

/// Configure Consul Health API routes
/// Returns a scope configured with all health check endpoints
pub fn consul_health_routes() -> actix_web::Scope {
    web::scope("/v1/health")
        // Service health (most commonly used)
        .route(
            "/service/{service}",
            web::get().to(health::get_service_health),
        )
        // Service checks
        .route(
            "/checks/{service}",
            web::get().to(health::get_service_checks),
        )
        // Checks by state
        .route("/state/{state}", web::get().to(health::get_checks_by_state))
        // Node checks
        .route("/node/{node}", web::get().to(health::get_node_checks))
}

/// Configure Consul KV Store API routes
/// Returns a scope configured with all key-value store endpoints
pub fn consul_kv_routes() -> actix_web::Scope {
    web::scope("/v1")
        // KV Store - use wildcard for nested keys
        .route("/kv/{key:.*}", web::get().to(kv::get_kv))
        .route("/kv/{key:.*}", web::put().to(kv::put_kv))
        .route("/kv/{key:.*}", web::delete().to(kv::delete_kv))
        // Transaction
        .route("/txn", web::put().to(kv::txn))
}

/// Configure Consul Catalog API routes
/// Returns a scope configured with all catalog endpoints
pub fn consul_catalog_routes() -> actix_web::Scope {
    web::scope("/v1/catalog")
        // List all datacenters
        .route("/datacenters", web::get().to(catalog::list_datacenters))
        // List all services
        .route("/services", web::get().to(catalog::list_services))
        // Get service instances
        .route("/service/{service}", web::get().to(catalog::get_service))
        // List all nodes
        .route("/nodes", web::get().to(catalog::list_nodes))
        // Get node details
        .route("/node/{node}", web::get().to(catalog::get_node))
        // Register to catalog
        .route("/register", web::put().to(catalog::register))
        // Deregister from catalog
        .route("/deregister", web::put().to(catalog::deregister))
}

/// Configure Consul ACL API routes
/// Returns a scope configured with all ACL management endpoints
pub fn consul_acl_routes() -> actix_web::Scope {
    web::scope("/v1/acl")
        // Token management
        .route("/tokens", web::get().to(acl::list_tokens))
        .route("/token", web::put().to(acl::create_token))
        .route("/token/{accessor_id}", web::get().to(acl::get_token))
        .route("/token/{accessor_id}", web::delete().to(acl::delete_token))
        // Policy management
        .route("/policies", web::get().to(acl::list_policies))
        .route("/policy", web::put().to(acl::create_policy))
        .route("/policy/{id}", web::get().to(acl::get_policy))
}

/// Configure all Consul API routes
/// This is the main entry point for integrating Consul API into the server
pub fn consul_routes() -> actix_web::Scope {
    web::scope("")
        .service(consul_agent_routes())
        .service(consul_health_routes())
        .service(consul_kv_routes())
        .service(consul_catalog_routes())
        .service(consul_acl_routes())
}
