//! AI Resource Importer (F-NAC-ADM-AI-008)
//!
//! Generic AI resource import endpoints aligned with Nacos 3.3.0 AI Importer.
//! Supports searching, validating, and importing AI resources (MCP servers, skills)
//! from external sources such as MCP Registry and skills.sh.
//!
//! Path prefix: `/v3/admin/ai/import`
//! Endpoints:
//!   GET  /sources  - list available import sources
//!   POST /search   - search external candidate resources
//!   POST /validate - validate selected items
//!   POST /execute  - execute import

use std::collections::HashMap;

use actix_web::{HttpRequest, Responder, Scope, get, post, web};
use serde::{Deserialize, Serialize};

use batata_server_common::error;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response as common_response;
use batata_server_common::secured::Secured;
use batata_server_common::{ActionTypes, ApiType, SignType, secured};

// ============================================================================
// Data models
// ============================================================================

/// Information about an available import source.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiResourceImportSourceInfo {
    source_id: String,
    display_name: String,
    description: String,
    plugin_name: String,
    resource_types: Vec<String>,
    enabled: bool,
    capabilities: Vec<String>,
}

/// Search request (form-encoded, Nacos SDK compatible).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Fields deserialized from HTTP form, may not all be read in Rust
struct AiResourceImportSearchRequest {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "resourceType")]
    resource_type: Option<String>,
    #[serde(default, alias = "sourceId")]
    source_id: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    options: Option<String>,
}

/// Search response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiResourceImportSearchResponse {
    source_id: String,
    resource_type: String,
    next_cursor: Option<String>,
    has_more: bool,
    items: Vec<AiResourceImportCandidateItem>,
}

/// A candidate resource item returned by search.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiResourceImportCandidateItem {
    external_id: String,
    name: String,
    version: String,
    description: String,
    metadata: HashMap<String, String>,
}

/// A single item to validate or import (deserialized from JSON within `selectedItems`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Fields deserialized from JSON, may not all be read in Rust
struct AiResourceImportItem {
    external_id: String,
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    metadata: HashMap<String, String>,
}

/// Validate request (form-encoded, Nacos SDK compatible).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Fields deserialized from HTTP form, may not all be read in Rust
struct AiResourceImportValidateRequest {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "resourceType")]
    resource_type: Option<String>,
    #[serde(default, alias = "sourceId")]
    source_id: Option<String>,
    #[serde(default, alias = "selectedItems")]
    selected_items: String,
    #[serde(default, alias = "overwriteExisting")]
    overwrite_existing: bool,
    #[serde(default)]
    options: Option<String>,
}

/// Validate response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiResourceImportValidateResponse {
    source_id: String,
    resource_type: String,
    validation_token: String,
    items: Vec<AiResourceImportValidationItem>,
}

/// Validation result for a single item.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiResourceImportValidationItem {
    external_id: String,
    name: String,
    version: String,
    status: String,
    exists: bool,
    conflict_type: Option<String>,
    warnings: Vec<String>,
    errors: Vec<String>,
}

/// Execute request (form-encoded, Nacos SDK compatible).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Fields deserialized from HTTP form, may not all be read in Rust
struct AiResourceImportExecuteRequest {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "resourceType")]
    resource_type: Option<String>,
    #[serde(default, alias = "sourceId")]
    source_id: Option<String>,
    #[serde(default, alias = "selectedItems")]
    selected_items: String,
    #[serde(default, alias = "validationToken")]
    validation_token: Option<String>,
    #[serde(default, alias = "overwriteExisting")]
    overwrite_existing: bool,
    #[serde(default)]
    options: Option<String>,
}

/// Execute response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiResourceImportExecuteResponse {
    success: bool,
    total_count: u32,
    success_count: u32,
    failed_count: u32,
    skipped_count: u32,
    results: Vec<AiResourceImportResultItem>,
}

/// Result of importing a single item.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiResourceImportResultItem {
    external_id: String,
    resource_name: String,
    version: String,
    status: String,
    error_message: String,
    warnings: Vec<String>,
}

// ============================================================================
// Built-in import sources
// ============================================================================

/// Return the list of built-in import sources.
fn builtin_sources() -> Vec<AiResourceImportSourceInfo> {
    vec![
        AiResourceImportSourceInfo {
            source_id: "mcp-official".to_string(),
            display_name: "MCP Registry (Official)".to_string(),
            description: "Import MCP servers from the official MCP Registry."
                .to_string(),
            plugin_name: "mcp-registry".to_string(),
            resource_types: vec!["mcp".to_string()],
            enabled: true,
            capabilities: vec![
                "search".to_string(),
                "validate".to_string(),
                "execute".to_string(),
            ],
        },
        AiResourceImportSourceInfo {
            source_id: "skills-sh".to_string(),
            display_name: "skills.sh".to_string(),
            description: "Import AI skills from skills.sh community registry."
                .to_string(),
            plugin_name: "skills-sh".to_string(),
            resource_types: vec!["skill".to_string()],
            enabled: true,
            capabilities: vec![
                "search".to_string(),
                "validate".to_string(),
                "execute".to_string(),
            ],
        },
        AiResourceImportSourceInfo {
            source_id: "skills-well-known".to_string(),
            display_name: "Well-known Skills".to_string(),
            description: "Import AI skills from well-known URLs.".to_string(),
            plugin_name: "skill-well-known".to_string(),
            resource_types: vec!["skill".to_string()],
            enabled: true,
            capabilities: vec![
                "search".to_string(),
                "validate".to_string(),
                "execute".to_string(),
            ],
        },
    ]
}

/// Parse the `selectedItems` JSON string into a vec of items.
fn parse_selected_items(raw: &str) -> Result<Vec<AiResourceImportItem>, String> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str::<Vec<AiResourceImportItem>>(raw)
        .map_err(|e| format!("Invalid selectedItems JSON: {}", e))
}

// ============================================================================
// Endpoints
// ============================================================================

/// GET /sources — list available import sources.
///
/// Optional query parameter `resourceType` filters sources by supported type.
#[get("/sources")]
async fn list_sources(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<HashMap<String, String>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "ai/import")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let mut sources = builtin_sources();

    // Optional resourceType filter
    if let Some(rt) = params.get("resourceType") {
        sources.retain(|s| s.resource_types.iter().any(|t| t == rt));
    }

    common_response::Result::<Vec<AiResourceImportSourceInfo>>::http_success(sources)
}

/// POST /search — search external candidate resources.
///
/// Returns an empty result set in this simplified implementation. The full
/// request/response models are retained for future integration with real
/// external source APIs.
#[post("/search")]
async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    body: web::Form<AiResourceImportSearchRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "ai/import")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = body.into_inner();
    let source_id = q.source_id.unwrap_or_else(|| "mcp-official".to_string());
    let resource_type = q.resource_type.unwrap_or_else(|| "mcp".to_string());

    // No external source API client is wired yet; return empty results.
    let response = AiResourceImportSearchResponse {
        source_id,
        resource_type,
        next_cursor: None,
        has_more: false,
        items: Vec::new(),
    };

    common_response::Result::<AiResourceImportSearchResponse>::http_success(response)
}

/// POST /validate — validate selected items.
///
/// Parses `selectedItems` (JSON array) and returns a VALID status for each
/// item. In a future implementation this would check for naming conflicts,
/// schema validity, and source availability.
#[post("/validate")]
async fn validate(
    req: HttpRequest,
    data: web::Data<AppState>,
    body: web::Form<AiResourceImportValidateRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "ai/import")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = body.into_inner();
    let source_id = q.source_id.unwrap_or_else(|| "mcp-official".to_string());
    let resource_type = q.resource_type.unwrap_or_else(|| "mcp".to_string());

    let items = match parse_selected_items(&q.selected_items) {
        Ok(items) => items,
        Err(e) => {
            return common_response::Result::<String>::http_response(
                400,
                error::PARAMETER_VALIDATE_ERROR.code,
                e,
                String::new(),
            );
        }
    };

    let validation_items: Vec<AiResourceImportValidationItem> = items
        .iter()
        .map(|item| AiResourceImportValidationItem {
            external_id: item.external_id.clone(),
            name: item.name.clone(),
            version: item.version.clone(),
            status: "VALID".to_string(),
            exists: false,
            conflict_type: None,
            warnings: Vec::new(),
            errors: Vec::new(),
        })
        .collect();

    // Generate a simple validation token (timestamp-based, not cryptographically secure).
    let validation_token = format!(
        "vt-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        validation_items.len()
    );

    let response = AiResourceImportValidateResponse {
        source_id,
        resource_type,
        validation_token,
        items: validation_items,
    };

    common_response::Result::<AiResourceImportValidateResponse>::http_success(response)
}

/// POST /execute — execute the import.
///
/// Parses `selectedItems` (JSON array) and returns a SUCCESS status for each
/// item. In a future implementation this would create the actual resources
/// (MCP server registrations, skill definitions, etc.).
#[post("/execute")]
async fn execute(
    req: HttpRequest,
    data: web::Data<AppState>,
    body: web::Form<AiResourceImportExecuteRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "ai/import")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = body.into_inner();

    let items = match parse_selected_items(&q.selected_items) {
        Ok(items) => items,
        Err(e) => {
            return common_response::Result::<String>::http_response(
                400,
                error::PARAMETER_VALIDATE_ERROR.code,
                e,
                String::new(),
            );
        }
    };

    let total = items.len() as u32;
    let results: Vec<AiResourceImportResultItem> = items
        .iter()
        .map(|item| AiResourceImportResultItem {
            external_id: item.external_id.clone(),
            resource_name: item.name.clone(),
            version: item.version.clone(),
            status: "SUCCESS".to_string(),
            error_message: String::new(),
            warnings: Vec::new(),
        })
        .collect();

    let response = AiResourceImportExecuteResponse {
        success: true,
        total_count: total,
        success_count: total,
        failed_count: 0,
        skipped_count: 0,
        results,
    };

    common_response::Result::<AiResourceImportExecuteResponse>::http_success(response)
}

/// Register AI import routes under a `/import` scope.
///
/// When mounted under `/v3/admin/ai`, the full paths become:
///   - GET  /v3/admin/ai/import/sources
///   - POST /v3/admin/ai/import/search
///   - POST /v3/admin/ai/import/validate
///   - POST /v3/admin/ai/import/execute
pub fn routes() -> Scope {
    web::scope("/import")
        .service(list_sources)
        .service(search)
        .service(validate)
        .service(execute)
}
