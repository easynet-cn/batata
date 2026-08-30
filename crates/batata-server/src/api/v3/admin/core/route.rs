//! Module `api::v3::admin::core::route` of the `batata-server` crate.
use actix_web::{Scope, web};

use super::{cluster, loader, lock, namespace, ops, state};

/// `routes` function.
///
/// # Returns
/// `Scope`.
pub fn routes() -> Scope {
    web::scope("/core")
        .service(cluster::routes())
        .service(lock::routes())
        .service(namespace::routes())
        .service(ops::routes())
        .service(loader::routes())
        .service(state::routes())
}
