use actix_web::{Scope, web};

use super::{cluster, loader, namespace, ops, state};

pub fn routes() -> Scope {
    web::scope("/core")
        .service(cluster::routes())
        .service(namespace::routes())
        .service(ops::routes())
        .service(loader::routes())
        .service(state::routes())
}
