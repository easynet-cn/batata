use actix_web::{Scope, web};

use crate::console::v3::namespace;

use super::{auth, health, server_state};

pub fn routers() -> Scope {
    return web::scope("/v3").service(auth::routers()).service(
        web::scope("/console")
            .service(health::routers())
            .service(namespace::routers())
            .service(server_state::routers()),
    );
}
