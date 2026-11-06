//! Route-level test for the MCP client API (`/v3/client/ai/mcp`).
//!
//! Upstream anchor: `com.alibaba.nacos.ai.controller.McpClientController`
//! registers six endpoints on `Constants.MCP_CLIENT_PATH` (`/v3/client/ai/mcp`).
//! Batata ports them in `batata-ai/src/api/mcp_client.rs`. `query`/`release`/
//! `search` reuse the real MCP domain storage; the endpoint family is backed by
//! `AiEndpointService`, which registers/deregisters real NamingService instances.
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run the same cases
//! against a real database. See `common/ai_store.rs`.

#[path = "common/ai_store.rs"]
mod ai_store;

use std::sync::Arc;

use actix_web::http::{Method, StatusCode};
use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_ai::model::{
    McpCapability, McpServerRegistration, McpServerType, McpTool, McpTransport,
};
use batata_ai::{AiEndpointService, McpServerIndex, McpServerOperationService};
use batata_api::naming::NamingServiceProvider;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo, McpServerService};
use batata_naming::NamingService;
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NS: &str = "public";
const NAME: &str = "client-mcp";

/// Cluster manager stub. The MCP client routes never touch cluster state.
struct StubClusterManager;

impl ClusterManager for StubClusterManager {
    fn is_standalone(&self) -> bool {
        true
    }
    fn is_leader(&self) -> bool {
        true
    }
    fn is_cluster_healthy(&self) -> bool {
        true
    }
    fn leader_address(&self) -> Option<String> {
        None
    }
    fn local_address(&self) -> &str {
        "127.0.0.1:8848"
    }
    fn member_count(&self) -> usize {
        1
    }
    fn all_members_extended(&self) -> Vec<ExtendedMemberInfo> {
        vec![]
    }
    fn healthy_members_extended(&self) -> Vec<ExtendedMemberInfo> {
        vec![]
    }
    fn get_member(&self, _address: &str) -> Option<ExtendedMemberInfo> {
        None
    }
    fn get_self_member(&self) -> ExtendedMemberInfo {
        unimplemented!("not used by the MCP client routes")
    }
    fn health_summary(&self) -> ClusterHealthSummary {
        ClusterHealthSummary::default()
    }
    fn refresh_self(&self) {}
    fn is_self(&self, _address: &str) -> bool {
        false
    }
    fn update_member_state(&self, _address: &str, _state: &str) -> Result<String, String> {
        Ok("UP".to_string())
    }
}

/// Build an actix app exposing the real MCP client routes.
async fn build_app(
    store: Arc<dyn PersistenceService>,
    mcp: Arc<dyn McpServerService>,
    endpoints: Arc<AiEndpointService>,
) -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    // `secured!` gates every handler; the default config has console auth on.
    let mut configuration = Configuration::default();
    configuration.typed.core.auth.console.enabled = false;

    let subscriber: Arc<dyn batata_common::ConfigSubscriptionService> =
        Arc::new(batata_core::ConfigSubscriberManager::new());
    let cluster: Arc<dyn ClusterManager> = Arc::new(StubClusterManager);

    let console_datasource = Arc::new(batata_console::datasource::local::LocalDataSource::new(
        store.clone(),
        cluster,
        subscriber.clone(),
        configuration.clone(),
        None,
        vec![],
    ));

    let app_state = Arc::new(AppState {
        configuration,
        cluster_manager: None,
        config_subscriber_manager: subscriber,
        console_datasource,
        oauth_service: None,
        auth_plugin: None,
        persistence: Some(store),
        auth_data: None,
        health_check_manager: None,
        raft_node: None,
        server_status: Arc::new(batata_server_common::ServerStatusManager::new()),
        control_plugin: None,
        encryption_service: None,
        plugin_state_providers: vec![],
        plugin_manager: None,
        log_level_setter: None,
        connection_manager: Arc::new(std::sync::OnceLock::new()),
    });

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            // `Data::new`, not `Data::from`: the handlers take `Data<Arc<...>>`.
            .app_data(web::Data::new(mcp))
            .app_data(web::Data::new(endpoints))
            // Mounted exactly as `api/v3/client/route.rs` does.
            .service(web::scope("/v3/client/ai").service(batata_ai::mcp_client_routes())),
    )
    .await
}

/// Open the store and build the backing services.
async fn setup() -> (
    TestStore,
    Arc<McpServerOperationService>,
    Arc<AiEndpointService>,
) {
    let store = TestStore::open().await;
    let index = Arc::new(McpServerIndex::new());
    let mcp = Arc::new(McpServerOperationService::new(store.store(), index));
    let naming: Arc<dyn NamingServiceProvider> = Arc::new(NamingService::new());
    let endpoints = Arc::new(AiEndpointService::new(naming));
    store.clean(NAME).await;
    (store, mcp, endpoints)
}

/// A minimal MCP server registration for `release` / `create`.
fn registration(version: &str, tool: &str) -> McpServerRegistration {
    McpServerRegistration {
        name: NAME.to_string(),
        display_name: "Client MCP".to_string(),
        description: format!("desc-{version}"),
        namespace: NS.to_string(),
        version: version.to_string(),
        endpoint: "http://localhost:8080".to_string(),
        server_type: McpServerType::Http,
        transport: McpTransport::default(),
        capabilities: vec![McpCapability::Tool],
        tools: vec![McpTool {
            name: tool.to_string(),
            description: format!("tool-{tool}"),
            input_schema: serde_json::json!({"type": "object"}),
        }],
        resources: vec![],
        prompts: vec![],
        metadata: std::collections::HashMap::new(),
        tags: vec![],
        auto_fetch_tools: true,
        health_check: None,
    }
}

/// Dispatch a request and return `(status, body)`.
async fn call<S>(app: &S, req: actix_http::Request) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    (
        status,
        String::from_utf8_lossy(&test::read_body(resp).await).to_string(),
    )
}

async fn get<S>(app: &S, uri: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    call(app, test::TestRequest::get().uri(uri).to_request()).await
}

// ----------------------------------------------------------------------------
// query (GET /)
// ----------------------------------------------------------------------------

/// `query` must 404 for a server that does not exist, not 200 with an empty doc.
#[actix_web::test]
async fn query_returns_404_for_unknown_server() {
    let (store, mcp, endpoints) = setup().await;
    let app = build_app(store.store(), mcp, endpoints).await;

    let (status, body) = get(&app, "/v3/client/ai/mcp?mcpName=does-not-exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unknown server: {body}");
}

/// `query` requires `mcpName` on the wire.
#[actix_web::test]
async fn query_requires_mcp_name() {
    let (store, mcp, endpoints) = setup().await;
    let app = build_app(store.store(), mcp, endpoints).await;

    let (status, body) = get(&app, "/v3/client/ai/mcp").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "missing mcpName: {body}");
}

/// `query` returns the serving version for an existing server.
#[actix_web::test]
async fn query_returns_200_for_existing_server() {
    let (store, mcp, endpoints) = setup().await;
    mcp.create_mcp_server(NS, &registration("1.0.0", "echo"))
        .await
        .expect("create server");
    let app = build_app(store.store(), mcp, endpoints).await;

    let (status, body) = get(&app, "/v3/client/ai/mcp?mcpName=client-mcp&version=1.0.0").await;
    assert_eq!(status, StatusCode::OK, "query should succeed: {body}");
    assert!(body.contains("client-mcp"), "body names the server: {body}");
    assert!(body.contains("1.0.0"), "body names the version: {body}");
}

// ----------------------------------------------------------------------------
// release (POST /)
// ----------------------------------------------------------------------------

/// `release` creates a server version directly and returns its stable id.
#[actix_web::test]
async fn release_creates_server_and_returns_id() {
    let (store, mcp, endpoints) = setup().await;
    let app = build_app(store.store(), mcp.clone(), endpoints).await;

    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/client/ai/mcp")
            .set_json(registration("2.0.0", "calc"))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "release must succeed: {body}");
    // Upstream returns the stable internal MCP id (a non-empty string).
    assert!(!body.trim().is_empty(), "release returns an id: {body}");

    // The released version is now queryable.
    let (status, body) = get(&app, "/v3/client/ai/mcp?mcpName=client-mcp&version=2.0.0").await;
    assert_eq!(status, StatusCode::OK, "released version queryable: {body}");
}

/// `release?createDraft=true` returns the draft detail.
#[actix_web::test]
async fn release_draft_returns_detail() {
    let (store, mcp, endpoints) = setup().await;
    let app = build_app(store.store(), mcp, endpoints).await;

    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/client/ai/mcp?createDraft=true")
            .set_json(registration("3.0.0", "draft-tool"))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "draft release must succeed: {body}");
    // Upstream returns the stable internal MCP id for both the direct-online
    // and the draft path, so the body carries a non-empty `data` id.
    assert!(
        body.contains("\"data\""),
        "draft release returns data: {body}"
    );
}

// ----------------------------------------------------------------------------
// endpoint family (POST /endpoints, DELETE /endpoints, PUT /endpoints/heartbeat)
// ----------------------------------------------------------------------------

/// `register_endpoint` requires the `clientId` header.
#[actix_web::test]
async fn register_endpoint_requires_client_id() {
    let (store, mcp, endpoints) = setup().await;
    let app = build_app(store.store(), mcp, endpoints).await;

    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/client/ai/mcp/endpoints")
            .set_json(serde_json::json!({
                "mcpName": NAME,
                "address": "10.0.0.1",
                "port": 8080,
                "version": "1.0.0"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "missing clientId: {body}");
}

/// `register_endpoint` rejects an out-of-range port.
#[actix_web::test]
async fn register_endpoint_rejects_bad_port() {
    let (store, mcp, endpoints) = setup().await;
    let app = build_app(store.store(), mcp, endpoints).await;

    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/client/ai/mcp/endpoints")
            .insert_header(("clientId", "client-1"))
            .set_json(serde_json::json!({
                "mcpName": NAME,
                "address": "10.0.0.1",
                "port": 0,
                "version": "1.0.0"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "port 0 rejected: {body}");
}

/// `register_endpoint` stores a real NamingService instance, and `heartbeat`
/// returns the client liveness contract.
#[actix_web::test]
async fn register_endpoint_then_heartbeat() {
    const VERSION: &str = "1.0.0";
    let (store, mcp, endpoints) = setup().await;
    mcp.create_mcp_server(NS, &registration(VERSION, "echo"))
        .await
        .expect("create server");
    let endpoints_clone = endpoints.clone();
    let app = build_app(store.store(), mcp, endpoints).await;

    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/client/ai/mcp/endpoints")
            .insert_header(("clientId", "client-1"))
            .set_json(serde_json::json!({
                "mcpName": NAME,
                "address": "10.0.0.1",
                "port": 8080,
                "version": VERSION
            }))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register must succeed: {body}");
    // `ClientLivenessInfo` is camelCase on the wire.
    assert!(body.contains("\"heartbeatIntervalMillis\":5000"), "{body}");
    assert!(body.contains("\"unhealthyTimeoutMillis\":15000"), "{body}");
    assert!(body.contains("\"expireTimeoutMillis\":30000"), "{body}");

    // The endpoint was really registered in NamingService.
    let registered = endpoints_clone.get_mcp_endpoints(NS, NAME, VERSION);
    assert_eq!(
        registered.len(),
        1,
        "endpoint should be registered: {:?}",
        registered
    );
    assert_eq!(registered[0].address, "10.0.0.1");
    assert_eq!(registered[0].port, 8080);

    // Deregister it again.
    let (status, _body) = call(
        &app,
        test::TestRequest::delete()
            .uri("/v3/client/ai/mcp/endpoints?mcpName=client-mcp&address=10.0.0.1&port=8080&version=1.0.0")
            .insert_header(("clientId", "client-1"))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "deregister must succeed");

    let registered = endpoints_clone.get_mcp_endpoints(NS, NAME, VERSION);
    assert!(
        registered.is_empty(),
        "endpoint should be deregistered: {:?}",
        registered
    );
}

/// `heartbeat` returns the liveness contract even without a prior registration.
#[actix_web::test]
async fn heartbeat_returns_the_client_liveness_contract() {
    let (store, mcp, endpoints) = setup().await;
    let app = build_app(store.store(), mcp, endpoints).await;

    let (status, body) = call(
        &app,
        test::TestRequest::with_uri("/v3/client/ai/mcp/endpoints/heartbeat")
            .method(Method::PUT)
            .insert_header(("clientId", "client-1"))
            .insert_header(("requestModule", "nacos-sdk"))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "heartbeat must succeed: {body}");
    assert!(body.contains("\"heartbeatIntervalMillis\":5000"), "{body}");
    assert!(body.contains("\"unhealthyTimeoutMillis\":15000"), "{body}");
    assert!(body.contains("\"expireTimeoutMillis\":30000"), "{body}");
}

// ----------------------------------------------------------------------------
// search (GET /search)
// ----------------------------------------------------------------------------

/// `search` returns a page of MCP servers.
#[actix_web::test]
async fn search_returns_a_page() {
    let (store, mcp, endpoints) = setup().await;
    mcp.create_mcp_server(NS, &registration("1.0.0", "echo"))
        .await
        .expect("create server");
    let app = build_app(store.store(), mcp, endpoints).await;

    let (status, body) = get(&app, "/v3/client/ai/mcp/search").await;
    assert_eq!(status, StatusCode::OK, "search must succeed: {body}");
    assert!(
        body.contains("\"totalCount\""),
        "page has totalCount: {body}"
    );
}
