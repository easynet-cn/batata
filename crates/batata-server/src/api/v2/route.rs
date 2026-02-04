//! V2 API routing configuration
//!
//! This module configures the routes for the Nacos V2 Open API.

use actix_web::{Scope, web};

use super::{client, cluster, config, health, history, instance, namespace, operator, service};

/// Create the V2 Config Service routes
///
/// Routes:
/// - GET /nacos/v2/cs/config - Get config
/// - POST /nacos/v2/cs/config - Publish config
/// - DELETE /nacos/v2/cs/config - Delete config
/// - GET /nacos/v2/cs/history/list - Get history list
/// - GET /nacos/v2/cs/history - Get specific history entry
/// - GET /nacos/v2/cs/history/previous - Get previous history entry
/// - GET /nacos/v2/cs/history/configs - Get all configs in namespace
pub fn config_routes() -> Scope {
    web::scope("/v2/cs")
        .service(
            web::scope("/config")
                .service(config::get_config)
                .service(config::publish_config)
                .service(config::delete_config),
        )
        .service(
            web::scope("/history")
                .service(history::get_history_list)
                .service(history::get_history)
                .service(history::get_previous_history)
                .service(history::get_namespace_configs),
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
                .service(instance::get_instance)
                .service(instance::get_instance_list)
                .service(instance::batch_update_metadata)
                .service(instance::batch_delete_metadata),
        )
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
        .service(web::scope("/health/instance").service(health::update_instance_health))
}

/// Create the V2 Core Cluster routes
///
/// Routes:
/// - GET /nacos/v2/core/cluster/node/self - Get current node
/// - GET /nacos/v2/core/cluster/node/list - Get node list
/// - GET /nacos/v2/core/cluster/node/self/health - Get current node health
/// - PUT /nacos/v2/core/cluster/lookup - Switch lookup mode
pub fn cluster_routes() -> Scope {
    web::scope("/v2/core/cluster")
        .service(
            web::scope("/node")
                .service(cluster::get_node_self)
                .service(cluster::get_node_list)
                .service(cluster::get_node_health),
        )
        .service(web::scope("/lookup").service(cluster::switch_lookup))
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
    web::scope("/v2/console").service(
        web::scope("/namespace")
            .service(namespace::get_namespace_list)
            .service(namespace::get_namespace)
            .service(namespace::create_namespace)
            .service(namespace::update_namespace)
            .service(namespace::delete_namespace),
    )
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
