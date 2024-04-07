use actix_web::{web, Scope};

use super::health;

pub fn routers() -> Scope {
    return web::scope("/v2").service(web::scope("/console").service(health::routes()));
}
