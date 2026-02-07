use actix_web::{Scope, web};

use super::{cs, ns};

pub fn client_routes() -> Scope {
    web::scope("/v3/client")
        .service(web::scope("/ns").service(ns::instance::routes()))
        .service(web::scope("/cs").service(cs::config::routes()))
}
