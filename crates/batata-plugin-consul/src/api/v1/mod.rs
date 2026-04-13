pub mod acl;
pub mod agent;
pub mod catalog;
pub mod config_entry;
pub mod connect;
pub mod connect_ca;
pub mod coordinate;
pub mod event;
pub mod health;
pub mod internal;
pub mod kv;
pub mod lock;
pub mod namespace;
pub mod operator;
pub mod peering;
pub mod query;
pub mod session;
pub mod snapshot;
pub mod status;

use actix_web::web;

/// Unified Consul API v1 routes (standalone/fallback mode).
///
/// All routes are under a single `/v1` scope to avoid actix-web scope shadowing.
/// Each sub-module uses a unique prefix (e.g., `/agent`, `/health`, `/kv`).
pub fn routes() -> actix_web::Scope {
    web::scope("/v1")
        .service(acl::routes())
        .service(agent::routes())
        .service(catalog::routes())
        .service(config_entry::routes())
        .service(connect::routes())
        .service(connect::exported_services_resource())
        .service(connect::imported_services_resource())
        .service(connect_ca::routes())
        .service(coordinate::routes())
        .service(event::routes())
        .service(health::routes())
        .service(internal::routes())
        .service(kv::routes())
        .service(kv::txn_resource())
        .service(lock::routes())
        .service(lock::semaphore_routes())
        .service(namespace::namespace_routes())
        .service(namespace::namespaces_routes())
        .service(operator::routes())
        .service(peering::routes())
        .service(peering::list_resource())
        .service(query::routes())
        .service(session::routes())
        .service(snapshot::routes())
        .service(status::routes())
}

/// Cluster-aware Consul API v1 routes.
///
/// Uses real ClusterManager-based handlers for agent, status, operator,
/// and internal endpoints. Other routes remain the same.
pub fn routes_real() -> actix_web::Scope {
    web::scope("/v1")
        .service(acl::routes())
        .service(agent::routes_real())
        .service(catalog::routes())
        .service(config_entry::routes())
        .service(connect::routes())
        .service(connect::exported_services_resource())
        .service(connect::imported_services_resource())
        .service(connect_ca::routes())
        .service(coordinate::routes())
        .service(event::routes())
        .service(health::routes())
        .service(internal::routes_real())
        .service(kv::routes())
        .service(kv::txn_resource())
        .service(lock::routes())
        .service(lock::semaphore_routes())
        .service(namespace::namespace_routes())
        .service(namespace::namespaces_routes())
        .service(operator::routes_real())
        .service(peering::routes())
        .service(peering::list_resource())
        .service(query::routes())
        .service(session::routes())
        .service(snapshot::routes())
        .service(status::routes_real())
}
