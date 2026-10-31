//! AI Resource Importer (F-NAC-ADM-AI-008)
//!
//! Generic AI resource import endpoints aligned with Nacos 3.3.0
//! `AiResourceImportAdminController` / `ConsoleAiResourceImportController` and
//! the `AiResourceImportManager` behind them.
//!
//! Paths (both are served from the same handlers, only the auth `ApiType`
//! differs, exactly like upstream):
//!   GET  /v3/admin/ai/import/sources    GET  /v3/console/ai/import/sources
//!   POST /v3/admin/ai/import/search     POST /v3/console/ai/import/search
//!   POST /v3/admin/ai/import/validate   POST /v3/console/ai/import/validate
//!   POST /v3/admin/ai/import/execute    POST /v3/console/ai/import/execute
//!
//! Two upstream facts shape the whole module:
//! - `resourceType` and `sourceId` are **mandatory** on search/validate/execute
//!   and `selectedItems` is mandatory on validate/execute. Missing ones fail
//!   with `PARAMETER_MISSING`; unparseable JSON ones fail with
//!   `PARAMETER_VALIDATE_ERROR`. An empty `selectedItems` list is rejected too.
//! - Nothing here fabricates a success. Every item is fetched from its source
//!   and only then marked `VALID`/`SUCCESS`; a fetch or import failure yields
//!   `INVALID`, or `FAILED`/`SKIPPED` (with `skipInvalid`) on execute.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use actix_web::{HttpRequest, HttpResponse, Scope, web};
use serde::{Deserialize, Serialize};

use batata_common::McpServerService;
use batata_common::model::ai::mcp::{McpServerRegistration, McpServerType, McpTransport};
use batata_server_common::error;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response as common_response;
use batata_server_common::secured::Secured;
use batata_server_common::{ActionTypes, ApiType, SignType, secured};

/// Resource type constant shared with the importer plugins (`mcp`).
const RESOURCE_TYPE_MCP: &str = "mcp";
/// Resource type constant shared with the importer plugins (`skill`).
const RESOURCE_TYPE_SKILL: &str = "skill";

/// Upstream `AiResourceImportConstants.DEFAULT_MAX_ITEM_COUNT`.
///
/// Also the ceiling applied to a caller-supplied `limit`.
const DEFAULT_MAX_ITEM_COUNT: i32 = 500;

/// Namespace used when the request leaves `namespaceId` blank.
const DEFAULT_NAMESPACE_ID: &str = "public";

/// Upstream `McpRegistryImportService.DEFAULT_FETCH_LIMIT`, used when the
/// request carries no limit (validate and execute never do).
const DEFAULT_FETCH_LIMIT: i32 = 30;

/// Upstream `McpRegistryClient.READ_TIMEOUT_SECONDS`.
const READ_TIMEOUT: Duration = Duration::from_secs(20);

/// Extra guard upstream does not have: batata pins the connect phase so an
/// unreachable registry cannot stall a request for the OS default.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

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
#[allow(dead_code)] // Carried on the wire by the SDK; only the source call consumes it.
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
    limit: Option<i32>,
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
#[allow(dead_code)] // `metadata` travels with the selection; the MCP registry fetch is keyed by id.
struct AiResourceImportItem {
    external_id: String,
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    metadata: HashMap<String, String>,
}

/// Validate request (form-encoded, Nacos SDK compatible).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiResourceImportValidateRequest {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "resourceType")]
    resource_type: Option<String>,
    #[serde(default, alias = "sourceId")]
    source_id: Option<String>,
    #[serde(default, alias = "selectedItems")]
    selected_items: Option<String>,
    #[serde(default, alias = "overwriteExisting")]
    overwrite_existing: bool,
    #[serde(default)]
    options: Option<String>,
}

/// Validate response.
///
/// `validation_token` is deliberately left unset: upstream's
/// `AiResourceImportManager#validate` never populates it and
/// `#execute` never reads it back, so a token here would be a lie about
/// server-side state the caller could rely on.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiResourceImportValidateResponse {
    source_id: String,
    resource_type: String,
    validation_token: Option<String>,
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
struct AiResourceImportExecuteRequest {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "resourceType")]
    resource_type: Option<String>,
    #[serde(default, alias = "sourceId")]
    source_id: Option<String>,
    #[serde(default, alias = "selectedItems")]
    selected_items: Option<String>,
    #[serde(default, alias = "validationToken")]
    validation_token: Option<String>,
    #[serde(default, alias = "overwriteExisting")]
    overwrite_existing: bool,
    #[serde(default, alias = "skipInvalid")]
    skip_invalid: bool,
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
// Errors
// ============================================================================

/// One upstream failure, already resolved to an HTTP status and v3 error code.
struct ImportError {
    status: u16,
    code: i32,
    message: String,
}

impl ImportError {
    /// `AbstractAiResourceImportForm#missingParameter`.
    fn missing(name: &str) -> Self {
        Self {
            status: 400,
            code: error::PARAMETER_MISSING.code,
            message: format!("Required parameter `{}` is not present.", name),
        }
    }

    /// `AbstractAiResourceImportForm#parseFailed`.
    fn unparsable(name: &str) -> Self {
        Self {
            status: 400,
            code: error::PARAMETER_VALIDATE_ERROR.code,
            message: format!("Request parameter `{}` can't be parsed.", name),
        }
    }

    /// `AiResourceImportPluginManager#resolveBuilder`, unknown plugin branch.
    fn plugin_not_found(source_id: &str) -> Self {
        Self {
            status: 404,
            code: error::RESOURCE_NOT_FOUND.code,
            message: format!("AI resource import plugin not found: {}", source_id),
        }
    }

    /// `AiResourceImportPluginManager#resolveBuilder`, disabled plugin branch.
    fn plugin_disabled(source_id: &str) -> Self {
        Self {
            status: 400,
            code: error::PARAMETER_VALIDATE_ERROR.code,
            message: format!("AI resource import plugin is disabled: {}", source_id),
        }
    }

    /// `AiResourceImportPluginManager#resolveBuilder`, type mismatch branch.
    fn unsupported_resource_type(resource_type: &str) -> Self {
        Self {
            status: 400,
            code: error::PARAMETER_VALIDATE_ERROR.code,
            message: format!(
                "AI resource import plugin does not support resource type: {}",
                resource_type
            ),
        }
    }

    /// `AbstractAiResourceImportServiceBuilder#validateEndpoint`, blank branch.
    fn endpoint_missing(source_id: &str) -> Self {
        Self {
            status: 400,
            code: error::PARAMETER_VALIDATE_ERROR.code,
            message: format!(
                "AI resource import plugin endpoint is missing: {}",
                source_id
            ),
        }
    }

    /// `AbstractAiResourceImportServiceBuilder#validateEndpoint`, URI branch.
    fn endpoint_invalid(source_id: &str) -> Self {
        Self {
            status: 400,
            code: error::PARAMETER_VALIDATE_ERROR.code,
            message: format!(
                "AI resource import plugin endpoint is invalid: {}",
                source_id
            ),
        }
    }

    /// `AiResourceImportManager#requireSelectedItems`.
    fn empty_selected_items() -> Self {
        Self {
            status: 400,
            code: error::PARAMETER_VALIDATE_ERROR.code,
            message: "AI resource import selected items must not be empty.".to_string(),
        }
    }

    /// The whole module is switched off.
    fn module_disabled() -> Self {
        Self {
            status: 501,
            code: error::SERVER_ERROR.code,
            message: "AI resource import is disabled.".to_string(),
        }
    }
}

/// Turn an [`ImportError`] into the standard v3 error body.
fn error_response(err: ImportError) -> HttpResponse {
    common_response::Result::<String>::http_response(
        err.status,
        err.code,
        err.message,
        String::new(),
    )
}

// ============================================================================
// Built-in import sources
// ============================================================================

/// Where a source's endpoint comes from.
#[derive(Debug, Clone, Copy)]
enum EndpointOrigin {
    /// Hard-coded by the plugin (`mcp-official`, `skills-sh`).
    Fixed(&'static str),
    /// Must be configured before the plugin can be used.
    Configurable,
}

/// A built-in import source, mirroring one upstream `AiResourceImportServiceBuilder`.
struct ImportSourceDef {
    source_id: &'static str,
    importer_type: &'static str,
    display_name: &'static str,
    description: &'static str,
    resource_types: &'static [&'static str],
    endpoint: EndpointOrigin,
}

/// The built-in sources, in upstream SPI order.
///
/// `mcp-registry-protocol` and `skills-well-known` ship with a blank endpoint,
/// so — exactly like upstream before an operator configures them — they are
/// listed but unusable.
const SOURCES: &[ImportSourceDef] = &[
    ImportSourceDef {
        source_id: "mcp-registry-protocol",
        importer_type: "mcp-registry",
        display_name: "MCP Registry Protocol",
        description: "Import MCP servers from an MCP Registry protocol endpoint.",
        resource_types: &[RESOURCE_TYPE_MCP],
        endpoint: EndpointOrigin::Configurable,
    },
    ImportSourceDef {
        source_id: "mcp-official",
        importer_type: "mcp-registry",
        display_name: "Official MCP Registry",
        description: "Import MCP servers from the official MCP registry.",
        resource_types: &[RESOURCE_TYPE_MCP],
        endpoint: EndpointOrigin::Fixed("https://registry.modelcontextprotocol.io/v0/servers"),
    },
    ImportSourceDef {
        source_id: "skills-well-known",
        importer_type: "skills-well-known",
        display_name: "Skill Well-known Registry",
        description: "Import Skills from a well-known Skill discovery endpoint.",
        resource_types: &[RESOURCE_TYPE_SKILL],
        endpoint: EndpointOrigin::Configurable,
    },
    ImportSourceDef {
        source_id: "skills-sh",
        importer_type: "skills-sh",
        display_name: "skills.sh",
        description: "Import Skills from skills.sh.",
        resource_types: &[RESOURCE_TYPE_SKILL],
        endpoint: EndpointOrigin::Fixed("https://skills.sh"),
    },
];

/// Look up a built-in source by id.
fn find_source(source_id: &str) -> Option<&'static ImportSourceDef> {
    SOURCES.iter().find(|each| each.source_id == source_id)
}

/// True when the source handles `resource_type` (a blank type matches all).
fn supports_resource_type(source: &ImportSourceDef, resource_type: &str) -> bool {
    resource_type.is_empty() || source.resource_types.contains(&resource_type)
}

/// Render the sanitized view of a source.
///
/// The endpoint and the per-source secrets are deliberately absent, matching
/// `AiResourceImportPluginManager#toSourceInfo`.
fn to_source_info(source: &ImportSourceDef) -> AiResourceImportSourceInfo {
    AiResourceImportSourceInfo {
        source_id: source.source_id.to_string(),
        display_name: source.display_name.to_string(),
        description: source.description.to_string(),
        plugin_name: source.importer_type.to_string(),
        resource_types: source
            .resource_types
            .iter()
            .map(|each| each.to_string())
            .collect(),
        enabled: true,
        capabilities: vec![
            "search".to_string(),
            "validate".to_string(),
            "execute".to_string(),
        ],
    }
}

// ============================================================================
// Request-scoped source resolution
// ============================================================================

/// A source resolved for one request: who it is and where it fetches from.
struct ResolvedSource {
    def: &'static ImportSourceDef,
    endpoint: String,
}

/// Whether the import module is switched on.
fn module_enabled(app_state: &AppState) -> bool {
    app_state.configuration.typed.ai.import.enabled
}

/// Whether an individual source is switched on.
fn source_enabled(app_state: &AppState, source_id: &str) -> bool {
    app_state
        .configuration
        .typed
        .ai
        .import
        .sources
        .get(source_id)
        .map(|each| each.enabled)
        .unwrap_or(true)
}

/// Resolve `sourceId` + `resourceType` to a usable source.
///
/// The check order is upstream's: module switch, plugin lookup, plugin switch,
/// resource type, then endpoint validity.
fn resolve_source(
    app_state: &AppState,
    source_id: &str,
    resource_type: &str,
) -> Result<ResolvedSource, ImportError> {
    if !module_enabled(app_state) {
        return Err(ImportError::module_disabled());
    }
    let def = find_source(source_id).ok_or_else(|| ImportError::plugin_not_found(source_id))?;
    if !source_enabled(app_state, source_id) {
        return Err(ImportError::plugin_disabled(source_id));
    }
    if !supports_resource_type(def, resource_type) {
        return Err(ImportError::unsupported_resource_type(resource_type));
    }
    let endpoint = match def.endpoint {
        EndpointOrigin::Fixed(endpoint) => endpoint.to_string(),
        EndpointOrigin::Configurable => app_state
            .configuration
            .typed
            .ai
            .import
            .sources
            .get(source_id)
            .and_then(|each| each.endpoint())
            .map(str::to_string)
            .ok_or_else(|| ImportError::endpoint_missing(source_id))?,
    };
    // Upstream parses the endpoint as an absolute URI with a host; batata only
    // needs the same two rejections, so the scheme+authority check is done by
    // hand rather than pulling in a URI parser.
    if !endpoint.contains("://") || endpoint.split("://").nth(1).unwrap_or("").is_empty() {
        return Err(ImportError::endpoint_invalid(source_id));
    }
    Ok(ResolvedSource { def, endpoint })
}

/// Resolve the namespace, defaulting to `public` when blank.
fn resolve_namespace(namespace_id: Option<&String>) -> String {
    namespace_id
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_NAMESPACE_ID)
        .to_string()
}

/// Resolve a required, non-blank string parameter.
fn require(param: Option<&String>, name: &str) -> Result<String, ImportError> {
    param
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ImportError::missing(name))
}

/// Parse the `options` JSON string into a map.
fn parse_options(raw: Option<&String>) -> Result<HashMap<String, String>, ImportError> {
    match raw {
        None => Ok(HashMap::new()),
        Some(raw) => serde_json::from_str(raw).map_err(|_| ImportError::unparsable("options")),
    }
}

/// Parse the `selectedItems` JSON string.
fn parse_selected_items(raw: Option<&String>) -> Result<Vec<AiResourceImportItem>, ImportError> {
    match raw {
        None => Ok(Vec::new()),
        Some(raw) => {
            serde_json::from_str(raw).map_err(|_| ImportError::unparsable("selectedItems"))
        }
    }
}

/// Upstream `AiResourceImportManager#resolveLimit`.
fn resolve_limit(limit: Option<i32>) -> i32 {
    match limit {
        Some(limit) if limit > 0 => limit.min(DEFAULT_MAX_ITEM_COUNT),
        _ => DEFAULT_MAX_ITEM_COUNT,
    }
}

// ============================================================================
// MCP Registry protocol source
// ============================================================================

/// One entry of an MCP Registry `GET /v0/servers` page.
///
/// Only the fields batata adapts are modelled; the registry schema has more.
#[derive(Debug, Deserialize)]
struct McpRegistryEntry {
    #[serde(default)]
    server: Option<McpRegistryServer>,
    #[serde(default)]
    meta: Option<McpRegistryMeta>,
}

/// The `server` object of a registry entry.
#[derive(Debug, Deserialize)]
struct McpRegistryServer {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    repository: Option<McpRegistryRepository>,
    #[serde(default)]
    remotes: Vec<McpRegistryRemote>,
    #[serde(default)]
    packages: Vec<serde_json::Value>,
}

/// The `repository` object of a registry server.
#[derive(Debug, Deserialize)]
struct McpRegistryRepository {
    #[serde(default)]
    url: String,
}

/// One `remotes` entry of a registry server.
#[derive(Debug, Deserialize)]
struct McpRegistryRemote {
    #[serde(default)]
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    url: String,
}

/// The `meta` object of a registry entry.
#[derive(Debug, Deserialize)]
struct McpRegistryMeta {
    #[serde(default)]
    official: Option<McpRegistryOfficialMeta>,
}

/// The `meta.official` object of a registry entry.
#[derive(Debug, Deserialize)]
struct McpRegistryOfficialMeta {
    #[serde(default)]
    #[serde(rename = "publishedAt")]
    published_at: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    #[serde(rename = "isLatest")]
    is_latest: bool,
}

/// A `GET /v0/servers` page.
#[derive(Debug, Deserialize)]
struct McpRegistryPage {
    #[serde(default)]
    servers: Vec<McpRegistryEntry>,
    #[serde(default)]
    metadata: Option<McpRegistryPageMetadata>,
}

/// The `metadata` object of a registry page.
#[derive(Debug, Deserialize)]
struct McpRegistryPageMetadata {
    #[serde(default)]
    #[serde(rename = "nextCursor")]
    next_cursor: Option<String>,
}

/// A candidate plus the registry server it came from.
struct McpRegistryCandidate {
    entry: McpRegistryEntry,
}

/// Fetch one page from an MCP Registry protocol endpoint.
///
/// Upstream `McpRegistryClient#fetchUrlPage` sends `cursor`, `limit` and
/// `search` and requires a 2xx plus a parseable body; both failures surface as
/// a data-access error so the caller can turn them into per-item results.
async fn fetch_registry_page(
    endpoint: &str,
    cursor: Option<&str>,
    limit: i32,
    search: Option<&str>,
) -> Result<McpRegistryPage, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(|e| format!("Build MCP registry client failed: {}", e))?;

    let mut query: Vec<(&str, String)> = Vec::new();
    if let Some(cursor) = cursor.filter(|each| !each.is_empty()) {
        query.push(("cursor", cursor.to_string()));
    }
    query.push(("limit", limit.to_string()));
    if let Some(search) = search.filter(|each| !each.is_empty()) {
        query.push(("search", search.to_string()));
    }

    let response = client
        .get(endpoint)
        .query(&query)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| format!("Search MCP registry source failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "HTTP {} when fetching {}",
            status.as_u16(),
            endpoint
        ));
    }
    let body = response
        .text()
        .await
        .map_err(|e| format!("Read MCP registry response failed: {}", e))?;
    serde_json::from_str(&body).map_err(|_| "Failed to parse response body".to_string())
}

/// Map a registry entry to the candidate item returned to the caller.
fn to_candidate(entry: &McpRegistryEntry) -> AiResourceImportCandidateItem {
    let server = entry.server.as_ref();
    let name = server.map(|each| each.name.clone()).unwrap_or_default();
    let mut metadata = HashMap::new();
    if let Some(server) = server {
        if !server.status.is_empty() {
            metadata.insert("status".to_string(), server.status.clone());
        } else if let Some(status) = entry
            .meta
            .as_ref()
            .and_then(|meta| meta.official.as_ref())
            .map(|official| official.status.as_str())
            .filter(|status| !status.is_empty())
        {
            metadata.insert("status".to_string(), (*status).to_string());
        }
        if let Some(protocol) = registry_protocol(server) {
            metadata.insert("protocol".to_string(), protocol.to_string());
        }
        if let Some(url) = server.repository.as_ref().map(|each| each.url.as_str()) {
            if !url.is_empty() {
                metadata.insert("repository".to_string(), url.to_string());
            }
        }
    }
    AiResourceImportCandidateItem {
        external_id: name.clone(),
        name,
        version: server.map(|each| each.version.clone()).unwrap_or_default(),
        description: server
            .map(|each| each.description.clone())
            .unwrap_or_default(),
        metadata,
    }
}

/// Upstream `McpRegistryClient#resolveServerProtocol`: packages mean stdio,
/// otherwise the first remote's transport decides.
fn registry_protocol(server: &McpRegistryServer) -> Option<&'static str> {
    if !server.packages.is_empty() {
        return Some("stdio");
    }
    server
        .remotes
        .first()
        .and_then(|remote| match remote.kind.trim().to_lowercase().as_str() {
            "sse" => Some("sse"),
            "streamable-http" | "streamable" => Some("streamable-http"),
            _ => None,
        })
}

/// Fetch one MCP server by external id.
///
/// Upstream searches the registry with the id as the query and then matches on
/// `name` or `id`; a miss is an error the item pipeline reports.
async fn fetch_mcp_artifact(
    endpoint: &str,
    external_id: &str,
    limit: i32,
) -> Result<McpRegistryCandidate, String> {
    let page = fetch_registry_page(endpoint, None, limit, Some(external_id)).await?;
    let matched = page
        .servers
        .into_iter()
        .find(|entry| matches!(entry.server.as_ref(), Some(server) if server.name == external_id))
        .ok_or_else(|| format!("MCP server not found in registry: {}", external_id))?;
    Ok(McpRegistryCandidate { entry: matched })
}

/// Adapt a registry entry into the registration batata stores.
fn to_registration(
    namespace: &str,
    entry: &McpRegistryEntry,
) -> Result<McpServerRegistration, String> {
    let server = entry
        .server
        .as_ref()
        .ok_or("MCP registry entry has no server")?;
    let protocol = registry_protocol(server);
    let remote_url = server
        .remotes
        .first()
        .map(|remote| remote.url.as_str())
        .unwrap_or("");
    let server_type = match protocol {
        Some("stdio") => McpServerType::Stdio,
        Some("sse") => McpServerType::Sse,
        _ => McpServerType::Http,
    };
    let mut registration = McpServerRegistration {
        name: server.name.clone(),
        display_name: server.name.clone(),
        description: server.description.clone(),
        namespace: namespace.to_string(),
        version: server.version.clone(),
        endpoint: remote_url.to_string(),
        server_type,
        transport: McpTransport {
            transport_type: protocol.unwrap_or("http").to_string(),
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            url: if remote_url.is_empty() {
                None
            } else {
                Some(remote_url.to_string())
            },
            headers: HashMap::new(),
            timeout_ms: 30000,
        },
        capabilities: Vec::new(),
        tools: Vec::new(),
        resources: Vec::new(),
        prompts: Vec::new(),
        metadata: HashMap::new(),
        tags: Vec::new(),
        auto_fetch_tools: false,
        health_check: None,
    };
    if let Some(official) = entry.meta.as_ref().and_then(|meta| meta.official.as_ref()) {
        if !official.published_at.is_empty() {
            registration
                .metadata
                .insert("publishedAt".to_string(), official.published_at.clone());
        }
        if official.is_latest {
            registration
                .metadata
                .insert("isLatest".to_string(), "true".to_string());
        }
    }
    Ok(registration)
}

// ============================================================================
// Item pipeline
// ============================================================================

/// The external id an item is fetched by.
///
/// Upstream `McpRegistryImportService#resolveExternalId` falls back to `name`
/// and then rejects the item.
fn external_id(item: &AiResourceImportItem) -> Result<String, String> {
    let id = if !item.external_id.trim().is_empty() {
        item.external_id.trim()
    } else {
        item.name.trim()
    };
    if id.is_empty() {
        return Err("MCP registry import item external id must not be empty.".to_string());
    }
    Ok(id.to_string())
}

/// A fetched artifact, ready to be validated or imported.
struct Artifact {
    external_id: String,
    name: String,
    version: String,
    registration: Option<McpServerRegistration>,
}

/// Fetch one item from its source.
async fn fetch_artifact(
    source: &ResolvedSource,
    namespace: &str,
    limit: i32,
    item: &AiResourceImportItem,
) -> Result<Artifact, String> {
    let id = external_id(item)?;
    // Upstream `McpRegistryImportService#resolveFetchLimit` defaults to 30 when
    // the context carries no usable limit.
    let fetch_limit = if limit > 0 { limit } else { 30 };
    match source.def.importer_type {
        "mcp-registry" => {
            let candidate = fetch_mcp_artifact(&source.endpoint, &id, fetch_limit).await?;
            let registration = to_registration(namespace, &candidate.entry)?;
            Ok(Artifact {
                external_id: id,
                name: registration.name.clone(),
                version: registration.version.clone(),
                registration: Some(registration),
            })
        }
        // Upstream ships `skills-sh` / `skills-well-known` fetchers; batata has
        // not ported them. Reporting the gap beats pretending the import worked.
        other => Err(format!(
            "AI resource import source `{}` is registered but its fetcher is not implemented yet.",
            other
        )),
    }
}

/// Validate one fetched artifact against what the server already holds.
async fn validate_artifact(
    svc: &Arc<dyn McpServerService>,
    namespace: &str,
    artifact: &Artifact,
) -> Result<AiResourceImportValidationItem, String> {
    let registration = artifact
        .registration
        .as_ref()
        .ok_or_else(|| "AI resource import artifact is empty.".to_string())?;
    if registration.name.trim().is_empty() {
        return Err("MCP import artifact name must not be empty.".to_string());
    }
    let version = if registration.version.is_empty() {
        None
    } else {
        Some(registration.version.as_str())
    };
    // A lookup failure means "unknown", not "conflict": the real outcome shows
    // up when the import is attempted.
    let exists = matches!(
        svc.get_mcp_server_detail(namespace, None, Some(&registration.name), version, None)
            .await,
        Ok(Some(_))
    );
    let name = artifact.name.clone();
    if exists {
        return Ok(AiResourceImportValidationItem {
            external_id: artifact.external_id.clone(),
            name,
            version: artifact.version.clone(),
            status: "CONFLICT".to_string(),
            exists: true,
            conflict_type: Some("existing".to_string()),
            warnings: Vec::new(),
            errors: Vec::new(),
        });
    }
    Ok(AiResourceImportValidationItem {
        external_id: artifact.external_id.clone(),
        name,
        version: artifact.version.clone(),
        status: "VALID".to_string(),
        exists: false,
        conflict_type: None,
        warnings: Vec::new(),
        errors: Vec::new(),
    })
}

/// Import one fetched artifact.
async fn import_artifact(
    svc: &Arc<dyn McpServerService>,
    namespace: &str,
    artifact: &Artifact,
    overwrite_existing: bool,
) -> Result<String, String> {
    let registration = artifact
        .registration
        .as_ref()
        .ok_or_else(|| "AI resource import artifact is empty.".to_string())?;
    let version = if registration.version.is_empty() {
        None
    } else {
        Some(registration.version.as_str())
    };
    let exists = matches!(
        svc.get_mcp_server_detail(namespace, None, Some(&registration.name), version, None)
            .await,
        Ok(Some(_))
    );
    if exists && !overwrite_existing {
        return Err(format!(
            "MCP server `{}` already exists and overwriteExisting is false.",
            registration.name
        ));
    }
    if exists {
        svc.update_mcp_server(namespace, registration)
            .await
            .map_err(|e| format!("Update MCP server failed: {}", e))?;
    } else {
        svc.create_mcp_server(namespace, registration)
            .await
            .map_err(|e| format!("Create MCP server failed: {}", e))?;
    }
    Ok(registration.name.clone())
}

// ============================================================================
// Endpoints
// ============================================================================

/// GET /sources — list available import sources.
///
/// Optional query parameter `resourceType` filters sources by supported type.
async fn list_sources(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<HashMap<String, String>>,
    api_type: ApiType,
) -> HttpResponse {
    secured!(
        Secured::builder(&req, &data, "ai/import")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(api_type)
            .build()
    );

    if !module_enabled(&data) {
        return error_response(ImportError::module_disabled());
    }

    let resource_type = params.get("resourceType").map(String::as_str).unwrap_or("");
    let sources: Vec<AiResourceImportSourceInfo> = SOURCES
        .iter()
        .filter(|each| source_enabled(&data, each.source_id))
        .filter(|each| supports_resource_type(each, resource_type))
        .map(to_source_info)
        .collect();

    common_response::Result::<Vec<AiResourceImportSourceInfo>>::http_success(sources)
}

/// POST /search — search external candidate resources.
async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    body: web::Form<AiResourceImportSearchRequest>,
    api_type: ApiType,
) -> HttpResponse {
    secured!(
        Secured::builder(&req, &data, "ai/import")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(api_type)
            .build()
    );

    let q = body.into_inner();
    let resource_type = match require(q.resource_type.as_ref(), "resourceType") {
        Ok(value) => value,
        Err(err) => return error_response(err),
    };
    let source_id = match require(q.source_id.as_ref(), "sourceId") {
        Ok(value) => value,
        Err(err) => return error_response(err),
    };
    let options = match parse_options(q.options.as_ref()) {
        Ok(options) => options,
        Err(err) => return error_response(err),
    };
    let _ = options;

    let source = match resolve_source(&data, &source_id, &resource_type) {
        Ok(source) => source,
        Err(err) => return error_response(err),
    };

    let limit = resolve_limit(q.limit);
    let page = match source.def.importer_type {
        "mcp-registry" => {
            let query = q.query.as_deref();
            match fetch_registry_page(&source.endpoint, q.cursor.as_deref(), limit, query).await {
                Ok(page) => page,
                Err(message) => {
                    return error_response(ImportError {
                        status: 500,
                        code: error::DATA_ACCESS_ERROR.code,
                        message: format!("Search MCP registry source failed: {}", message),
                    });
                }
            }
        }
        other => {
            return error_response(ImportError {
                status: 501,
                code: error::SERVER_ERROR.code,
                message: format!(
                    "AI resource import source `{}` is registered but its searcher is not implemented yet.",
                    other
                ),
            });
        }
    };

    let next_cursor = page
        .metadata
        .as_ref()
        .and_then(|meta| meta.next_cursor.clone())
        .filter(|each| !each.is_empty());
    let has_more = next_cursor.is_some();
    let response = AiResourceImportSearchResponse {
        source_id: source.def.source_id.to_string(),
        resource_type,
        next_cursor,
        has_more,
        items: page.servers.iter().map(to_candidate).collect(),
    };

    common_response::Result::<AiResourceImportSearchResponse>::http_success(response)
}

/// POST /validate — validate selected items.
async fn validate(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    body: web::Form<AiResourceImportValidateRequest>,
    api_type: ApiType,
) -> HttpResponse {
    secured!(
        Secured::builder(&req, &data, "ai/import")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(api_type)
            .build()
    );

    let q = body.into_inner();
    let resource_type = match require(q.resource_type.as_ref(), "resourceType") {
        Ok(value) => value,
        Err(err) => return error_response(err),
    };
    let source_id = match require(q.source_id.as_ref(), "sourceId") {
        Ok(value) => value,
        Err(err) => return error_response(err),
    };
    if q.selected_items
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        return error_response(ImportError::missing("selectedItems"));
    }
    let items = match parse_selected_items(q.selected_items.as_ref()) {
        Ok(items) => items,
        Err(err) => return error_response(err),
    };
    let options = match parse_options(q.options.as_ref()) {
        Ok(options) => options,
        Err(err) => return error_response(err),
    };
    let _ = options;

    let source = match resolve_source(&data, &source_id, &resource_type) {
        Ok(source) => source,
        Err(err) => return error_response(err),
    };
    if items.is_empty() {
        return error_response(ImportError::empty_selected_items());
    }

    let namespace = resolve_namespace(q.namespace_id.as_ref());
    // Neither validate nor execute carries a `limit`, so upstream's
    // `resolveFetchLimit` falls back to its own 30-item default.
    let limit = DEFAULT_FETCH_LIMIT;
    let mut validation_items = Vec::with_capacity(items.len());
    for item in &items {
        let mut validation_item = match fetch_artifact(&source, &namespace, limit, item).await {
            Ok(artifact) => match validate_artifact(&svc, &namespace, &artifact).await {
                Ok(validation_item) => validation_item,
                Err(message) => invalid_item(&item, &message),
            },
            Err(message) => invalid_item(&item, &message),
        };
        // Upstream keeps `overwriteExisting` out of the per-item verdict; a
        // conflict is still a conflict, the caller decides at execute time.
        let _ = q.overwrite_existing;
        validation_item.warnings = Vec::new();
        validation_items.push(validation_item);
    }

    let response = AiResourceImportValidateResponse {
        source_id: source.def.source_id.to_string(),
        resource_type,
        validation_token: None,
        items: validation_items,
    };

    common_response::Result::<AiResourceImportValidateResponse>::http_success(response)
}

/// POST /execute — execute the import.
async fn execute(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    body: web::Form<AiResourceImportExecuteRequest>,
    api_type: ApiType,
) -> HttpResponse {
    secured!(
        Secured::builder(&req, &data, "ai/import")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(api_type)
            .build()
    );

    let q = body.into_inner();
    let resource_type = match require(q.resource_type.as_ref(), "resourceType") {
        Ok(value) => value,
        Err(err) => return error_response(err),
    };
    let source_id = match require(q.source_id.as_ref(), "sourceId") {
        Ok(value) => value,
        Err(err) => return error_response(err),
    };
    if q.selected_items
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        return error_response(ImportError::missing("selectedItems"));
    }
    let items = match parse_selected_items(q.selected_items.as_ref()) {
        Ok(items) => items,
        Err(err) => return error_response(err),
    };
    let options = match parse_options(q.options.as_ref()) {
        Ok(options) => options,
        Err(err) => return error_response(err),
    };
    let _ = options;
    // Upstream accepts `validationToken` on the wire but never checks it.
    let _ = q.validation_token.as_deref();

    let source = match resolve_source(&data, &source_id, &resource_type) {
        Ok(source) => source,
        Err(err) => return error_response(err),
    };
    if items.is_empty() {
        return error_response(ImportError::empty_selected_items());
    }

    let namespace = resolve_namespace(q.namespace_id.as_ref());
    // Neither validate nor execute carries a `limit`, so upstream's
    // `resolveFetchLimit` falls back to its own 30-item default.
    let limit = DEFAULT_FETCH_LIMIT;
    let mut results = Vec::with_capacity(items.len());
    for item in &items {
        let result = match fetch_artifact(&source, &namespace, limit, item).await {
            Ok(artifact) => {
                match import_artifact(&svc, &namespace, &artifact, q.overwrite_existing).await {
                    Ok(name) => AiResourceImportResultItem {
                        external_id: artifact.external_id.clone(),
                        resource_name: name,
                        version: artifact.version.clone(),
                        status: "SUCCESS".to_string(),
                        error_message: String::new(),
                        warnings: Vec::new(),
                    },
                    Err(message) => failed_item(&artifact, &message, q.skip_invalid),
                }
            }
            Err(message) => AiResourceImportResultItem {
                external_id: item.external_id.clone(),
                resource_name: item.name.clone(),
                version: item.version.clone(),
                status: if q.skip_invalid { "SKIPPED" } else { "FAILED" }.to_string(),
                error_message: message,
                warnings: Vec::new(),
            },
        };
        results.push(result);
    }

    let mut success_count = 0;
    let mut failed_count = 0;
    let mut skipped_count = 0;
    for each in &results {
        match each.status.as_str() {
            "SUCCESS" => success_count += 1,
            "SKIPPED" => skipped_count += 1,
            _ => failed_count += 1,
        }
    }
    let total_count = results.len() as u32;

    let response = AiResourceImportExecuteResponse {
        success: failed_count == 0,
        total_count,
        success_count,
        failed_count,
        skipped_count,
        results,
    };

    common_response::Result::<AiResourceImportExecuteResponse>::http_success(response)
}

/// Build the `INVALID` verdict used when an item cannot even be fetched.
fn invalid_item(item: &AiResourceImportItem, message: &str) -> AiResourceImportValidationItem {
    AiResourceImportValidationItem {
        external_id: item.external_id.clone(),
        name: item.name.clone(),
        version: item.version.clone(),
        status: "INVALID".to_string(),
        exists: false,
        conflict_type: None,
        warnings: Vec::new(),
        errors: vec![message.to_string()],
    }
}

/// Build the `FAILED`/`SKIPPED` verdict used when an import does not land.
fn failed_item(
    artifact: &Artifact,
    message: &str,
    skip_invalid: bool,
) -> AiResourceImportResultItem {
    AiResourceImportResultItem {
        external_id: artifact.external_id.clone(),
        resource_name: artifact.name.clone(),
        version: artifact.version.clone(),
        status: if skip_invalid { "SKIPPED" } else { "FAILED" }.to_string(),
        error_message: message.to_string(),
        warnings: Vec::new(),
    }
}

// ============================================================================
// Route registration
// ============================================================================

/// Register the import endpoints under a `/import` scope.
///
/// `api_type` is the only thing that separates the two mounts: upstream has
/// `AiResourceImportAdminController` (`ApiType.ADMIN_API`) and
/// `ConsoleAiResourceImportController` (`ApiType.CONSOLE_API`) over the same
/// four paths.
fn routes_with(api_type: ApiType) -> Scope {
    web::scope("/import")
        .route(
            "/sources",
            web::get().to(
                move |req: HttpRequest,
                      data: web::Data<AppState>,
                      params: web::Query<HashMap<String, String>>| {
                    list_sources(req, data, params, api_type)
                },
            ),
        )
        .route(
            "/search",
            web::post().to(
                move |req: HttpRequest,
                      data: web::Data<AppState>,
                      body: web::Form<AiResourceImportSearchRequest>| {
                    search(req, data, body, api_type)
                },
            ),
        )
        .route(
            "/validate",
            web::post().to(
                move |req: HttpRequest,
                      data: web::Data<AppState>,
                      svc: web::Data<Arc<dyn McpServerService>>,
                      body: web::Form<AiResourceImportValidateRequest>| {
                    validate(req, data, svc, body, api_type)
                },
            ),
        )
        .route(
            "/execute",
            web::post().to(
                move |req: HttpRequest,
                      data: web::Data<AppState>,
                      svc: web::Data<Arc<dyn McpServerService>>,
                      body: web::Form<AiResourceImportExecuteRequest>| {
                    execute(req, data, svc, body, api_type)
                },
            ),
        )
}

/// Admin mount: `/v3/admin/ai/import`.
pub fn admin_routes() -> Scope {
    routes_with(ApiType::AdminApi)
}

/// Console mount: `/v3/console/ai/import`.
pub fn console_routes() -> Scope {
    routes_with(ApiType::ConsoleApi)
}
