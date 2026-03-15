//! Console V3 routing configuration
//!
//! This module provides the non-AI console routes.
//! AI handlers (mcp, a2a, plugin) remain in batata-server.

use actix_web::{Scope, web};

use super::{
    audit, cluster, config, health, history, metrics, namespace, server_state, service, sync,
    tracing_api,
};

/// Create the v3 console routes (non-AI only)
pub fn routes() -> Scope {
    web::scope("/v3/console")
        .service(cluster::routes())
        .service(health::routes())
        .service(metrics::routes())
        .service(server_state::routes())
        .service(config::routes())
        .service(history::routes())
        .service(namespace::routes())
        .service(service::routes())
        .service(audit::routes())
        .service(sync::routes())
        .service(tracing_api::routes())
}
