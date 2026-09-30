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
    AgentCapabilities, AgentCard, AgentSkill, McpCapability, McpServerRegistration, McpServerType,
    McpTool, McpTransport,
};
use batata_ai::{A2aServerOperationService, McpServerIndex, McpServerOperationService};
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::ExternalDbPersistService;
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NS: &str = "public";
const NAME: &str = "route-mcp";

/// Connect to the test database, failing fast with an actionable message.
///
/// A stopped Podman container leaves the forwarded port *accepting* TCP while
/// nothing completes the handshake, so a plain `Database::connect` blocks for
/// the default ~30s per test. With fifteen tests here that is several minutes
/// of silence ending in a pool-timeout message that reads like a code bug.
async fn connect_database(url: &str) -> sea_orm::DatabaseConnection {
    use std::time::Duration;
    let mut options = sea_orm::ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(3))
        .acquire_timeout(Duration::from_secs(3));
    match sea_orm::Database::connect(options).await {
        Ok(connection) => connection,
        Err(error) => panic!(
            "cannot reach the test database at {url}: {error}\n\
             hint: containers do not come back on their own after a machine \
             restart or a podman machine stop — try `podman start mysql postgres`"
        ),
    }
}

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

    // Cloned before `store` is moved into `AppState`: the agent service needs
    // its own handle to the same persistence.
    let agent_store = store.clone();

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

    // Agent admin routes are mounted in the same app so the agent version
    // lifecycle can be exercised over HTTP without rebuilding AppState.
    let agents: Arc<dyn batata_common::A2aAgentService> =
        Arc::new(batata_ai::A2aServerOperationService::new(agent_store));

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(mcp))
            .app_data(web::Data::new(agents))
            .service(batata_console::v3::ai_mcp::routes())
            .service(web::scope("/v3/admin/ai").service(batata_ai::agent_admin_routes())),
    )
    .await
}

/// Connect, clean and build the app together with the backing service.
async fn setup() -> (Arc<ExternalDbPersistService>, Arc<McpServerOperationService>) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
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

/// GET /ai/mcp/search returns ranked hits once the index has been built.
#[actix_web::test]
#[ignore]
async fn search_endpoint_returns_ranked_hits() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration("1.0.0", "echo"))
        .await
        .expect("create server");

    // Build the index the same way the background consumer would.
    let consumer = batata_ai::search::consumer::AiResourceIndexConsumer::new(
        store.clone() as std::sync::Arc<dyn batata_persistence::PersistenceService>,
    );
    svc.publish_mcp_server_version(NS, NAME, "1.0.0")
        .await
        .expect("publish schedules the task");
    consumer.consume_once().await.expect("consume");

    let app = build_app(store.clone(), svc.clone()).await;

    let req = test::TestRequest::get()
        .uri("/ai/mcp/search?query=echo")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "search should succeed");

    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains(NAME), "the server must be found: {body}");
    assert!(body.contains("score"), "hits must carry a score: {body}");

    // A term that matches nothing returns an empty page, not an error.
    let req = test::TestRequest::get()
        .uri("/ai/mcp/search?query=zzzznomatchzzzz")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    clean(&store).await;
}

/// A missing query is rejected with 400.
#[actix_web::test]
#[ignore]
async fn search_endpoint_requires_query() {
    let (store, svc) = setup().await;
    let app = build_app(store.clone(), svc.clone()).await;

    let req = test::TestRequest::get().uri("/ai/mcp/search").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a missing query must be rejected"
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

/// Create the server, then a draft version on top of it. The service requires
/// the resource to exist before a draft can be added.
async fn server_with_draft(svc: &McpServerOperationService, base: &str, draft: &str) {
    svc.create_mcp_server(NS, &registration(base, "echo"))
        .await
        .unwrap_or_else(|e| panic!("create server {base}: {e}"));
    svc.create_mcp_server_draft(NS, &registration(draft, "echo"), false)
        .await
        .unwrap_or_else(|e| panic!("create draft {draft}: {e}"));
}

/// Assert a lifecycle endpoint moved a version to `expected_status`.
async fn assert_transition(uri: &str, expected_status: &str, label: &str) {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration("1.0.0", "echo"))
        .await
        .expect("create server");
    let app = build_app(store.clone(), svc.clone()).await;

    let req = test::TestRequest::post().uri(uri).to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK, "{label} should succeed");
    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);
    assert!(
        body.contains(expected_status),
        "{label} should move the version to '{expected_status}': {body}"
    );

    clean(&store).await;
}

/// POST /ai/mcp/submit moves a draft to reviewing.
#[actix_web::test]
#[ignore]
async fn submit_endpoint_moves_draft_to_reviewing() {
    let (store, svc) = setup().await;
    server_with_draft(&svc, "1.0.0", "2.0.0").await;

    let app = build_app(store.clone(), svc.clone()).await;
    let req = test::TestRequest::post()
        .uri("/ai/mcp/submit?mcpName=route-mcp&version=2.0.0")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK, "submit should succeed");
    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("reviewing"), "submit should review: {body}");

    clean(&store).await;
}

/// POST /ai/mcp/publish moves a reviewed version online.
#[actix_web::test]
#[ignore]
async fn publish_endpoint_moves_reviewing_to_online() {
    let (store, svc) = setup().await;
    server_with_draft(&svc, "1.0.0", "2.0.0").await;
    svc.submit_mcp_server_version(NS, NAME, "2.0.0")
        .await
        .expect("submit");

    let app = build_app(store.clone(), svc.clone()).await;
    let req = test::TestRequest::post()
        .uri("/ai/mcp/publish?mcpName=route-mcp&version=2.0.0")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK, "publish should succeed");
    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("online"), "publish should go online: {body}");

    clean(&store).await;
}

/// POST /ai/mcp/force-publish publishes without passing review.
#[actix_web::test]
#[ignore]
async fn force_publish_endpoint_skips_review() {
    let (store, svc) = setup().await;
    server_with_draft(&svc, "1.0.0", "2.0.0").await;

    let app = build_app(store.clone(), svc.clone()).await;
    let req = test::TestRequest::post()
        .uri("/ai/mcp/force-publish?mcpName=route-mcp&version=2.0.0")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK, "force-publish should succeed");
    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("online"), "force-publish should go online: {body}");

    clean(&store).await;
}

/// POST /ai/mcp/redraft moves a version back to draft.
#[actix_web::test]
#[ignore]
async fn redraft_endpoint_moves_version_back_to_draft() {
    assert_transition("/ai/mcp/redraft?mcpName=route-mcp&version=1.0.0", "draft", "redraft").await;
}

/// POST /ai/mcp/offline takes an online version offline.
#[actix_web::test]
#[ignore]
async fn offline_endpoint_takes_version_offline() {
    assert_transition("/ai/mcp/offline?mcpName=route-mcp&version=1.0.0", "offline", "offline").await;
}

/// POST /ai/mcp/online brings an offline version back online.
#[actix_web::test]
#[ignore]
async fn online_endpoint_brings_version_back_online() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration("1.0.0", "echo"))
        .await
        .expect("create server");
    svc.offline_mcp_server_version(NS, NAME, "1.0.0")
        .await
        .expect("offline");

    let app = build_app(store.clone(), svc.clone()).await;
    let req = test::TestRequest::post()
        .uri("/ai/mcp/online?mcpName=route-mcp&version=1.0.0")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK, "online should succeed");
    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("online"), "online should go online: {body}");

    clean(&store).await;
}

/// PUT /ai/mcp/labels replaces custom labels but keeps the server-managed
/// `latest` label.
#[actix_web::test]
#[ignore]
async fn labels_endpoint_replaces_labels_and_keeps_latest() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration("1.0.0", "echo"))
        .await
        .expect("create server");

    let app = build_app(store.clone(), svc.clone()).await;
    let req = test::TestRequest::put()
        .uri("/ai/mcp/labels?mcpName=route-mcp")
        .set_json(serde_json::json!({"labels": {"stable": "1.0.0"}}))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK, "labels should succeed");
    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("stable"), "custom label must be stored: {body}");
    assert!(body.contains("latest"), "the latest label must survive: {body}");

    clean(&store).await;
}

/// PUT /ai/mcp/scope changes the visibility scope.
#[actix_web::test]
#[ignore]
async fn scope_endpoint_sets_scope() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration("1.0.0", "echo"))
        .await
        .expect("create server");

    let app = build_app(store.clone(), svc.clone()).await;
    let req = test::TestRequest::put()
        .uri("/ai/mcp/scope?mcpName=route-mcp&scope=PUBLIC")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK, "scope should succeed");

    clean(&store).await;
}

/// Post a JSON body to `/ai/mcp/import/validate` and return the response body.
async fn validate_payload<S>(app: &S, content: &str) -> String
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let req = test::TestRequest::post()
        .uri("/ai/mcp/import/validate")
        .set_json(serde_json::json!({ "content": content }))
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "validate should answer 200");
    let body = test::read_body(resp).await;
    String::from_utf8_lossy(&body).to_string()
}

/// Import validation must judge the payload semantically, not merely parse it
/// as JSON — that is the whole point of upstream's validation service.
#[actix_web::test]
#[ignore]
async fn import_validate_rejects_an_incomplete_server() {
    let (store, svc) = setup().await;
    let app = build_app(store.clone(), svc.clone()).await;

    // Parses fine, but has no protocol, no description and no remote config.
    let body = validate_payload(&app, r#"[{"name":"broken"}]"#).await;
    assert!(body.contains("\"valid\":false"), "must be invalid: {body}");
    assert!(body.contains("Protocol is required"), "missing protocol: {body}");
    assert!(
        body.contains("Description is required"),
        "missing description: {body}"
    );

    clean(&store).await;
}

#[actix_web::test]
#[ignore]
async fn import_validate_accepts_a_complete_server() {
    let (store, svc) = setup().await;
    let app = build_app(store.clone(), svc.clone()).await;

    let content = r#"[{"name":"complete","protocol":"http","description":"does things","remoteServerConfig":{"url":"http://localhost:8080"}}]"#;
    let body = validate_payload(&app, content).await;
    assert!(body.contains("\"valid\":true"), "must be valid: {body}");
    assert!(body.contains("\"validCount\":1"), "one valid server: {body}");

    clean(&store).await;
}

// ---- agent admin endpoints (`/v3/admin/ai/agents`) --------------------------

/// Build an agent card for `name` at `version`.
fn agent_card(name: &str, version: &str) -> AgentCard {
    AgentCard {
        name: name.to_string(),
        display_name: name.to_string(),
        description: "route agent".to_string(),
        version: version.to_string(),
        url: "http://localhost:8080".to_string(),
        protocol_version: "1.0".to_string(),
        capabilities: AgentCapabilities {
            streaming: true,
            tool_use: true,
            ..Default::default()
        },
        skills: vec![AgentSkill {
            name: "coding".to_string(),
            description: "code generation".to_string(),
            proficiency: 90,
            examples: vec![],
        }],
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        preferred_transport: None,
        provider: None,
        documentation_url: None,
        icon_url: None,
        supports_authenticated_extended_card: None,
        metadata: Default::default(),
        tags: vec![],
    }
}

/// Register an agent so it has an online version to act on.
async fn register_agent(svc: &A2aServerOperationService, name: &str) {
    svc.register_agent(&agent_card(name, "1.0.0"), NS, "manual")
        .await
        .unwrap_or_else(|e| panic!("register agent {name}: {e}"));
}

/// Remove rows for an agent so the tests are repeatable.
async fn clean_agent(store: &ExternalDbPersistService, name: &str) {
    let db = store.db();
    ai_resource_version::Entity::delete_many()
        .filter(ai_resource_version::Column::Name.eq(name))
        .exec(db)
        .await
        .expect("clean versions");
    ai_resource::Entity::delete_many()
        .filter(ai_resource::Column::Name.eq(name))
        .exec(db)
        .await
        .expect("clean resources");
}

/// Post to an agent admin endpoint and return `(status, body)`.
async fn agent_post<S>(app: &S, path: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let req = test::TestRequest::post().uri(path).to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    (status, body)
}

/// The agent lifecycle endpoints drive the shared state machine over HTTP.
#[actix_web::test]
#[ignore]
async fn agent_admin_endpoints_walk_the_lifecycle() {
    let (store, _svc) = setup().await;
    let agents = Arc::new(A2aServerOperationService::new(store.clone()));
    register_agent(&agents, "route-agent").await;

    let app = build_app(store.clone(), _svc.clone()).await;

    let base = "/v3/admin/ai/agents";
    let name = "route-agent";
    let version = "1.0.0";

    // Registration publishes directly, so the version starts online.
    let (status, body) = agent_post(&app, &format!("{base}/offline?agentName={name}&version={version}")).await;
    assert_eq!(status, StatusCode::OK, "offline should succeed: {body}");
    assert!(body.contains("offline"), "must report the new status: {body}");

    let (status, body) = agent_post(&app, &format!("{base}/online?agentName={name}&version={version}")).await;
    assert_eq!(status, StatusCode::OK, "online should succeed: {body}");
    assert!(body.contains("online"), "must report the new status: {body}");

    let (status, body) = agent_post(&app, &format!("{base}/redraft?agentName={name}&version={version}")).await;
    assert_eq!(status, StatusCode::OK, "redraft should succeed: {body}");
    assert!(body.contains("draft"), "must report the new status: {body}");

    let (status, body) = agent_post(&app, &format!("{base}/submit?agentName={name}&version={version}")).await;
    assert_eq!(status, StatusCode::OK, "submit should succeed: {body}");
    assert!(body.contains("reviewing"), "must report the new status: {body}");

    let (status, body) = agent_post(&app, &format!("{base}/publish?agentName={name}&version={version}")).await;
    assert_eq!(status, StatusCode::OK, "publish should succeed: {body}");
    assert!(body.contains("online"), "must report the new status: {body}");

    clean_agent(&store, name).await;
    clean(&store).await;
}

/// Drafts are created, updated, submitted and deleted through the endpoints.
#[actix_web::test]
#[ignore]
async fn agent_admin_draft_endpoints_walk_the_flow() {
    use batata_ai::model::{AgentCapabilities, AgentCard, AgentSkill};

    let (store, _svc) = setup().await;
    let agents = Arc::new(A2aServerOperationService::new(store.clone()));
    register_agent(&agents, "draft-agent").await;

    let app = build_app(store.clone(), _svc.clone()).await;
    let base = "/v3/admin/ai/agents";
    let name = "draft-agent";

    // A draft is a new version on an agent that already exists.
    let mut card = AgentCard {
        name: name.to_string(),
        display_name: name.to_string(),
        description: "draft version".to_string(),
        version: "2.0.0".to_string(),
        url: "http://localhost:8080".to_string(),
        protocol_version: "1.0".to_string(),
        capabilities: AgentCapabilities {
            streaming: true,
            tool_use: true,
            ..Default::default()
        },
        skills: vec![AgentSkill {
            name: "coding".to_string(),
            description: "code generation".to_string(),
            proficiency: 90,
            examples: vec![],
        }],
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        preferred_transport: None,
        provider: None,
        documentation_url: None,
        icon_url: None,
        supports_authenticated_extended_card: None,
        metadata: Default::default(),
        tags: vec![],
    };

    let req = test::TestRequest::post()
        .uri(&format!("{base}/draft?namespaceId={NS}&overwrite=false"))
        .set_json(&card)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "create draft should succeed");
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(body.contains("draft"), "the new version must be a draft: {body}");

    // A second draft without overwrite must be refused: only one is edited.
    let req = test::TestRequest::post()
        .uri(&format!("{base}/draft?namespaceId={NS}&overwrite=false"))
        .set_json(&card)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() != StatusCode::OK,
        "a second draft must not silently replace the first"
    );

    // Updating the draft being edited works.
    card.description = "edited draft".to_string();
    let req = test::TestRequest::put()
        .uri(&format!("{base}/draft?namespaceId={NS}"))
        .set_json(&card)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "update draft should succeed");

    // The draft can then be submitted for review.
    let (status, body) =
        agent_post(&app, &format!("{base}/submit?agentName={name}&version=2.0.0")).await;
    assert_eq!(status, StatusCode::OK, "submit should succeed: {body}");
    assert!(body.contains("reviewing"), "must report the new status: {body}");

    // And the leftover draft of another version can be deleted.
    let req = test::TestRequest::delete()
        .uri(&format!("{base}/draft?agentName={name}&version=2.0.0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "delete draft should succeed");

    clean_agent(&store, name).await;
    clean(&store).await;
}

/// The read endpoints and create / update / delete work over HTTP.
#[actix_web::test]
#[ignore]
async fn agent_admin_crud_endpoints_work() {
    let (store, _svc) = setup().await;
    let app = build_app(store.clone(), _svc.clone()).await;
    let base = "/v3/admin/ai/agents";
    let name = "crud-agent";

    // ---- create -------------------------------------------------------------
    let card = serde_json::to_value(agent_card(name, "1.0.0")).expect("serialize card");
    let req = test::TestRequest::post()
        .uri(&format!("{base}?namespaceId={NS}"))
        .set_json(&card)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "create should succeed");
    println!("ok: create");

    // ---- detail -------------------------------------------------------------
    let req = test::TestRequest::get()
        .uri(&format!("{base}?agentName={name}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "detail should succeed");
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(body.contains(name), "detail must name the agent: {body}");

    // ---- versions -----------------------------------------------------------
    let req = test::TestRequest::get()
        .uri(&format!("{base}/versions?agentName={name}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "versions should succeed");

    // ---- list ---------------------------------------------------------------
    let req = test::TestRequest::get()
        .uri(&format!("{base}/list?pageNo=1&pageSize=20"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "list should succeed");
    println!("ok: read endpoints");

    // ---- update -------------------------------------------------------------
    let mut updated = agent_card(name, "1.0.0");
    updated.description = "updated description".to_string();
    let req = test::TestRequest::put()
        .uri(&format!("{base}?namespaceId={NS}"))
        .set_json(&updated)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "update should succeed");

    // ---- delete -------------------------------------------------------------
    let req = test::TestRequest::delete()
        .uri(&format!("{base}?agentName={name}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "delete should succeed");

    // The agent is really gone.
    let req = test::TestRequest::get()
        .uri(&format!("{base}?agentName={name}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "agent must be gone");
    println!("ok: delete removed the agent");

    clean_agent(&store, name).await;
    clean(&store).await;
}

/// Missing parameters are rejected rather than silently ignored.
#[actix_web::test]
#[ignore]
async fn agent_admin_endpoints_require_the_version() {
    let (store, _svc) = setup().await;
    let app = build_app(store.clone(), _svc.clone()).await;

    let (status, _body) = agent_post(&app, "/v3/admin/ai/agents/offline?agentName=route-agent").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "version is required");

    clean(&store).await;
}

/// Post a JSON body to `/ai/mcp/import/execute` and return the response body.
async fn execute_payload<S>(app: &S, body: serde_json::Value) -> String
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let req = test::TestRequest::post()
        .uri("/ai/mcp/import/execute")
        .set_json(body)
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "execute should answer 200");
    String::from_utf8_lossy(&test::read_body(resp).await).to_string()
}

/// A one-server import payload for `name`.
///
/// Each import test uses its own name: `clean` only removes `NAME`, so a
/// shared name would leak between tests and make the next run see an
/// already-existing server.
fn import_content(name: &str) -> String {
    format!(
        r#"[{{"name":"{name}","protocol":"http","description":"imported server","version":"1.0.0","remoteServerConfig":{{"url":"http://localhost:9099"}}}}]"#
    )
}

/// Remove one server by name, including its versions.
async fn clean_named(store: &ExternalDbPersistService, name: &str) {
    let db = store.db();
    ai_resource_version::Entity::delete_many()
        .filter(ai_resource_version::Column::Name.eq(name))
        .exec(db)
        .await
        .expect("clean versions");
    ai_resource::Entity::delete_many()
        .filter(ai_resource::Column::Name.eq(name))
        .exec(db)
        .await
        .expect("clean resources");
}

/// A valid batch is actually imported: the server must exist afterwards.
#[actix_web::test]
#[ignore]
async fn import_execute_creates_the_server() {
    let (store, svc) = setup().await;
    let app = build_app(store.clone(), svc.clone()).await;

    let body = execute_payload(
        &app,
        serde_json::json!({ "content": import_content("imported-create"), "namespace": NS }),
    )
    .await;
    assert!(body.contains("\"success\":true"), "import must succeed: {body}");
    assert!(body.contains("\"successCount\":1"), "one imported: {body}");

    // The point of the endpoint: the server is now really there.
    let detail = svc
        .get_mcp_server_detail(NS, None, Some("imported-create"), Some("1.0.0"), None)
        .await
        .expect("detail lookup");
    assert!(detail.is_some(), "the imported server must exist");

    clean_named(&store, "imported-create").await;
    clean(&store).await;
}

/// Without `overwrite`, an existing server is skipped and flagged, not clobbered.
#[actix_web::test]
#[ignore]
async fn import_execute_skips_an_existing_server() {
    let (store, svc) = setup().await;
    let mut registration = registration("1.0.0", "echo");
    registration.name = "imported-skip".to_string();
    svc.create_mcp_server(NS, &registration)
        .await
        .expect("create server");

    let app = build_app(store.clone(), svc.clone()).await;
    let body = execute_payload(
        &app,
        serde_json::json!({ "content": import_content("imported-skip"), "namespace": NS }),
    )
    .await;
    assert!(body.contains("\"skippedCount\":1"), "must skip: {body}");
    assert!(body.contains("existing"), "must report the conflict: {body}");
    // A skip is not a failure.
    assert!(body.contains("\"success\":true"), "skips do not fail: {body}");

    clean_named(&store, "imported-skip").await;
    clean(&store).await;
}

/// An invalid batch is refused outright unless the caller opts into skipping.
#[actix_web::test]
#[ignore]
async fn import_execute_refuses_an_invalid_batch() {
    let (store, svc) = setup().await;
    let app = build_app(store.clone(), svc.clone()).await;

    let body = execute_payload(
        &app,
        serde_json::json!({ "content": r#"[{"name":"broken"}]"#, "namespace": NS }),
    )
    .await;
    assert!(body.contains("\"success\":false"), "must be refused: {body}");
    assert!(
        body.contains("Import validation failed"),
        "must explain why: {body}"
    );

    clean(&store).await;
}

/// A server that already exists is imported as an update.
#[actix_web::test]
#[ignore]
async fn import_execute_updates_when_overwrite_is_requested() {
    let (store, svc) = setup().await;
    let app = build_app(store.clone(), svc.clone()).await;

    let content = import_content("imported-update");
    let body = execute_payload(
        &app,
        serde_json::json!({ "content": content, "namespace": NS }),
    )
    .await;
    assert!(body.contains("\"successCount\":1"), "first import: {body}");

    let body = execute_payload(
        &app,
        serde_json::json!({ "content": content, "namespace": NS, "overwrite": true }),
    )
    .await;
    assert!(body.contains("\"successCount\":1"), "second import updates: {body}");
    assert!(
        !body.contains("\"skippedCount\":1"),
        "overwrite must not skip: {body}"
    );

    clean_named(&store, "imported-update").await;
    clean(&store).await;
}

/// A server that already exists in the namespace is reported as a duplicate
/// rather than silently accepted.
#[actix_web::test]
#[ignore]
async fn import_validate_flags_an_existing_server_as_duplicate() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration("1.0.0", "echo"))
        .await
        .expect("create server");

    let app = build_app(store.clone(), svc.clone()).await;
    let content = r#"[{"name":"route-mcp","protocol":"http","description":"d","version":"1.0.0","remoteServerConfig":{}}]"#;
    let body = validate_payload(&app, content).await;
    assert!(
        body.contains("Server already exists"),
        "must report the existing server: {body}"
    );
    assert!(body.contains("\"duplicateCount\":1"), "one duplicate: {body}");

    clean(&store).await;
}
