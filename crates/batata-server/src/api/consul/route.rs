// Consul API route configuration
// Combines plugin routes with local export/import routes

use actix_web::web;

// Import local kv module for export/import handlers
use super::kv;

// Re-export plugin route functions
pub use batata_plugin_consul::route::routes;

/// Configure Consul KV API routes under /v1 scope.
///
/// Previously also mounted `/v1/lock/*` and `/v1/semaphore/*`, but those
/// endpoints do not exist in upstream Consul — locks and semaphores are
/// client-side constructs built on `/v1/kv/?acquire=<sid>` + `/v1/session/*`.
/// The self-coined endpoints have been removed to avoid protocol drift and
/// to prevent the old handlers from polluting user KV namespace with
/// synthetic `<key>.lock` entries.
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
}
