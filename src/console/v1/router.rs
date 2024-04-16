use actix_web::{web, Scope};

use super::{auth, config, health, namespace, server_state};

pub fn routers() -> Scope {
    return web::scope("/v1")
        .service(auth::routers())
        .service(config::routers())
        .service(
            web::scope("/console")
                .service(health::routers())
                .service(namespace::routers())
                .service(server_state::routers()),
        );
}
