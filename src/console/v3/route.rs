use actix_web::{Scope, web};

use crate::console::v3::namespace;

use super::{auth, health, server_state};

pub fn routes() -> Scope {
    return web::scope("/v3").service(auth::routes()).service(
        web::scope("/console")
            .service(health::routes())
            .service(namespace::routes())
            .service(server_state::routes()),
    );
}
