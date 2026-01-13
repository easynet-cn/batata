// Consul API route configuration
// Combines plugin routes with local export/import routes

use actix_web::web;

// Import local kv module for export/import handlers
use super::kv;

// Re-export plugin route functions
pub use batata_plugin_consul::route::{
    consul_acl_routes, consul_agent_routes, consul_catalog_routes, consul_health_routes,
};

/// Configure Consul KV Store API routes with export/import
/// Returns a scope configured with all key-value store endpoints
/// Including local export/import handlers that need AppState
pub fn consul_kv_routes() -> actix_web::Scope {
    web::scope("/v1")
        // Export/Import - local handlers (must be before wildcard routes)
        .route("/kv/export", web::get().to(kv::export_kv))
        .route("/kv/import", web::put().to(kv::import_kv))
        // KV Store - use wildcard for nested keys (from plugin)
        .route("/kv/{key:.*}", web::get().to(kv::get_kv))
        .route("/kv/{key:.*}", web::put().to(kv::put_kv))
        .route("/kv/{key:.*}", web::delete().to(kv::delete_kv))
        // Transaction
        .route("/txn", web::put().to(kv::txn))
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
