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

    /// The constraint the batch advertises (Nacos: `versionRange`).
    #[serde(default)]
    version_range: String,

    #[serde(default)]
    protocol: String,

    #[serde(default)]
    endpoints: Vec<EndpointEntryInput>,
}

/// POST /endpoints as upstream's SDK sends it: a form whose `endpoints` field
/// is a JSON *string*.
///
/// Upstream `AgentEndpointRegistrationForm` carries `namespaceId`, `agentName`,
/// `runtimeVersion`, `versionRange`, `protocol` and `endpoints`; Batata's own
/// JSON body is accepted as well, so the content type decides.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentEndpointRegistrationForm {
    #[serde(default)]
    namespace_id: Option<String>,

    agent_name: String,

    /// The runtime version the batch is published under.
    #[serde(alias = "version")]
    runtime_version: String,

    /// The constraint the batch advertises, e.g. `1.x`.
    #[serde(default)]
    version_range: String,

    #[serde(default)]
    protocol: String,

    /// `Vec<EndpointEntryInput>` as JSON.
    #[serde(default)]
    endpoints: String,
}

impl AgentEndpointRegistrationForm {
    fn into_registration(self) -> std::result::Result<AgentEndpointRegistration, String> {
        let endpoints = if self.endpoints.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str::<Vec<EndpointEntryInput>>(self.endpoints.trim())
                .map_err(|e| format!("Invalid endpoints JSON: {e}"))?
        };
        Ok(AgentEndpointRegistration {
            namespace_id: self.namespace_id,
            agent_name: self.agent_name,
            version: self.runtime_version,
            version_range: self.version_range,
            protocol: self.protocol,
            endpoints,
        })
    }
}

/// Read a registration from either a form body or a JSON body.
///
/// Upstream posts the batch as a form with the endpoints as a JSON string; the
/// content type decides, with a JSON-looking body as the fallback for a client
/// that never sends one.
fn parse_endpoint_registration(
    content_type: &str,
    body: &[u8],
) -> std::result::Result<AgentEndpointRegistration, String> {
    let body = body.trim_ascii();
    if body.is_empty() {
        return Err("an endpoint registration is required".to_string());
    }
    if content_type.contains("json") || body.first() == Some(&b'{') {
        return serde_json::from_slice::<AgentEndpointRegistration>(body)
            .map_err(|e| format!("Invalid endpoint registration: {e}"));
    }
    serde_urlencoded::from_bytes::<AgentEndpointRegistrationForm>(body)
        .map_err(|e| format!("Invalid form body: {e}"))?
        .into_registration()
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
///
/// Two spellings are accepted because the two encodings upstream and Batata
/// use do not agree on the shape: an upstream `Endpoint` is a `uri` (scheme,
/// host, port and path in one string), while Batata models an endpoint as an
/// address plus a port, which is what its Naming registration takes.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EndpointEntryInput {
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    /// Upstream's `Endpoint.uri`, e.g. `http://10.0.0.1:8080/a2a`.
    #[serde(default)]
    uri: Option<String>,
    /// Accepted from the API for forward-compatibility; not yet consumed by
    /// `AiEndpointService::create_agent_endpoint` which does not accept metadata.
    #[serde(default)]
    #[allow(dead_code)]
    metadata: HashMap<String, String>,
}

impl EndpointEntryInput {
    /// The `(address, port)` pair to register.
    fn resolve(&self) -> std::result::Result<(String, u16), String> {
        if let Some(address) = self.address.as_deref().filter(|a| !a.is_empty()) {
            let port = self
                .port
                .ok_or_else(|| "endpoint 'port' is required when 'address' is used".to_string())?;
            return Ok((address.to_string(), port));
        }
        let uri = self
            .uri
            .as_deref()
            .filter(|u| !u.is_empty())
            .ok_or_else(|| "endpoint requires 'address' and 'port', or a 'uri'".to_string())?;
        split_uri(uri).ok_or_else(|| format!("endpoint uri '{uri}' has no host"))
    }
}

/// Split an endpoint `uri` into its host and port.
///
/// Upstream carries the port inside the uri, so a client that sends one never
/// sends a port. A uri without a port is served on the default HTTP port, which
/// is the only sensible reading of an authority with no explicit port.
fn split_uri(uri: &str) -> Option<(String, u16)> {
    let authority = uri.split_once("://").map_or(uri, |(_, rest)| rest);
    let authority = authority.split(['/', '?']).next().unwrap_or(authority);
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, rest)| rest);
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, port.parse::<u16>().ok()?),
        None => (authority, 80),
    };
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), port))
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

            let result_page =
                Page::new(page.total_count, page_no as u64, page_size as u64, entries);

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
    let requested = query.version.as_deref().map(str::trim).unwrap_or("");

    // A missing version asks for the `latest` label — upstream
    // `AgentDiscoveryApplicationService#resolveVersion` defaults the label to
    // `latest` and only 404s when the agent carries no such label.
    let version = if requested.is_empty() {
        let versions = match agent_service.list_versions(ns, &query.agent_name).await {
            Ok(versions) => versions,
            Err(e) => {
                return Result::<()>::http_bad_request(
                    &batata_common::error::PARAMETER_VALIDATE_ERROR,
                    e.to_string(),
                );
            }
        };
        match versions.iter().find(|v| v.is_latest) {
            Some(v) => v.version.clone(),
            None => {
                return Result::<()>::http_not_found(
                    &batata_common::error::RESOURCE_NOT_FOUND,
                    format!("agent '{}' has no latest version", query.agent_name),
                );
            }
        }
    } else if crate::service::version_range::is_range(requested) {
        // A range is resolved against what is actually published; an exact
        // version is used as given, so asking for an unpublished one reports
        // "not found" rather than quietly serving a different version's
        // endpoints.
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
        match crate::service::version_range::select_version(&available, requested) {
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
        requested.to_string()
    };

    let endpoints =
        endpoint_service.get_agent_endpoints_for_version(ns, &query.agent_name, &version);
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
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let content_type = get_header(&req, "content-type");
    let form = match parse_endpoint_registration(&content_type, &body) {
        Ok(form) => form,
        Err(e) => {
            return Result::<String>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e,
            );
        }
    };
    let ns = normalize_namespace(form.namespace_id.as_deref().unwrap_or(""));

    // Upstream binds the external `clientId` header to an internal publisher
    // and admits the batch through the capacity gate. Batata has no
    // client-lifecycle store, so the header is used directly; without it there
    // is no Client to charge and the batch is admitted unconditionally.
    let client_id = get_header(&req, CLIENT_ID_HEADER);
    let endpoints = match form
        .endpoints
        .iter()
        .map(EndpointEntryInput::resolve)
        .collect::<std::result::Result<Vec<(String, u16)>, String>>()
    {
        Ok(endpoints) => endpoints,
        Err(e) => {
            return Result::<String>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e,
            );
        }
    };
    let version_range = Some(form.version_range.as_str()).filter(|r| !r.is_empty());

    if let Err(over) = endpoint_service.register_agent_endpoint_batch(
        &client_id,
        ns,
        &form.agent_name,
        &form.version,
        version_range,
        &endpoints,
    ) {
        // Upstream raises `NacosApiException(NacosException.OVER_THRESHOLD,
        // ErrorCode.AGENT_ENDPOINT_PUBLICATION_OVER_LIMIT, ...)`, i.e. HTTP 503
        // carrying business code 50103.
        let error_code = batata_common::error::AGENT_ENDPOINT_PUBLICATION_OVER_LIMIT;
        return Result::<String>::http_response(
            actix_web::http::StatusCode::SERVICE_UNAVAILABLE.as_u16(),
            error_code.code,
            error_code.message.to_string(),
            over.to_string(),
        );
    }

    tracing::info!(
        namespace = %ns,
        agent_name = %form.agent_name,
        version = %form.version,
        version_range = %form.version_range,
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

    // Upstream drops whatever *that client* published
    // (`AgentRuntimeRegistryService#deregisterPublisher`), so it reaches a
    // runtime version that was never published through the catalog. Batata's
    // Naming path carries no publisher identity, so the client's own
    // reservations are consulted alongside the catalog.
    let client_id = get_header(&req, CLIENT_ID_HEADER);

    // List all versions for this agent, then delete endpoints for each version
    match a2a_service.list_versions(ns, &form.agent_name).await {
        Ok(versions) => {
            let catalog: Vec<String> = versions.iter().map(|v| v.version.clone()).collect();
            endpoint_service.deregister_agent_publication(
                &client_id,
                ns,
                &form.agent_name,
                &catalog,
            );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uri_is_split_into_host_and_port() {
        assert_eq!(
            split_uri("http://10.0.0.1:8080/a2a"),
            Some(("10.0.0.1".to_string(), 8080))
        );
        assert_eq!(
            split_uri("https://agent.example.com:9443"),
            Some(("agent.example.com".to_string(), 9443))
        );
        // No scheme, no path: still an authority.
        assert_eq!(
            split_uri("10.0.0.2:7000"),
            Some(("10.0.0.2".to_string(), 7000))
        );
    }

    /// Upstream's `Endpoint` has no port field, so a uri without one is served
    /// on the default HTTP port rather than being rejected.
    #[test]
    fn a_uri_without_a_port_uses_the_default_http_port() {
        assert_eq!(
            split_uri("http://10.0.0.3/a2a"),
            Some(("10.0.0.3".to_string(), 80))
        );
    }

    #[test]
    fn a_uri_without_a_host_is_rejected() {
        assert_eq!(split_uri("http:///a2a"), None);
        assert_eq!(split_uri("http://:8080"), None);
        // A non-numeric port is not a port.
        assert_eq!(split_uri("http://10.0.0.1:http"), None);
    }

    #[test]
    fn an_endpoint_entry_resolves_from_either_spelling() {
        let entry: EndpointEntryInput =
            serde_json::from_str(r#"{"address":"h","port":1}"#).expect("address + port");
        assert_eq!(entry.resolve(), Ok(("h".to_string(), 1)));

        let entry: EndpointEntryInput =
            serde_json::from_str(r#"{"uri":"http://h:2/x"}"#).expect("uri");
        assert_eq!(entry.resolve(), Ok(("h".to_string(), 2)));
    }

    /// A half-specified endpoint is a client error, not a silent default.
    #[test]
    fn an_endpoint_entry_without_an_address_or_uri_is_rejected() {
        let entry: EndpointEntryInput = serde_json::from_str(r#"{"port":1}"#).expect("port only");
        assert!(entry.resolve().is_err());

        let entry: EndpointEntryInput =
            serde_json::from_str(r#"{"address":"h"}"#).expect("address only");
        assert!(entry.resolve().is_err());
    }
}
