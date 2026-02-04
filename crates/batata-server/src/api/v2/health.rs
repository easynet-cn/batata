//! V2 Health API handlers
//!
//! Implements the Nacos V2 health management API endpoints:
//! - PUT /nacos/v2/ns/health/instance - Update instance health status

use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, put, web};

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured, service::naming::NamingService,
};

use super::model::InstanceHealthParam;

/// Update instance health status
///
/// PUT /nacos/v2/ns/health/instance
///
/// Manually updates the health status of a specific instance.
/// This is typically used for external health checkers.
#[put("")]
pub async fn update_instance_health(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<InstanceHealthParam>,
) -> impl Responder {
    // Validate required parameters
    if params.service_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'serviceName' is missing".to_string(),
            false,
        );
    }

    if params.ip.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'ip' is missing".to_string(),
            false,
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();
    let cluster_name = params.cluster_name_or_default();

    // Check authorization
    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, params.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Check if the instance exists
    let instances = naming_service.get_instances(
        namespace_id,
        group_name,
        &params.service_name,
        cluster_name,
        false,
    );

    let instance_exists = instances
        .iter()
        .any(|i| i.ip == params.ip && i.port == params.port);

    if !instance_exists {
        return Result::<bool>::http_response(
            404,
            404,
            format!(
                "instance {}:{} not found in service {}",
                params.ip, params.port, params.service_name
            ),
            false,
        );
    }

    // Update the instance health status
    let success = naming_service.update_instance_health(
        namespace_id,
        group_name,
        &params.service_name,
        &params.ip,
        params.port,
        cluster_name,
        params.healthy,
    );

    if success {
        tracing::info!(
            namespace_id = %namespace_id,
            group_name = %group_name,
            service_name = %params.service_name,
            ip = %params.ip,
            port = %params.port,
            healthy = %params.healthy,
            "Instance health status updated"
        );
        Result::<bool>::http_success(true)
    } else {
        Result::<bool>::http_response(
            500,
            500,
            "Failed to update instance health status".to_string(),
            false,
        )
    }
}
