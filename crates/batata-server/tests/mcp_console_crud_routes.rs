//! Route-level test for the console MCP endpoints that `mcp_console_routes`
//! does not reach: the plain CRUD surface (`list`, detail, create, update,
//! delete), `stats`, `status` and `importToolsFromMcp`.
//!
//! `mcp_console_routes` covers the version lifecycle and the import batch;
//! these are the ones that were left with no coverage at all.
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run the same cases
//! against a real database. See `common/ai_store.rs`.

#[path = "common/ai_store.rs"]
mod ai_store;

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_ai::model::{
    McpCapability, McpServerRegistration, McpServerType, McpTool, McpTransport,
};
use batata_ai::{McpServerIndex, McpServerOperationService};
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NS: &str = "public";
const NAME: &str = "crud-route-mcp";
/// A second name, so list filtering has something to exclude.
const OTHER: &str = "crud-route-mcp-other";

/// Cluster manager stub. The MCP routes never touch cluster state.
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
        unimplemented!("not used by the MCP console routes")
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

fn registration(name: &str, version: &str, tool: &str) -> McpServerRegistration {
    McpServerRegistration {
        name: name.to_string(),
        display_name: "CRUD Route MCP".to_string(),
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

/// Build an actix app exposing the real console MCP routes.
async fn build_app(
    store: Arc<dyn PersistenceService>,
    svc: Arc<McpServerOperationService>,
) -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    // `secured!` gates every handler on console auth; `CoreAuthConsoleConfig`
    // defaults to enabled, so turn it off for the test.
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

    let mcp: Arc<dyn batata_common::McpServerService> = svc;

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(mcp))
            .service(batata_console::v3::ai_mcp::routes()),
    )
    .await
}

/// Open the store and build the backing service.
async fn setup() -> (TestStore, Arc<McpServerOperationService>) {
    let store = TestStore::open().await;
    let index = Arc::new(McpServerIndex::new());
    let svc = Arc::new(McpServerOperationService::new(store.store(), index));
    store.clean(NAME).await;
    store.clean(OTHER).await;
    (store, svc)
}

/// GET /ai/mcp/list returns the servers that exist.
#[actix_web::test]
async fn list_endpoint_lists_the_created_servers() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration(NAME, "1.0.0", "echo"))
        .await
        .expect("create first server");
    svc.create_mcp_server(NS, &registration(OTHER, "1.0.0", "ping"))
        .await
        .expect("create second server");

    let app = build_app(store.store(), svc.clone()).await;
    let req = test::TestRequest::get().uri("/ai/mcp/list").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "list should return 200");

    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(body.contains(NAME), "list must contain the first: {body}");
    assert!(body.contains(OTHER), "list must contain the second: {body}");

    store.clean(NAME).await;
    store.clean(OTHER).await;
}

/// An accurate name filter excludes the other server.
#[actix_web::test]
async fn list_endpoint_filters_by_name() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration(NAME, "1.0.0", "echo"))
        .await
        .expect("create first server");
    svc.create_mcp_server(NS, &registration(OTHER, "1.0.0", "ping"))
        .await
        .expect("create second server");

    let app = build_app(store.store(), svc.clone()).await;
    let req = test::TestRequest::get()
        .uri(&format!("/ai/mcp/list?mcpName={NAME}&search=accurate"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(
        body.contains(NAME),
        "the requested server must be listed: {body}"
    );
    assert!(
        !body.contains(OTHER),
        "an accurate filter must exclude the other server: {body}"
    );

    store.clean(NAME).await;
    store.clean(OTHER).await;
}

/// GET /ai/mcp returns one server, and 404s for one that does not exist.
#[actix_web::test]
async fn detail_endpoint_returns_one_server() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration(NAME, "1.0.0", "echo"))
        .await
        .expect("create server");

    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::get()
        .uri(&format!("/ai/mcp?mcpName={NAME}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "detail should return 200");
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(body.contains(NAME), "detail must name the server: {body}");
    assert!(body.contains("echo"), "detail must carry the tool: {body}");

    let req = test::TestRequest::get()
        .uri("/ai/mcp?mcpName=does-not-exist")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "an unknown server must 404"
    );

    store.clean(NAME).await;
}

/// POST /ai/mcp creates a server from the JSON body the console sends.
#[actix_web::test]
async fn register_endpoint_creates_a_server() {
    let (store, svc) = setup().await;
    let app = build_app(store.store(), svc.clone()).await;

    let body = registration(NAME, "2.0.0", "summarize");
    let req = test::TestRequest::post()
        .uri("/ai/mcp")
        .set_json(&body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "create should return 200");

    // The server really landed in persistence, not just in the response.
    let found = store
        .store()
        .ai_resource_find(NS, NAME, "mcp")
        .await
        .expect("resource lookup")
        .expect("the created server must be persisted");
    assert_eq!(found.name, NAME);

    store.clean(NAME).await;
}

/// PUT /ai/mcp applies the body to the server named in the query.
#[actix_web::test]
async fn update_endpoint_changes_the_server() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration(NAME, "1.0.0", "echo"))
        .await
        .expect("create server");

    let app = build_app(store.store(), svc.clone()).await;

    let mut body = registration(NAME, "1.0.0", "echo");
    body.description = "updated-description".to_string();
    let req = test::TestRequest::put()
        .uri(&format!("/ai/mcp?mcpName={NAME}"))
        .set_json(&body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "update should return 200");

    // Read it back through the detail endpoint rather than the service, so
    // the test proves the write is visible to the next request.
    let req = test::TestRequest::get()
        .uri(&format!("/ai/mcp?mcpName={NAME}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(
        body.contains("updated-description"),
        "the update must be visible: {body}"
    );

    store.clean(NAME).await;
}

/// DELETE /ai/mcp removes the server.
#[actix_web::test]
async fn delete_endpoint_removes_the_server() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration(NAME, "1.0.0", "echo"))
        .await
        .expect("create server");

    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::delete()
        .uri(&format!("/ai/mcp?mcpName={NAME}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "delete should return 200");

    assert!(
        store
            .store()
            .ai_resource_find(NS, NAME, "mcp")
            .await
            .expect("resource lookup")
            .is_none(),
        "the server must be gone from persistence"
    );

    store.clean(NAME).await;
}

/// Deleting a server that does not exist is a 404, not a silent success.
#[actix_web::test]
async fn delete_endpoint_404s_for_an_unknown_server() {
    let (store, svc) = setup().await;
    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::delete()
        .uri("/ai/mcp?mcpName=does-not-exist")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "an unknown server must 404"
    );
}

/// PUT /ai/mcp/status flips the resource status the console drives.
#[actix_web::test]
async fn status_endpoint_flips_the_resource_status() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration(NAME, "1.0.0", "echo"))
        .await
        .expect("create server");

    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::put()
        .uri(&format!("/ai/mcp/status?mcpName={NAME}&status=disable"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "disable should return 200");
    assert_eq!(
        store
            .store()
            .ai_resource_find(NS, NAME, "mcp")
            .await
            .expect("resource lookup")
            .expect("server must still exist")
            .status
            .as_deref(),
        Some(batata_ai::repository::meta_status::DISABLE),
        "disabling must flip the resource status"
    );

    let req = test::TestRequest::put()
        .uri(&format!("/ai/mcp/status?mcpName={NAME}&status=enable"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "enable should return 200");
    assert_eq!(
        store
            .store()
            .ai_resource_find(NS, NAME, "mcp")
            .await
            .expect("resource lookup")
            .expect("server must still exist")
            .status
            .as_deref(),
        Some(batata_ai::repository::meta_status::ENABLE),
        "enabling must flip it back"
    );

    store.clean(NAME).await;
}

/// A status that is neither `enable` nor `disable` is rejected.
#[actix_web::test]
async fn status_endpoint_rejects_an_unknown_value() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration(NAME, "1.0.0", "echo"))
        .await
        .expect("create server");

    let app = build_app(store.store(), svc.clone()).await;
    let req = test::TestRequest::put()
        .uri(&format!("/ai/mcp/status?mcpName={NAME}&status=sideways"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "an unknown status must 400"
    );

    store.clean(NAME).await;
}

/// GET /ai/mcp/stats is wired through to the service.
///
/// The counters are still placeholders — the AI-resource-backed service does
/// not track registry-wide statistics — so this pins the contract (200 and the
/// documented shape) rather than the numbers.
#[actix_web::test]
async fn stats_endpoint_returns_the_registry_shape() {
    let (store, svc) = setup().await;
    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::get().uri("/ai/mcp/stats").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "stats should return 200");

    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    for field in [
        "totalServers",
        "healthyServers",
        "unhealthyServers",
        "byNamespace",
        "byType",
    ] {
        assert!(body.contains(field), "stats must carry `{field}`: {body}");
    }
}

/// GET /ai/mcp/importToolsFromMcp only speaks `mcp-sse`.
///
/// Reaching a live MCP server is out of scope for a route test; what matters
/// here is that an unsupported transport is refused before any network call.
#[actix_web::test]
async fn import_tools_endpoint_rejects_an_unsupported_transport() {
    let (store, svc) = setup().await;
    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::get()
        .uri("/ai/mcp/importToolsFromMcp?transportType=mcp-streamable&baseUrl=http://127.0.0.1:9&endpoint=/sse")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "an unsupported transport must be refused"
    );
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(
        body.contains("Unsupported transport type"),
        "the refusal must say why: {body}"
    );
}
