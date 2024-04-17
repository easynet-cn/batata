use actix_web::{web, Scope};

use super::{config, health};

pub fn routers() -> Scope {
    return web::scope("/v2")
        .service(config::routers())
        .service(web::scope("/console").service(health::routers()));
}
