//! The `api::v3::admin::route` module.
use actix_web::{Scope, web};

use super::{client, cluster, health, instance, ops, service};

/// Performs the `routes` operation.
pub fn routes() -> Scope {
    web::scope("/ns")
        .service(service::routes())
        .service(instance::routes())
        .service(cluster::routes())
        .service(health::routes())
        .service(client::routes())
        .service(ops::routes())
}
