// Consul API route configuration
// Combines plugin routes with local export/import routes

use actix_web::web;

// Import local kv module for export/import handlers
use super::kv;
// Import lock module from plugin for lock/semaphore handlers
use batata_plugin_consul::lock;

// Re-export plugin route functions
pub use batata_plugin_consul::route::{
    consul_acl_routes, consul_agent_routes, consul_catalog_routes, consul_event_routes,
    consul_health_routes, consul_query_routes, consul_session_routes, consul_status_routes,
};

/// Configure Consul KV, Lock, and Semaphore API routes under /v1 scope.
///
/// This merges KV store, lock, and semaphore routes into a single /v1 scope
/// to avoid actix-web scope conflicts. Multiple scopes with the same prefix
/// cause the first one to capture all matching requests.
pub fn consul_kv_and_lock_routes() -> actix_web::Scope {
    web::scope("/v1")
        // Export/Import - local handlers (must be before wildcard KV routes)
        .route("/kv/export", web::get().to(kv::export_kv))
        .route("/kv/import", web::put().to(kv::import_kv))
        // KV Store - use wildcard for nested keys (from plugin)
        .route("/kv/{key:.*}", web::get().to(kv::get_kv))
        .route("/kv/{key:.*}", web::put().to(kv::put_kv))
        .route("/kv/{key:.*}", web::delete().to(kv::delete_kv))
        // Transaction
        .route("/txn", web::put().to(kv::txn))
        // Lock endpoints
        .route("/lock/acquire", web::post().to(lock::acquire_lock))
        .route("/lock/release/{key:.*}", web::put().to(lock::release_lock))
        .route("/lock/renew/{key:.*}", web::put().to(lock::renew_lock))
        .route("/lock/{key:.*}", web::get().to(lock::get_lock))
        .route("/lock/{key:.*}", web::delete().to(lock::destroy_lock))
        // Semaphore endpoints
        .route(
            "/semaphore/acquire",
            web::post().to(lock::acquire_semaphore),
        )
        .route(
            "/semaphore/release/{prefix:.*}",
            web::put().to(lock::release_semaphore),
        )
        .route("/semaphore/{prefix:.*}", web::get().to(lock::get_semaphore))
}

/// Configure all Consul API routes
/// This is the main entry point for integrating Consul API into the server.
///
/// IMPORTANT: The /v1 scope (KV + lock + semaphore) MUST be registered LAST
/// because it matches all /v1/* requests. More specific scopes like /v1/agent,
/// /v1/session etc. must be tried first.
pub fn consul_routes() -> actix_web::Scope {
    web::scope("")
        // Specific /v1/* scopes first (these won't conflict)
        .service(consul_agent_routes())
        .service(consul_health_routes())
        .service(consul_catalog_routes())
        .service(consul_acl_routes())
        .service(consul_session_routes())
        .service(consul_status_routes())
        .service(consul_event_routes())
        .service(consul_query_routes())
        // Broad /v1 scope LAST (KV + lock + semaphore merged to avoid conflicts)
        .service(consul_kv_and_lock_routes())
}
