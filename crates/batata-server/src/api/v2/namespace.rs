//! V2 Namespace API handlers
//!
//! Implements the Nacos V2 namespace management API endpoints:
//! - GET /nacos/v2/console/namespace/list - List all namespaces
//! - GET /nacos/v2/console/namespace - Get namespace detail
//! - POST /nacos/v2/console/namespace - Create namespace
//! - PUT /nacos/v2/console/namespace - Update namespace
//! - DELETE /nacos/v2/console/namespace - Delete namespace

use std::sync::LazyLock;

use actix_web::{HttpMessage, HttpRequest, Responder, delete, get, post, put, web};

use batata_config::Namespace;

use crate::{
    ActionTypes, ApiType, Secured, SignType, error, model::common::AppState,
    model::response::Result, secured, service,
};

use super::model::{
    NamespaceCreateParam, NamespaceDeleteParam, NamespaceDetailParam, NamespaceResponse,
    NamespaceUpdateParam,
};

/// Regex for validating namespace IDs
static NAMESPACE_ID_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[\w-]+$").expect("Invalid regex pattern"));

/// Regex for validating namespace names (no special chars)
static NAMESPACE_NAME_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[^@#$%^&*]+$").expect("Invalid regex pattern"));

/// Maximum length for namespace ID
const NAMESPACE_ID_MAX_LENGTH: usize = 128;

/// List all namespaces
///
/// GET /nacos/v2/console/namespace/list
///
/// Returns a list of all namespaces.
#[get("list")]
pub async fn get_namespace_list(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    // Check authorization
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let namespaces: Vec<Namespace> = service::namespace::find_all(data.db()).await;

    // Convert to response format
    let response: Vec<NamespaceResponse> = namespaces
        .into_iter()
        .map(|ns| NamespaceResponse {
            namespace: ns.namespace,
            namespace_show_name: ns.namespace_show_name,
            namespace_desc: ns.namespace_desc,
            quota: ns.quota,
            config_count: ns.config_count,
            type_: ns.type_,
        })
        .collect();

    Result::<Vec<NamespaceResponse>>::http_success(response)
}

/// Get namespace detail
///
/// GET /nacos/v2/console/namespace
///
/// Returns details of a specific namespace.
#[get("")]
pub async fn get_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<NamespaceDetailParam>,
) -> impl Responder {
    // Check authorization
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::OpenApi)
            .build()
    );

    if params.namespace_id.is_empty() {
        return Result::<NamespaceResponse>::http_response(
            400,
            400,
            "Required parameter 'namespaceId' is missing".to_string(),
            NamespaceResponse {
                namespace: String::new(),
                namespace_show_name: String::new(),
                namespace_desc: String::new(),
                quota: 0,
                config_count: 0,
                type_: 0,
            },
        );
    }

    match service::namespace::get_by_namespace_id(data.db(), &params.namespace_id, "1").await {
        Ok(ns) => {
            let response = NamespaceResponse {
                namespace: ns.namespace,
                namespace_show_name: ns.namespace_show_name,
                namespace_desc: ns.namespace_desc,
                quota: ns.quota,
                config_count: ns.config_count,
                type_: ns.type_,
            };
            Result::<NamespaceResponse>::http_success(response)
        }
        Err(_) => Result::<NamespaceResponse>::http_response(
            404,
            404,
            format!("Namespace not found: {}", params.namespace_id),
            NamespaceResponse {
                namespace: String::new(),
                namespace_show_name: String::new(),
                namespace_desc: String::new(),
                quota: 0,
                config_count: 0,
                type_: 0,
            },
        ),
    }
}

/// Create a namespace
///
/// POST /nacos/v2/console/namespace
///
/// Creates a new namespace.
#[post("")]
pub async fn create_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<NamespaceCreateParam>,
) -> impl Responder {
    // Check authorization
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Get or generate namespace ID
    let mut namespace_id = form
        .get_namespace_id()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if namespace_id.is_empty() {
        namespace_id = uuid::Uuid::new_v4().to_string();
    }

    let namespace_name = form.namespace_name.trim().to_string();
    let namespace_desc = form.namespace_desc.clone().unwrap_or_default();

    // Validate namespace ID format
    if !NAMESPACE_ID_REGEX.is_match(&namespace_id) {
        return Result::<bool>::http_response(
            400,
            error::ILLEGAL_NAMESPACE.code,
            error::ILLEGAL_NAMESPACE.message.to_string(),
            false,
        );
    }

    // Validate namespace ID length
    if namespace_id.chars().count() > NAMESPACE_ID_MAX_LENGTH {
        return Result::<bool>::http_response(
            400,
            error::ILLEGAL_NAMESPACE.code,
            format!(
                "Namespace ID too long, max {} characters",
                NAMESPACE_ID_MAX_LENGTH
            ),
            false,
        );
    }

    // Validate namespace name
    if namespace_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'namespaceName' is missing".to_string(),
            false,
        );
    }

    if !NAMESPACE_NAME_REGEX.is_match(&namespace_name) {
        return Result::<bool>::http_response(
            400,
            error::ILLEGAL_NAMESPACE.code,
            format!(
                "Namespace name '{}' contains illegal characters",
                namespace_name
            ),
            false,
        );
    }

    // Create namespace
    match service::namespace::create(data.db(), &namespace_id, &namespace_name, &namespace_desc)
        .await
    {
        Ok(_) => {
            tracing::info!(
                namespace_id = %namespace_id,
                namespace_name = %namespace_name,
                "Namespace created"
            );
            Result::<bool>::http_success(true)
        }
        Err(e) => {
            tracing::error!(
                namespace_id = %namespace_id,
                error = %e,
                "Failed to create namespace"
            );
            Result::<bool>::http_response(
                500,
                500,
                format!("Failed to create namespace: {}", e),
                false,
            )
        }
    }
}

/// Update a namespace
///
/// PUT /nacos/v2/console/namespace
///
/// Updates an existing namespace.
#[put("")]
pub async fn update_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<NamespaceUpdateParam>,
) -> impl Responder {
    // Check authorization
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Validate required fields
    if form.namespace_id.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'namespaceId' is missing".to_string(),
            false,
        );
    }

    if form.namespace_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'namespaceName' is missing".to_string(),
            false,
        );
    }

    // Validate namespace name
    if !NAMESPACE_NAME_REGEX.is_match(&form.namespace_name) {
        return Result::<bool>::http_response(
            400,
            error::ILLEGAL_NAMESPACE.code,
            format!(
                "Namespace name '{}' contains illegal characters",
                form.namespace_name
            ),
            false,
        );
    }

    let namespace_desc = form.namespace_desc.clone().unwrap_or_default();

    // Update namespace
    match service::namespace::update(
        data.db(),
        &form.namespace_id,
        &form.namespace_name,
        &namespace_desc,
    )
    .await
    {
        Ok(updated) => {
            if updated {
                tracing::info!(
                    namespace_id = %form.namespace_id,
                    namespace_name = %form.namespace_name,
                    "Namespace updated"
                );
            }
            Result::<bool>::http_success(updated)
        }
        Err(e) => {
            tracing::error!(
                namespace_id = %form.namespace_id,
                error = %e,
                "Failed to update namespace"
            );
            Result::<bool>::http_response(
                500,
                500,
                format!("Failed to update namespace: {}", e),
                false,
            )
        }
    }
}

/// Delete a namespace
///
/// DELETE /nacos/v2/console/namespace
///
/// Deletes a namespace.
#[delete("")]
pub async fn delete_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<NamespaceDeleteParam>,
) -> impl Responder {
    // Check authorization
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::OpenApi)
            .build()
    );

    if params.namespace_id.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'namespaceId' is missing".to_string(),
            false,
        );
    }

    // Prevent deletion of public namespace
    if params.namespace_id == "public" || params.namespace_id.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Cannot delete the public namespace".to_string(),
            false,
        );
    }

    // Delete namespace
    match service::namespace::delete(data.db(), &params.namespace_id).await {
        Ok(deleted) => {
            if deleted {
                tracing::info!(
                    namespace_id = %params.namespace_id,
                    "Namespace deleted"
                );
            }
            Result::<bool>::http_success(deleted)
        }
        Err(e) => {
            tracing::error!(
                namespace_id = %params.namespace_id,
                error = %e,
                "Failed to delete namespace"
            );
            Result::<bool>::http_response(
                500,
                500,
                format!("Failed to delete namespace: {}", e),
                false,
            )
        }
    }
}
