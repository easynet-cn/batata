//! Route-level test for the console MCP management API.
//!
//! The service layer is covered by `batata-ai`'s `mcp_persistence` test. This
//! one covers the HTTP layer on top of it: that the endpoints are actually
//! registered on the scope, and that query / body parameters reach the service.
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-server --test mcp_console_routes -- --ignored --nocapture
//!
//! DATABASE_URL="postgres://postgres:devterry@127.0.0.1:5432/batata_ai_test" \
//!   cargo test -p batata-server --test mcp_console_routes -- --ignored --nocapture
//! ```

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{test, web, App};
use batata_ai::model::{
    McpCapability, McpServerRegistration, McpServerType, McpTool, McpTransport,
};
use batata_ai::{McpServerIndex, McpServerOperationService};
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::ExternalDbPersistService;
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NS: &str = "public";
const NAME: &str = "route-mcp";

/// Cluster manager stub. The MCP routes never touch cluster state, so every
/// method returns a fixed value; the ones that would need real member data are
/// never reached.
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

fn registration(version: &str, tool: &str) -> McpServerRegistration {
    McpServerRegistration {
        name: NAME.to_string(),
        display_name: "Route MCP".to_string(),
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

/// Remove rows left by a previous run so the test is repeatable.
async fn clean(store: &ExternalDbPersistService) {
    let db = store.db();
    ai_resource_version::Entity::delete_many()
        .filter(ai_resource_version::Column::Name.eq(NAME))
        .exec(db)
        .await
        .expect("clean versions");
    ai_resource::Entity::delete_many()
        .filter(ai_resource::Column::Name.eq(NAME))
        .exec(db)
        .await
        .expect("clean resources");
}

/// Build an actix app exposing the real console MCP routes, backed by a real
/// database and the real MCP service.
async fn build_app(
    store: Arc<ExternalDbPersistService>,
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
        health_check_manager: None,
        raft_node: None,
        server_status: Arc::new(batata_server_common::ServerStatusManager::new()),
        control_plugin: None,
        encryption_service: None,
        plugin_state_providers: vec![],
        plugin_manager: None,
        log_level_setter: None,
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

/// Connect, clean and build the app together with the backing service.
async fn setup() -> (Arc<ExternalDbPersistService>, Arc<McpServerOperationService>) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = Database::connect(&url)
        .await
        .unwrap_or_else(|e| panic!("connect {url}: {e}"));
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let index = Arc::new(McpServerIndex::new());
    let svc = Arc::new(McpServerOperationService::new(store.clone(), index));
    clean(&store).await;
    (store, svc)
}

/// GET /ai/mcp/versions lists the versions created through the service.
#[actix_web::test]
#[ignore]
async fn versions_endpoint_lists_versions() {
    let (store, svc) = setup().await;
    // Drafts only exist for a server that has already been created.
    svc.create_mcp_server(NS, &registration("1.0.0", "echo"))
        .await
        .expect("create server");

    let app = build_app(store.clone(), svc.clone()).await;
    let req = test::TestRequest::get()
        .uri("/ai/mcp/versions?mcpName=route-mcp")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK, "versions should return 200");
    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("1.0.0"), "body should list the version: {body}");

    clean(&store).await;
}

/// GET /ai/mcp/version returns one version.
#[actix_web::test]
#[ignore]
async fn version_endpoint_returns_one_version() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration("2.0.0", "echo"))
        .await
        .expect("create server");

    let app = build_app(store.clone(), svc.clone()).await;
    let req = test::TestRequest::get()
        .uri("/ai/mcp/version?mcpName=route-mcp&version=2.0.0")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK, "version should return 200");
    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("2.0.0"), "body should name the version: {body}");

    clean(&store).await;
}

/// A missing required parameter is rejected with 400 rather than reaching the
/// service.
#[actix_web::test]
#[ignore]
async fn versions_endpoint_requires_mcp_name() {
    let (store, svc) = setup().await;
    let app = build_app(store.clone(), svc.clone()).await;

    let req = test::TestRequest::get().uri("/ai/mcp/versions").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "missing mcpName should be rejected"
    );

    clean(&store).await;
}

/// An unknown scope is rejected with 400 by the handler, before the service
/// sees it.
#[actix_web::test]
#[ignore]
async fn scope_endpoint_rejects_unknown_value() {
    let (store, svc) = setup().await;
    let app = build_app(store.clone(), svc.clone()).await;

    let req = test::TestRequest::put()
        .uri("/ai/mcp/status?mcpName=route-mcp&status=bogus")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "unknown status should be rejected"
    );

    clean(&store).await;
}

/// POST /ai/mcp/draft reaches the service and creates the version.
#[actix_web::test]
#[ignore]
async fn draft_endpoint_creates_version() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration("1.0.0", "echo"))
        .await
        .expect("create server");

    let app = build_app(store.clone(), svc.clone()).await;

    let req = test::TestRequest::post()
        .uri("/ai/mcp/draft?overwrite=false")
        .set_json(registration("3.0.0", "echo"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK, "draft create should succeed");

    // The version must now be visible through the read endpoint.
    let req = test::TestRequest::get()
        .uri("/ai/mcp/versions?mcpName=route-mcp")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("3.0.0"), "draft should be listed: {body}");

    clean(&store).await;
}
