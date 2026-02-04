//! Console V3 routing configuration

use actix_web::{Scope, web};

use super::{
    cluster, config, health, history, instance, metrics, namespace, server_state, service,
};

/// Create the v3 console routes
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
        .service(instance::routes())
}
