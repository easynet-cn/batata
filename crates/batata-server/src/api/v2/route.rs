//! V2 API routing configuration
//!
//! This module configures the routes for the Nacos V2 Open API.

use actix_web::{Scope, web};

use super::{
    capacity, client, cluster, config, console_health, core_ops, health, history, instance,
    listener, namespace, naming_catalog, operator, service,
};

/// Create the V2 Config Service routes
///
/// Routes:
/// - GET /nacos/v2/cs/config - Get config
/// - POST /nacos/v2/cs/config - Publish config
/// - DELETE /nacos/v2/cs/config - Delete config
/// - GET /nacos/v2/cs/config/searchDetail - Search config detail
/// - POST /nacos/v2/cs/config/listener - Listen for config changes
/// - GET /nacos/v2/cs/history/list - Get history list
/// - GET /nacos/v2/cs/history - Get specific history entry
/// - GET /nacos/v2/cs/history/detail - Get history detail with version comparison
/// - GET /nacos/v2/cs/history/previous - Get previous history entry
/// - GET /nacos/v2/cs/history/configs - Get all configs in namespace
/// - GET /nacos/v2/cs/capacity - Get capacity
/// - POST /nacos/v2/cs/capacity - Update capacity
/// - DELETE /nacos/v2/cs/capacity - Delete capacity
pub fn config_routes() -> Scope {
    web::scope("/v2/cs")
        .service(
            web::scope("/config")
                .service(config::get_config)
                .service(config::publish_config)
                .service(config::delete_config)
                .service(config::search_config_detail)
                .service(listener::config_listener),
        )
        .service(
            web::scope("/history")
                .service(history::get_history_list)
                .service(history::get_history)
                .service(history::get_history_detail)
                .service(history::get_previous_history)
                .service(history::get_namespace_configs),
        )
        .service(
            web::scope("/capacity")
                .service(capacity::get_capacity)
                .service(capacity::update_capacity)
                .service(capacity::delete_capacity),
        )
}

/// Create the V2 Naming Service routes
///
/// Routes:
/// - POST /nacos/v2/ns/instance - Register instance
/// - DELETE /nacos/v2/ns/instance - Deregister instance
/// - PUT /nacos/v2/ns/instance - Update instance
/// - GET /nacos/v2/ns/instance - Get instance detail
/// - GET /nacos/v2/ns/instance/list - Get instance list
/// - PUT /nacos/v2/ns/instance/metadata/batch - Batch update instance metadata
/// - DELETE /nacos/v2/ns/instance/metadata/batch - Batch delete instance metadata
/// - POST /nacos/v2/ns/service - Create service
/// - DELETE /nacos/v2/ns/service - Delete service
/// - PUT /nacos/v2/ns/service - Update service
/// - GET /nacos/v2/ns/service - Get service detail
/// - GET /nacos/v2/ns/service/list - Get service list
/// - GET /nacos/v2/ns/client/list - Get client list
/// - GET /nacos/v2/ns/client - Get client detail
/// - GET /nacos/v2/ns/client/publish/list - Get client published services
/// - GET /nacos/v2/ns/client/subscribe/list - Get client subscribed services
/// - GET /nacos/v2/ns/client/service/publisher/list - Get service publishers
/// - GET /nacos/v2/ns/client/service/subscriber/list - Get service subscribers
/// - GET /nacos/v2/ns/operator/switches - Get system switches
/// - PUT /nacos/v2/ns/operator/switches - Update system switches
/// - GET /nacos/v2/ns/operator/metrics - Get naming service metrics
/// - PUT /nacos/v2/ns/health/instance - Update instance health status
pub fn naming_routes() -> Scope {
    web::scope("/v2/ns")
        .service(
            web::scope("/instance")
                .service(instance::register_instance)
                .service(instance::deregister_instance)
                .service(instance::update_instance)
                .service(instance::patch_instance)
                .service(instance::get_instance)
                .service(instance::get_instance_list)
                .service(instance::beat_instance)
                .service(instance::get_instance_statuses)
                .service(instance::batch_update_metadata)
                .service(instance::batch_delete_metadata),
        )
        .service(web::scope("/catalog/instances").service(naming_catalog::list_catalog_instances))
        .service(
            web::scope("/service")
                .service(service::create_service)
                .service(service::delete_service)
                .service(service::update_service)
                .service(service::get_service)
                .service(service::get_service_list),
        )
        .service(
            web::scope("/client")
                .service(client::get_client_list)
                .service(client::get_client_detail)
                .service(client::get_client_published_services)
                .service(client::get_client_subscribed_services)
                .service(client::get_service_publishers)
                .service(client::get_service_subscribers),
        )
        .service(
            web::scope("/operator")
                .service(operator::get_switches)
                .service(operator::update_switches)
                .service(operator::get_metrics),
        )
        // Nacos registers operator endpoints under both /operator and /ops (dual path)
        .service(
            web::scope("/ops")
                .route("switches", web::get().to(operator::get_switches_handler))
                .route("switches", web::put().to(operator::update_switches_handler))
                .route("metrics", web::get().to(operator::get_metrics_handler)),
        )
        .service(
            web::scope("/health")
                .service(health::update_instance_health)
                .route(
                    "/instance",
                    web::put().to(health::update_instance_health_handler),
                ),
        )
}

/// Create the V2 Core Cluster routes
///
/// Routes:
/// - GET /nacos/v2/core/cluster/node/self - Get current node
/// - GET /nacos/v2/core/cluster/node/list - Get node list
/// - GET /nacos/v2/core/cluster/node/self/health - Get current node health
/// - PUT /nacos/v2/core/cluster/lookup - Switch lookup mode
pub fn cluster_routes() -> Scope {
    web::scope("/v2/core")
        .service(
            web::scope("/cluster")
                .service(
                    web::scope("/node")
                        .service(cluster::get_node_self)
                        .service(cluster::get_node_list)
                        .service(cluster::get_node_health)
                        .service(cluster::update_node_list),
                )
                .service(web::scope("/nodes").service(cluster::remove_nodes))
                .service(web::scope("/lookup").service(cluster::switch_lookup)),
        )
        .service(core_ops::routes())
}

/// Create the V2 Console routes
///
/// Routes:
/// - GET /nacos/v2/console/namespace/list - List all namespaces
/// - GET /nacos/v2/console/namespace - Get namespace detail
/// - POST /nacos/v2/console/namespace - Create namespace
/// - PUT /nacos/v2/console/namespace - Update namespace
/// - DELETE /nacos/v2/console/namespace - Delete namespace
pub fn console_routes() -> Scope {
    web::scope("/v2/console")
        .service(
            web::scope("/namespace")
                .service(namespace::get_namespace_list)
                .service(namespace::get_namespace)
                .service(namespace::create_namespace)
                .service(namespace::update_namespace)
                .service(namespace::delete_namespace),
        )
        .service(console_health::routes())
}

/// Create all V2 API routes
///
/// This combines all V2 API route groups under the /nacos prefix.
pub fn routes() -> Scope {
    web::scope("/nacos")
        .service(config_routes())
        .service(naming_routes())
        .service(cluster_routes())
        .service(console_routes())
}
