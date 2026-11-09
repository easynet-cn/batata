//! Console V3 routing configuration
//!
//! This module owns the single canonical v3 console route set. Both the
//! production `configure_v3_console_routes` (mounted inside an existing
//! `/v3/console` scope) and the self-contained `v3_routes()` scope delegate to
//! [`configure`], so a test can never silently exercise a different route
//! table than the server.

use actix_web::{Scope, web};

use super::{
    ai_import, audit, cluster, cmdb, config, control, health, history, metrics, namespace, plugin,
    prometheus, server_state, service, sync, tracing_api,
};

/// Register every v3 console scope on an existing `/v3/console` scope.
///
/// `batata-server` calls this via `configure_v3_console_routes` and then adds
/// the AI scopes it owns. Keep the list here, not in the server, so both
/// mounts stay identical.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(cluster::routes())
        .service(health::routes())
        .service(metrics::routes())
        .service(server_state::routes())
        .service(config::routes())
        .service(history::routes())
        .service(namespace::routes())
        .service(service::routes())
        .service(audit::routes())
        .service(sync::routes())
        .service(tracing_api::routes())
        .service(control::routes())
        .service(cmdb::routes())
        .service(prometheus::routes())
        .service(plugin::routes())
        // AI resource importer (console mirror of the admin endpoints).
        // Fully-qualified scope on purpose: a bare `/ai` scope would shadow
        // the sibling `/ai/a2a`, `/ai/mcp`, ... scopes registered by the server.
        .service(ai_import::console_routes());
}

/// Create a self-contained `/v3/console` scope with the canonical route set.
pub fn routes() -> Scope {
    web::scope("/v3/console").configure(configure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, test};

    /// One GET probe per registered scope. A missing scope shows up as 404.
    const ROUTE_PROBES: &[&str] = &[
        "/v3/console/core/cluster/nodes",
        "/v3/console/health/liveness",
        "/v3/console/metrics",
        "/v3/console/server/state",
        "/v3/console/cs/config",
        "/v3/console/cs/history/list",
        "/v3/console/core/namespace/list",
        "/v3/console/ns/service/list",
        "/v3/console/audit/logs",
        "/v3/console/sync/environments",
        "/v3/console/trace/list",
        "/v3/console/control/stats",
        "/v3/console/cmdb/entities",
        "/v3/console/prometheus/query",
        "/v3/console/plugin/list",
        "/v3/console/ai/import/sources",
    ];

    #[actix_web::test]
    async fn production_and_standalone_route_tables_are_identical() {
        let production = test::init_service(
            App::new()
                .service(web::scope("/v3/console").configure(crate::configure_v3_console_routes)),
        )
        .await;
        let standalone = test::init_service(App::new().service(routes())).await;

        for path in ROUTE_PROBES {
            let prod =
                test::call_service(&production, test::TestRequest::get().uri(path).to_request())
                    .await
                    .status();
            let standalone_status =
                test::call_service(&standalone, test::TestRequest::get().uri(path).to_request())
                    .await
                    .status();

            assert_ne!(prod.as_u16(), 404, "production route {path} is missing");
            assert_eq!(
                prod, standalone_status,
                "route parity mismatch for {path}: production vs v3_routes()"
            );
        }
    }
}
