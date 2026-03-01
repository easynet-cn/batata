use actix_web::{Scope, web};

use super::{client, cluster, health, instance, ops, service};

pub fn routes() -> Scope {
    web::scope("/ns")
        .service(service::routes())
        .service(instance::routes())
        .service(cluster::routes())
        .service(health::routes())
        .service(client::routes())
        .service(ops::routes())
}
