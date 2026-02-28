//! Batata Console - Management console backend
//!
//! This crate provides:
//! - Console API endpoints
//! - Data source abstraction (local/remote)
//! - Cluster management UI API
//! - Metrics API

pub mod datasource;
pub mod model;
pub mod v3;

// Re-export commonly used types
pub use datasource::{
    ConsoleDataSource, create_datasource, embedded::EmbeddedLocalDataSource,
    local::LocalDataSource, remote::RemoteDataSource,
};
pub use model::*;

// Re-export v3 routes and metrics
pub use v3::metrics::{METRICS, Metrics};
pub use v3::route::routes as v3_routes;
pub use v3::server_state::ServerStateConfig;

/// Configure v3 console routes on a given ServiceConfig.
/// This allows composing console routes with additional routes (e.g., AI handlers)
/// in the same `/v3/console` scope.
pub fn configure_v3_console_routes(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(v3::cluster::routes())
        .service(v3::health::routes())
        .service(v3::metrics::routes())
        .service(v3::server_state::routes())
        .service(v3::config::routes())
        .service(v3::history::routes())
        .service(v3::namespace::routes())
        .service(v3::service::routes());
}
