//! V2 Console Namespace API handlers
//!
//! Implements the Nacos V2 console namespace management API endpoints:
//! - GET /v2/console/namespace/list - List all namespaces
//! - GET /v2/console/namespace - Get namespace detail
//! - POST /v2/console/namespace - Create namespace
//! - PUT /v2/console/namespace - Update namespace
//! - DELETE /v2/console/namespace - Delete namespace

use std::sync::LazyLock;

use actix_web::{HttpRequest, Responder, delete, get, post, put, web};
use serde::{Deserialize, Serialize};

use crate::model::Namespace;

use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType, error,
    model::{AppState, response::Result},
    secured,
};

/// Regex for validating namespace IDs
static NAMESPACE_ID_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[\w-]+$").expect("Invalid regex pattern"));

/// Regex for validating namespace names (no special chars)
static NAMESPACE_NAME_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[^@#$%^&*]+$").expect("Invalid regex pattern"));

/// Maximum length for namespace ID
const NAMESPACE_ID_MAX_LENGTH: usize = 128;

/// Request parameters for getting namespace detail
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamespaceDetailParam {
    namespace_id: String,
}

/// Request parameters for creating a namespace
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamespaceCreateParam {
    /// Custom namespace ID (optional, will generate UUID if not provided)
    #[serde(default)]
    custom_namespace_id: Option<String>,
    /// Namespace ID (alternative to custom_namespace_id)
    #[serde(default)]
    namespace_id: Option<String>,
    /// Namespace name (required)
    namespace_name: String,
    /// Namespace description (optional)
    #[serde(default)]
    namespace_desc: Option<String>,
}

impl NamespaceCreateParam {
    /// Get the namespace ID, preferring custom_namespace_id over namespace_id
    fn get_namespace_id(&self) -> Option<&str> {
        self.custom_namespace_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| self.namespace_id.as_deref().filter(|s| !s.is_empty()))
    }
}

/// Request parameters for updating a namespace
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamespaceUpdateParam {
    namespace_id: String,
    namespace_name: String,
    #[serde(default)]
    namespace_desc: Option<String>,
}

/// Request parameters for deleting a namespace
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamespaceDeleteParam {
    namespace_id: String,
}

/// Response data for namespace (V2 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamespaceResponse {
    namespace: String,
    namespace_show_name: String,
    namespace_desc: String,
    quota: i32,
    config_count: i32,
    #[serde(rename = "type")]
    type_: i32,
}

impl From<Namespace> for NamespaceResponse {
    fn from(ns: Namespace) -> Self {
        Self {
            namespace: ns.namespace,
            namespace_show_name: ns.namespace_show_name,
            namespace_desc: ns.namespace_desc,
            quota: ns.quota,
            config_count: ns.config_count,
            type_: ns.type_,
        }
    }
}

/// List all namespaces
///
/// GET /v2/console/namespace/list
#[get("list")]
pub async fn get_namespace_list(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns_list = data.console_datasource.namespace_find_all().await;

    let response: Vec<NamespaceResponse> =
        ns_list.into_iter().map(NamespaceResponse::from).collect();

    Result::<Vec<NamespaceResponse>>::http_success(response)
}

/// Get namespace detail
///
/// GET /v2/console/namespace
#[get("")]
pub async fn get_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<NamespaceDetailParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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

    match data
        .console_datasource
        .namespace_get_by_id(&params.namespace_id, "1")
        .await
    {
        Ok(ns) => {
            let response = NamespaceResponse::from(ns);
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
/// POST /v2/console/namespace
#[post("")]
pub async fn create_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<NamespaceCreateParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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
    match data
        .console_datasource
        .namespace_create(&namespace_id, &namespace_name, &namespace_desc)
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
/// PUT /v2/console/namespace
#[put("")]
pub async fn update_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<NamespaceUpdateParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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
    match data
        .console_datasource
        .namespace_update(&form.namespace_id, &form.namespace_name, &namespace_desc)
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
/// DELETE /v2/console/namespace
#[delete("")]
pub async fn delete_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<NamespaceDeleteParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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
    match data
        .console_datasource
        .namespace_delete(&params.namespace_id)
        .await
    {
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
