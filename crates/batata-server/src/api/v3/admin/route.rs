//! Module `api::v3::admin::route` of the `batata-server` crate.
use actix_web::{Scope, web};

use super::{ai, core, cs, ns};

/// `admin_routes` function.
///
/// # Returns
/// `Scope`.
pub fn admin_routes() -> Scope {
    web::scope("/v3/admin")
        .service(ns::route::routes())
        .service(cs::route::routes())
        .service(core::route::routes())
        .service(ai::route::routes())
}
