//! Agent Client HTTP API handlers -- Nacos 3.x compatible (F-NAC-CLIENT-010/011)
//!
//! Client: `/v3/client/ai/agents` (5 endpoints)
//! - GET    /search             - Search visible Agent catalog entries
//! - GET    /                   - Discover one exact Agent version and its endpoints
//! - POST   /endpoints          - Register agent endpoints (replace batch)
//! - DELETE /endpoints          - Deregister agent endpoints
//! - PUT    /endpoints/heartbeat - Refresh client liveness

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use serde::{Deserialize, Serialize};

use batata_common::model::Page;
use batata_common::{ActionTypes, ApiType, DEFAULT_NAMESPACE_ID, SignType};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};

use crate::service::traits::A2aAgentService;
use crate::{AiEndpointService, EndpointInfo};

// ============================================================================
// Constants
// ============================================================================

/// HTTP header for client identity (Nacos: `ClientConstants.HTTP_CLIENT_ID_HEADER`)
const CLIENT_ID_HEADER: &str = "clientId";

/// HTTP header for request module (Nacos: `HttpHeaderConsts.REQUEST_MODULE`)
const REQUEST_MODULE_HEADER: &str = "requestModule";

/// Default heartbeat interval in milliseconds (5 seconds)
const DEFAULT_HEARTBEAT_INTERVAL_MILLIS: i64 = 5_000;

/// Default unhealthy timeout in milliseconds (15 seconds)
const DEFAULT_UNHEALTHY_TIMEOUT_MILLIS: i64 = 15_000;

/// Default expire timeout in milliseconds (30 seconds)
const DEFAULT_EXPIRE_TIMEOUT_MILLIS: i64 = 30_000;

// ============================================================================
// Helper functions
// ============================================================================

fn normalize_namespace(ns: &str) -> &str {
    if ns.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        ns
    }
}

fn get_header(req: &HttpRequest, name: &str) -> String {
    req.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

// ============================================================================
// Request Models
// ============================================================================

/// GET /search query parameters.
///
/// Nacos-compatible field names via `camelCase` serde rename.
/// Accepts `agentName` or `agentNameContains` (Nacos `AgentSearchForm`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentSearchQuery {
    #[serde(default)]
    namespace_id: Option<String>,

    /// Agent name filter (Nacos: `agentNameContains`)
    #[serde(default, alias = "agentNameContains")]
    agent_name: Option<String>,

    #[serde(default)]
    page_no: Option<u32>,

    #[serde(default)]
    page_size: Option<u32>,
}

/// GET / query parameters.
///
/// Nacos-compatible field names via `camelCase` serde rename.
/// Maps to Nacos `AgentDiscoveryForm`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentDiscoveryQuery {
    #[serde(default)]
    namespace_id: Option<String>,

    agent_name: String,

    #[serde(default)]
    version: Option<String>,
}

/// POST /endpoints JSON body.
///
/// Nacos-compatible field names via `camelCase` serde rename.
/// Accepts `version` or `runtimeVersion` (Nacos `AgentEndpointRegistrationForm`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentEndpointRegistration {
    #[serde(default)]
    namespace_id: Option<String>,

    agent_name: String,

    /// Agent version (Nacos: `runtimeVersion`)
    #[serde(alias = "runtimeVersion")]
    version: String,

    #[serde(default)]
    protocol: String,

    #[serde(default)]
    endpoints: Vec<EndpointEntryInput>,
}

/// DELETE /endpoints query parameters.
///
/// Nacos-compatible field names via `camelCase` serde rename.
/// Maps to Nacos `AgentEndpointDeregistrationForm`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentEndpointDeregistration {
    #[serde(default)]
    namespace_id: Option<String>,

    agent_name: String,

    #[serde(default)]
    protocol: String,
}

/// Endpoint entry in registration requests.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EndpointEntryInput {
    address: String,
    port: u16,
    /// Accepted from the API for forward-compatibility; not yet consumed by
    /// `AiEndpointService::create_agent_endpoint` which does not accept metadata.
    #[serde(default)]
    #[allow(dead_code)]
    metadata: HashMap<String, String>,
}

// ============================================================================
// Response Models
// ============================================================================

/// Endpoint entry in responses (Nacos `Endpoint` compatible).
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EndpointEntry {
    address: String,
    port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    healthy: Option<bool>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    metadata: HashMap<String, String>,
}

impl EndpointEntry {
    fn from_endpoint_info(info: &EndpointInfo) -> Self {
        Self {
            address: info.address.clone(),
            port: info.port,
            healthy: Some(info.healthy),
            metadata: info.metadata.clone(),
        }
    }
}

/// Agent catalog version entry (Nacos `AgentCatalogVersion` compatible).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentCatalogVersion {
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    release_date: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_latest: bool,
}

/// Search catalog entry for one remote Agent (Nacos `AgentCatalogEntry` compatible).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentCatalogEntry {
    agent_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    latest_version: String,
    versions: Vec<AgentCatalogVersion>,
}

/// Discovery result for one exact Agent version (Nacos `AgentDiscoveryResult` compatible).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentDiscoveryResult {
    namespace_id: String,
    agent_name: String,
    version: String,
    endpoints: Vec<EndpointEntry>,
}

/// HTTP publisher liveness info (Nacos `ClientLivenessInfo` compatible).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientLivenessInfo {
    heartbeat_interval_millis: i64,
    unhealthy_timeout_millis: i64,
    expire_timeout_millis: i64,
}

impl Default for ClientLivenessInfo {
    fn default() -> Self {
        Self {
            heartbeat_interval_millis: DEFAULT_HEARTBEAT_INTERVAL_MILLIS,
            unhealthy_timeout_millis: DEFAULT_UNHEALTHY_TIMEOUT_MILLIS,
            expire_timeout_millis: DEFAULT_EXPIRE_TIMEOUT_MILLIS,
        }
    }
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /v3/client/ai/agents/search -- Search visible Agent catalog entries.
///
/// Uses `A2aAgentService.list_agents` to search agents and converts the result
/// to `Page<AgentCatalogEntry>`.
#[get("search")]
async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    a2a_service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentSearchQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or(""));
    let page_no = query.page_no.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(100);

    // Determine search type: blur for partial match, accurate for exact
    let agent_name = query.agent_name.as_deref().filter(|n| !n.is_empty());
    let search_type = "blur";

    match a2a_service
        // No end-user identity on this internal client path.
        .list_agents(ns, agent_name, search_type, page_no, page_size, None)
        .await
    {
        Ok(page) => {
            // Convert Page<AgentCardVersionInfo> to Page<AgentCatalogEntry>
            let entries: Vec<AgentCatalogEntry> = page
                .page_items
                .iter()
                .map(|v| AgentCatalogEntry {
                    agent_name: v.name.clone(),
                    display_name: Some(v.name.clone()),
                    description: None,
                    latest_version: v.latest_published_version.clone(),
                    versions: v
                        .version_details
                        .iter()
                        .map(|vd| AgentCatalogVersion {
                            version: vd.version.clone(),
                            release_date: if vd.release_date.is_empty() {
                                None
                            } else {
                                Some(vd.release_date.clone())
                            },
                            is_latest: vd.is_latest,
                        })
                        .collect(),
                })
                .collect();

            let result_page = Page::new(
                page.total_count,
                page_no as u64,
                page_size as u64,
                entries,
            );

            HttpResponse::Ok().json(Result::success(result_page))
        }
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/client/ai/agents -- Discover one exact Agent version and its endpoints.
///
/// Uses `AiEndpointService.get_agent_endpoints` to retrieve registered endpoints
/// for the specified agent name and version.
#[get("")]
async fn discover(
    req: HttpRequest,
    data: web::Data<AppState>,
    endpoint_service: web::Data<Arc<AiEndpointService>>,
    agent_service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentDiscoveryQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or(""));
    let requested = match query.version.as_deref() {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "version is required",
            );
        }
    };

    // A range is resolved against what is actually published; an exact version
    // is used as given, so asking for an unpublished one reports "not found"
    // rather than quietly serving a different version's endpoints.
    let version = if crate::service::version_range::is_range(&requested) {
        let versions = match agent_service.list_versions(ns, &query.agent_name).await {
            Ok(versions) => versions,
            Err(e) => {
                return Result::<()>::http_bad_request(
                    &batata_common::error::PARAMETER_VALIDATE_ERROR,
                    e.to_string(),
                );
            }
        };
        let available: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
        match crate::service::version_range::select_version(&available, &requested) {
            Some(v) => v.to_string(),
            None => {
                return Result::<()>::http_not_found(
                    &batata_common::error::RESOURCE_NOT_FOUND,
                    format!(
                        "no published version of agent '{}' matches '{}'",
                        query.agent_name, requested
                    ),
                );
            }
        }
    } else {
        requested
    };

    let endpoints = endpoint_service.get_agent_endpoints(ns, &query.agent_name, &version);
    let endpoint_entries: Vec<EndpointEntry> = endpoints
        .iter()
        .map(EndpointEntry::from_endpoint_info)
        .collect();

    let result = AgentDiscoveryResult {
        namespace_id: ns.to_string(),
        agent_name: query.agent_name.clone(),
        version,
        endpoints: endpoint_entries,
    };

    HttpResponse::Ok().json(Result::success(result))
}

/// POST /v3/client/ai/agents/endpoints -- Replace one HTTP Publisher's complete
/// Agent Endpoint batch.
///
/// Registers each endpoint via `AiEndpointService.create_agent_endpoint`.
/// Returns `ClientLivenessInfo` with default heartbeat intervals.
#[post("endpoints")]
async fn register_endpoints(
    req: HttpRequest,
    data: web::Data<AppState>,
    endpoint_service: web::Data<Arc<AiEndpointService>>,
    body: web::Json<AgentEndpointRegistration>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(form.namespace_id.as_deref().unwrap_or(""));

    // Register each endpoint in the batch
    for endpoint in &form.endpoints {
        endpoint_service.create_agent_endpoint(
            ns,
            &form.agent_name,
            &form.version,
            &endpoint.address,
            endpoint.port,
        );
    }

    tracing::info!(
        namespace = %ns,
        agent_name = %form.agent_name,
        version = %form.version,
        protocol = %form.protocol,
        endpoint_count = form.endpoints.len(),
        "Agent endpoints registered via client API"
    );

    let info = ClientLivenessInfo::default();
    HttpResponse::Ok().json(Result::success(info))
}

/// DELETE /v3/client/ai/agents/endpoints -- Remove one HTTP Publisher's complete
/// Agent Endpoint publication.
///
/// Lists all versions for the agent via `A2aAgentService.list_versions`, then
/// removes all registered endpoints for each version via
/// `AiEndpointService.delete_agent_endpoint`.
#[delete("endpoints")]
async fn deregister_endpoints(
    req: HttpRequest,
    data: web::Data<AppState>,
    endpoint_service: web::Data<Arc<AiEndpointService>>,
    a2a_service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentEndpointDeregistration>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let form = query.into_inner();
    let ns = normalize_namespace(form.namespace_id.as_deref().unwrap_or(""));

    // List all versions for this agent, then delete endpoints for each version
    match a2a_service.list_versions(ns, &form.agent_name).await {
        Ok(versions) => {
            for vd in &versions {
                let endpoints =
                    endpoint_service.get_agent_endpoints(ns, &form.agent_name, &vd.version);
                for ep in &endpoints {
                    endpoint_service.delete_agent_endpoint(
                        ns,
                        &form.agent_name,
                        &vd.version,
                        &ep.address,
                        ep.port,
                    );
                }
            }

            tracing::info!(
                namespace = %ns,
                agent_name = %form.agent_name,
                protocol = %form.protocol,
                version_count = versions.len(),
                "Agent endpoints deregistered via client API"
            );

            HttpResponse::Ok().json(Result::success(()))
        }
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// PUT /v3/client/ai/agents/endpoints/heartbeat -- Refresh one HTTP Client and
/// all Agent Endpoint publications it owns.
///
/// Returns `ClientLivenessInfo` with default heartbeat intervals.
/// A full implementation would track client heartbeats and return actual
/// intervals based on the client's registration status.
#[put("endpoints/heartbeat")]
async fn heartbeat(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let client_id = get_header(&req, CLIENT_ID_HEADER);
    let request_module = get_header(&req, REQUEST_MODULE_HEADER);

    tracing::debug!(
        client_id = %client_id,
        request_module = %request_module,
        "Agent client heartbeat received"
    );

    let info = ClientLivenessInfo::default();
    HttpResponse::Ok().json(Result::success(info))
}

// ============================================================================
// Route configuration
// ============================================================================

/// Configure client agent routes at `/v3/client/ai/agents`.
///
/// Registers 5 endpoints matching Nacos `AgentClientController`:
/// - `GET    /search`             -> search
/// - `GET    /`                   -> discover
/// - `POST   /endpoints`          -> register_endpoints
/// - `DELETE /endpoints`          -> deregister_endpoints
/// - `PUT    /endpoints/heartbeat` -> heartbeat
pub fn agent_client_routes() -> actix_web::Scope {
    web::scope("/agents")
        .service(search)
        .service(discover)
        .service(register_endpoints)
        .service(deregister_endpoints)
        .service(heartbeat)
}
