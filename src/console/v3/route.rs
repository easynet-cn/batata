use actix_web::{Scope, web};

use super::{cluster, config, health, history, namespace, server_state};

pub fn routes() -> Scope {
    web::scope("/v3/console")
        .service(cluster::routes())
        .service(health::routes())
        .service(server_state::routes())
        .service(config::routes())
        .service(history::routes())
        .service(namespace::routes())
}
