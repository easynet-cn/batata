//! V2 Console routing configuration

use actix_web::{Scope, web};

use super::{health, namespace};

/// Create the V2 Console routes
///
/// Routes:
/// - GET /v2/console/namespace/list - List all namespaces
/// - GET /v2/console/namespace - Get namespace detail
/// - POST /v2/console/namespace - Create namespace
/// - PUT /v2/console/namespace - Update namespace
/// - DELETE /v2/console/namespace - Delete namespace
/// - GET /v2/console/health/liveness - Check if server is alive
/// - GET /v2/console/health/readiness - Check if server is ready
pub fn routes() -> Scope {
    web::scope("/v2/console")
        .service(
            web::scope("/namespace")
                .service(namespace::get_namespace_list)
                .service(namespace::get_namespace)
                .service(namespace::create_namespace)
                .service(namespace::update_namespace)
                .service(namespace::delete_namespace),
        )
        .service(health::routes())
}
