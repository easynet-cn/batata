use actix_web::{Scope, web};

use super::{cluster, loader, lock, namespace, ops, state};

pub fn routes() -> Scope {
    web::scope("/core")
        .service(cluster::routes())
        .service(lock::routes())
        .service(namespace::routes())
        .service(ops::routes())
        .service(loader::routes())
        .service(state::routes())
}
