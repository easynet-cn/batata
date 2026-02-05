use actix_web::{Scope, web};

use super::{
    a2a, cluster, config, health, history, instance, mcp, metrics, namespace, plugin, server_state,
    service,
};

pub fn routes() -> Scope {
    web::scope("/v3/console")
        .service(cluster::routes())
        .service(health::routes())
        .service(metrics::routes())
        .service(server_state::routes())
        .service(config::routes())
        .service(history::routes())
        .service(namespace::routes())
        .service(service::routes())
        .service(instance::routes())
        .service(mcp::routes())
        .service(a2a::routes())
        .service(plugin::routes())
}
