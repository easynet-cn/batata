//! V2 Health API handlers
//!
//! Implements the Nacos V2 health management API endpoints:
//! - PUT /nacos/v2/ns/health - Update instance health status
//! - PUT /nacos/v2/ns/health/instance - Update instance health status (alternative path)
//!
//! Nacos registers this handler at both paths: `@PutMapping(value = {"", "/instance"})`.

use std::sync::Arc;

use actix_web::{HttpRequest, Responder, put, web};

use batata_common::{ActionTypes, ApiType, SignType};
use batata_server_common::error;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};

use batata_api::naming::NamingServiceProvider;

use super::model::InstanceHealthParam;

/// Shared implementation for updating instance health status.
async fn do_update_instance_health(
    req: &HttpRequest,
    data: &web::Data<AppState>,
    naming_service: &web::Data<Arc<dyn NamingServiceProvider>>,
    form: &InstanceHealthParam,
) -> actix_web::HttpResponse {
    // Validate required parameters
    if form.service_name.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'serviceName' is missing".to_string(),
            String::new(),
        );
    }

    if form.ip.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'ip' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = form.namespace_id_or_default();
    let group_name = form.group_name_or_default();
    let cluster_name = form.cluster_name_or_default();

    // Check authorization
    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, form.service_name
    );
    secured!(
        Secured::builder(req, data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Check if the instance exists (only Batata-registered instances)
    let instances = naming_service.get_instances(
        namespace_id,
        group_name,
        &form.service_name,
        cluster_name,
        false,
    );

    let instance_exists = instances
        .iter()
        .any(|i| i.ip == form.ip && i.port == form.port);

    if !instance_exists {
        return Result::<String>::http_response(
            404,
            error::INSTANCE_NOT_FOUND.code,
            format!(
                "instance {}:{} not found in service {}",
                form.ip, form.port, form.service_name
            ),
            String::new(),
        );
    }

    // Update the instance health status
    let success = naming_service.update_instance_health(
        namespace_id,
        group_name,
        &form.service_name,
        &form.ip,
        form.port,
        cluster_name,
        form.healthy,
    );

    if success {
        tracing::info!(
            namespace_id = %namespace_id,
            group_name = %group_name,
            service_name = %form.service_name,
            ip = %form.ip,
            port = %form.port,
            healthy = %form.healthy,
            "Instance health status updated"
        );
        Result::<String>::http_success("ok".to_string())
    } else {
        Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            "Failed to update instance health status".to_string(),
            String::new(),
        )
    }
}

/// Update instance health status
///
/// PUT /nacos/v2/ns/health
///
/// Manually updates the health status of a specific instance.
/// This is typically used for external health checkers.
/// Also registered at PUT /nacos/v2/ns/health/instance via route alias.
#[put("")]
pub async fn update_instance_health(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    form: web::Form<InstanceHealthParam>,
) -> impl Responder {
    do_update_instance_health(&req, &data, &naming_service, &form).await
}

/// Plain handler for PUT /nacos/v2/ns/health/instance (dual-path alias).
/// Delegates to the same logic as `update_instance_health`.
pub async fn update_instance_health_handler(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    form: web::Form<InstanceHealthParam>,
) -> impl Responder {
    do_update_instance_health(&req, &data, &naming_service, &form).await
}
