//! V2 Naming API routing configuration

use actix_web::{Scope, web};

use super::{client, health, instance, naming_catalog, operator, service};

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
