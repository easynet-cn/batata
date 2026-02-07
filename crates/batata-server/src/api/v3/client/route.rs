use actix_web::{Scope, web};

use super::{cs, ns};

pub fn client_routes() -> Scope {
    web::scope("/v3/client")
        .service(ns::instance::routes())
        .service(cs::config::routes())
}
