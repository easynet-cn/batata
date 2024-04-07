use actix_web::{web, Scope};

use super::{auth, health, namespace, server_state};

pub fn routers() -> Scope {
    return web::scope("/v1").service(auth::routes()).service(
        web::scope("/console")
            .service(health::routes())
            .service(namespace::routers())
            .service(server_state::routers()),
    );
}
