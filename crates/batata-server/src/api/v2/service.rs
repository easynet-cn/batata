//! V2 Service API handlers
//!
//! Implements the Nacos V2 service management API endpoints:
//! - POST /nacos/v2/ns/service - Create service
//! - DELETE /nacos/v2/ns/service - Delete service
//! - PUT /nacos/v2/ns/service - Update service
//! - GET /nacos/v2/ns/service - Get service detail
//! - GET /nacos/v2/ns/service/list - Get service list

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, delete, get, post, put, web};
use tracing::info;

use batata_naming::service::ServiceMetadata;

use crate::{
    ActionTypes, ApiType, Secured, SignType, error, model::common::AppState,
    model::response::Result, secured, service::naming::NamingService,
};

use super::model::{
    SelectorResponse, ServiceCreateParam, ServiceDeleteParam, ServiceDetailParam,
    ServiceDetailResponse, ServiceListParam, ServiceListResponse, ServiceUpdateParam,
};

/// Create service
///
/// POST /nacos/v2/ns/service
///
/// Creates a new service.
#[post("")]
pub async fn create_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Form<ServiceCreateParam>,
) -> impl Responder {
    // Validate required parameters
    if form.service_name.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'serviceName' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = form.namespace_id_or_default();
    let group_name = form.group_name_or_default();

    // Check authorization
    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, form.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Check if service already exists
    if naming_service.service_exists(namespace_id, group_name, &form.service_name) {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            format!("service {} already exists", form.service_name),
            String::new(),
        );
    }

    // Parse metadata if provided
    let metadata: HashMap<String, String> = form
        .metadata
        .as_ref()
        .and_then(|m| serde_json::from_str(m).ok())
        .unwrap_or_default();

    // Parse selector
    let (selector_type, selector_expression) = if let Some(selector) = &form.selector {
        // Selector format: {"type": "label", "expression": "..."}
        let selector_obj: serde_json::Value = serde_json::from_str(selector).unwrap_or_default();
        (
            selector_obj["type"].as_str().unwrap_or("none").to_string(),
            selector_obj["expression"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        )
    } else {
        ("none".to_string(), String::new())
    };

    // Create service metadata
    let service_metadata = ServiceMetadata {
        protect_threshold: form.protect_threshold.unwrap_or(0.0),
        metadata,
        selector_type,
        selector_expression,
        ..Default::default()
    };

    // Set service metadata (this creates the service)
    naming_service.set_service_metadata(
        namespace_id,
        group_name,
        &form.service_name,
        service_metadata,
    );

    info!(
        namespace_id = %namespace_id,
        group_name = %group_name,
        service_name = %form.service_name,
        "Service created successfully"
    );

    Result::<String>::http_success("ok".to_string())
}

/// Delete service
///
/// DELETE /nacos/v2/ns/service
///
/// Deletes a service. Service must have no instances.
#[delete("")]
pub async fn delete_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<ServiceDeleteParam>,
) -> impl Responder {
    // Validate required parameters
    if params.service_name.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'serviceName' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

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

    // Check if service exists
    if !naming_service.service_exists(namespace_id, group_name, &params.service_name) {
        return Result::<String>::http_response(
            404,
            error::SERVICE_NOT_EXIST.code,
            format!("service {} not found", params.service_name),
            String::new(),
        );
    }

    // Check if service has instances
    let instances =
        naming_service.get_instances(namespace_id, group_name, &params.service_name, "", false);

    if !instances.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            format!(
                "service {} has {} instances, cannot delete",
                params.service_name,
                instances.len()
            ),
            String::new(),
        );
    }

    // Delete service metadata
    naming_service.delete_service_metadata(namespace_id, group_name, &params.service_name);

    info!(
        namespace_id = %namespace_id,
        group_name = %group_name,
        service_name = %params.service_name,
        "Service deleted successfully"
    );

    Result::<String>::http_success("ok".to_string())
}

/// Update service
///
/// PUT /nacos/v2/ns/service
///
/// Updates an existing service.
#[put("")]
pub async fn update_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Form<ServiceUpdateParam>,
) -> impl Responder {
    // Validate required parameters
    if form.service_name.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'serviceName' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = form.namespace_id_or_default();
    let group_name = form.group_name_or_default();

    // Check authorization
    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, form.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Check if service exists
    if !naming_service.service_exists(namespace_id, group_name, &form.service_name) {
        return Result::<String>::http_response(
            404,
            error::SERVICE_NOT_EXIST.code,
            format!("service {} not found", form.service_name),
            String::new(),
        );
    }

    // Update protect threshold if provided
    if let Some(threshold) = form.protect_threshold {
        naming_service.update_service_protect_threshold(
            namespace_id,
            group_name,
            &form.service_name,
            threshold,
        );
    }

    // Update metadata if provided
    if let Some(metadata_str) = &form.metadata
        && let Ok(metadata) = serde_json::from_str::<HashMap<String, String>>(metadata_str)
    {
        naming_service.update_service_metadata_map(
            namespace_id,
            group_name,
            &form.service_name,
            metadata,
        );
    }

    // Update selector if provided
    if let Some(selector) = &form.selector {
        let selector_obj: serde_json::Value = serde_json::from_str(selector).unwrap_or_default();
        let selector_type = selector_obj["type"].as_str().unwrap_or("none");
        let selector_expression = selector_obj["expression"].as_str().unwrap_or("");

        naming_service.update_service_selector(
            namespace_id,
            group_name,
            &form.service_name,
            selector_type,
            selector_expression,
        );
    }

    info!(
        namespace_id = %namespace_id,
        group_name = %group_name,
        service_name = %form.service_name,
        "Service updated successfully"
    );

    Result::<String>::http_success("ok".to_string())
}

/// Get service detail
///
/// GET /nacos/v2/ns/service
///
/// Retrieves details of a specific service.
#[get("")]
pub async fn get_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<ServiceDetailParam>,
) -> impl Responder {
    // Validate required parameters
    if params.service_name.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'serviceName' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

    // Check authorization
    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, params.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Check if service exists
    if !naming_service.service_exists(namespace_id, group_name, &params.service_name) {
        return Result::<Option<ServiceDetailResponse>>::http_response(
            404,
            error::SERVICE_NOT_EXIST.code,
            format!("service {} not found", params.service_name),
            None::<ServiceDetailResponse>,
        );
    }

    // Get service metadata
    let metadata_opt =
        naming_service.get_service_metadata(namespace_id, group_name, &params.service_name);

    // Get instances to calculate counts
    let instances =
        naming_service.get_instances(namespace_id, group_name, &params.service_name, "", false);

    // Calculate cluster count
    let clusters: std::collections::HashSet<_> =
        instances.iter().map(|i| i.cluster_name.clone()).collect();

    let (protect_threshold, metadata, selector) = if let Some(meta) = metadata_opt {
        let selector = if meta.selector_type != "none" && !meta.selector_type.is_empty() {
            Some(SelectorResponse {
                r#type: meta.selector_type,
                expression: if meta.selector_expression.is_empty() {
                    None
                } else {
                    Some(meta.selector_expression)
                },
            })
        } else {
            None
        };

        (
            meta.protect_threshold,
            if meta.metadata.is_empty() {
                None
            } else {
                Some(meta.metadata)
            },
            selector,
        )
    } else {
        (0.0, None, None)
    };

    // Build cluster map from cluster names
    let cluster_map: Option<std::collections::HashMap<String, serde_json::Value>> =
        if clusters.is_empty() {
            None
        } else {
            let map: std::collections::HashMap<String, serde_json::Value> = clusters
                .iter()
                .map(|c| (c.clone(), serde_json::json!({})))
                .collect();
            Some(map)
        };

    let response = ServiceDetailResponse {
        namespace: namespace_id.to_string(),
        group_name: group_name.to_string(),
        service_name: params.service_name.clone(),
        protect_threshold,
        metadata,
        selector,
        cluster_map,
        ephemeral: true, // Default to ephemeral
    };

    Result::<ServiceDetailResponse>::http_success(response)
}

/// Get service list
///
/// GET /nacos/v2/ns/service/list
///
/// Retrieves list of services with pagination.
#[get("list")]
pub async fn get_service_list(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<ServiceListParam>,
) -> impl Responder {
    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

    // Check authorization for namespace-level access
    let resource = format!("{}:{}:naming/*", namespace_id, group_name);
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Cap page_size at 500
    let page_size = params.page_size.min(500);

    // Get service list
    let (total_count, service_names) = naming_service.list_services(
        namespace_id,
        group_name,
        params.page_no as i32,
        page_size as i32,
    );

    let response = ServiceListResponse {
        count: total_count,
        services: service_names,
    };

    Result::<ServiceListResponse>::http_success(response)
}
