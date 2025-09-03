use actix_web::{Scope, web};

use super::{auth, config, health, namespace, server_state};

pub fn routes() -> Scope {
    web::scope("/v3").service(auth::routes()).service(
        web::scope("/console")
            .service(health::routes())
            .service(server_state::routes())
            .service(config::routes())
            .service(namespace::routes()),
    )
}
