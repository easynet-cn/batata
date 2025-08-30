use actix_web::{Scope, web};

use super::{config, health};

pub fn routes() -> Scope {
    return web::scope("/v2")
        .service(config::routes())
        .service(web::scope("/console").service(health::routes()));
}
