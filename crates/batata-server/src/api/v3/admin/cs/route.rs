use actix_web::{Scope, web};

use super::{capacity, config, history, listener, metrics, ops};

pub fn routes() -> Scope {
    web::scope("/cs")
        .service(config::routes())
        .service(ops::routes())
        .service(capacity::routes())
        .service(listener::routes())
        .service(history::routes())
        .service(metrics::routes())
}
