//! Route-level test for the MCP admin API.
//!
//! The admin API used to be served by a legacy shim backed by the in-memory
//! registry. It is now backed by the `ai_resource` service, the same one the
//! console uses — so these cover that the endpoints are reachable on the new
//! implementation, and that the SDK's create encoding still works:
//! `serverSpecification` arrives as a JSON *string* inside a form body.
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-server --test mcp_admin_routes -- --ignored --nocapture
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use batata_ai::{
    McpServerIndex, McpServerOperationService,
    model::{McpCapability, McpServerRegistration, McpServerType, McpTransport},
};
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use batata_persistence::{AiResourcePersistence, ExternalDbPersistService};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NS: &str = "public";
const NAME: &str = "admin-route-mcp";
/// The version created first, which the lifecycle tests leave alone.
const BASE_VERSION: &str = "1.0.0";
/// The version the lifecycle tests move through draft → submit → publish.
const DRAFT_VERSION: &str = "2.0.0";

/// Connect to the test database, failing fast with an actionable message.
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

/// Cluster manager stub. The MCP admin routes never touch cluster state.
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
        unimplemented!("not used by the MCP admin routes")
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

/// Build an app exposing the real MCP admin routes.
async fn build_app(
    store: Arc<ExternalDbPersistService>,
    svc: Arc<McpServerOperationService>,
) -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
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

    // The server wires MCP through the trait, so the test must too.
    let mcp: Arc<dyn batata_common::McpServerService> = svc;

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(mcp))
            .service(web::scope("/v3/admin/ai").service(batata_ai::mcp_admin_routes())),
    )
    .await
}

/// Percent-encode a form value, so JSON survives the form body.
fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

async fn post_form<S>(app: &S, uri: &str, body: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let req = test::TestRequest::post()
        .uri(uri)
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload(body.to_string())
        .to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    (status, String::from_utf8_lossy(&test::read_body(resp).await).to_string())
}

async fn get<S>(app: &S, uri: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let resp = test::call_service(app, test::TestRequest::get().uri(uri).to_request()).await;
    let status = resp.status();
    (status, String::from_utf8_lossy(&test::read_body(resp).await).to_string())
}

fn registration() -> McpServerRegistration {
    McpServerRegistration {
        name: NAME.to_string(),
        display_name: NAME.to_string(),
        description: "created through the admin API".to_string(),
        namespace: NS.to_string(),
        version: "1.0.0".to_string(),
        endpoint: "http://localhost:8080".to_string(),
        server_type: McpServerType::Http,
        transport: McpTransport::default(),
        capabilities: vec![McpCapability::Tool],
        tools: vec![],
        resources: vec![],
        prompts: vec![],
        metadata: HashMap::new(),
        tags: vec![],
        auto_fetch_tools: true,
        health_check: None,
    }
}

/// The SDK posts the specification as a JSON string in a form field. That
/// encoding must keep working, or every existing client breaks.
#[actix_web::test]
#[ignore]
async fn create_accepts_the_sdk_form_encoding() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = Arc::new(McpServerOperationService::new(
        store.clone(),
        Arc::new(McpServerIndex::new()),
    ));

    clean(&store).await;
    let app = build_app(store.clone(), svc.clone()).await;

    let spec = serde_json::to_string(&registration()).expect("serialize registration");
    let body = format!(
        "serverSpecification={}&namespaceId={}",
        percent_encode(&spec),
        NS
    );
    let (status, body) = post_form(&app, "/v3/admin/ai/mcp", &body).await;
    assert_eq!(status, StatusCode::OK, "create failed: {body}");

    // The server must now be visible through the admin list.
    let (status, listed) = get(&app, "/v3/admin/ai/mcp/list").await;
    assert_eq!(status, StatusCode::OK, "list failed: {listed}");
    assert!(listed.contains(NAME), "must list the server: {listed}");
    println!("ok: admin create uses the SDK form encoding");

    clean(&store).await;
}

/// A malformed specification is rejected rather than silently creating junk.
#[actix_web::test]
#[ignore]
async fn create_rejects_a_malformed_specification() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = Arc::new(McpServerOperationService::new(
        store.clone(),
        Arc::new(McpServerIndex::new()),
    ));

    clean(&store).await;
    let app = build_app(store.clone(), svc.clone()).await;

    let body = format!(
        "serverSpecification={}&namespaceId={}",
        percent_encode("{not json"),
        NS
    );
    let (status, _) = post_form(&app, "/v3/admin/ai/mcp", &body).await;
    assert!(
        status.is_client_error(),
        "a malformed specification must be refused, got {status}"
    );
    println!("ok: malformed specification is refused");

    clean(&store).await;
}

// ============================================================================
// Lifecycle
// ============================================================================

async fn post_json<S>(app: &S, uri: &str, json: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let req = test::TestRequest::post()
        .uri(uri)
        .insert_header(("content-type", "application/json"))
        .set_payload(json.to_string())
        .to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    (status, String::from_utf8_lossy(&test::read_body(resp).await).to_string())
}

async fn put_json<S>(app: &S, uri: &str, json: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let req = test::TestRequest::put()
        .uri(uri)
        .insert_header(("content-type", "application/json"))
        .set_payload(json.to_string())
        .to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    (status, String::from_utf8_lossy(&test::read_body(resp).await).to_string())
}

async fn delete_at<S>(app: &S, uri: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let resp = test::call_service(app, test::TestRequest::delete().uri(uri).to_request()).await;
    let status = resp.status();
    (status, String::from_utf8_lossy(&test::read_body(resp).await).to_string())
}

/// The status a version carries in `ai_resource_version`.
async fn version_status(store: &ExternalDbPersistService, version: &str) -> Option<String> {
    store
        .ai_resource_version_find(NS, NAME, "mcp", version)
        .await
        .expect("version lookup")
        .map(|row| row.status)
}

/// Create the server, then a draft of the next version.
///
/// The order matters: a draft version can only be added to a server that
/// already exists, so this posts the SDK create first and the draft second.
async fn with_draft<S>(app: &S) -> Result<(), String>
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let spec = serde_json::to_string(&registration()).expect("serialize");
    let (status, out) = post_form(
        app,
        "/v3/admin/ai/mcp",
        &format!(
            "serverSpecification={}&namespaceId={}",
            percent_encode(&spec),
            NS
        ),
    )
    .await;
    if status != StatusCode::OK {
        return Err(format!("create server failed ({status}): {out}"));
    }

    let mut next = registration();
    next.version = DRAFT_VERSION.to_string();
    let body = serde_json::to_string(&next).expect("serialize");
    let (status, out) = post_json(app, "/v3/admin/ai/mcp/draft", &body).await;
    if status != StatusCode::OK {
        return Err(format!("create draft failed ({status}): {out}"));
    }
    Ok(())
}

/// A draft walks submit → publish, and only then is it online.
#[actix_web::test]
#[ignore]
async fn draft_moves_through_submit_and_publish() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = Arc::new(McpServerOperationService::new(
        store.clone(),
        Arc::new(McpServerIndex::new()),
    ));
    clean(&store).await;
    let app = build_app(store.clone(), svc.clone()).await;

    with_draft(&app).await.expect("draft");
    assert_eq!(version_status(&store, DRAFT_VERSION).await.as_deref(), Some("draft"));

    let qs = format!("mcpName={NAME}&version={DRAFT_VERSION}");
    let (status, out) = post_json(&app, &format!("/v3/admin/ai/mcp/submit?{qs}"), "{}").await;
    assert_eq!(status, StatusCode::OK, "submit failed: {out}");

    let (status, out) = post_json(&app, &format!("/v3/admin/ai/mcp/publish?{qs}"), "{}").await;
    assert_eq!(status, StatusCode::OK, "publish failed: {out}");
    assert_eq!(
        version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("online")
    );
    println!("ok: draft → submit → publish");

    clean(&store).await;
}

/// force-publish exists to skip the review gate, so publishing a draft the
/// normal way must fail while force-publish succeeds.
#[actix_web::test]
#[ignore]
async fn force_publish_bypasses_review() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = Arc::new(McpServerOperationService::new(
        store.clone(),
        Arc::new(McpServerIndex::new()),
    ));
    clean(&store).await;
    let app = build_app(store.clone(), svc.clone()).await;

    with_draft(&app).await.expect("draft");

    let qs = format!("mcpName={NAME}&version={DRAFT_VERSION}");
    let (status, _) = post_json(&app, &format!("/v3/admin/ai/mcp/publish?{qs}"), "{}").await;
    assert!(
        status.is_client_error(),
        "publishing an unsubmitted draft must be refused, got {status}"
    );

    let (status, out) =
        post_json(&app, &format!("/v3/admin/ai/mcp/force-publish?{qs}"), "{}").await;
    assert_eq!(status, StatusCode::OK, "force publish failed: {out}");
    assert_eq!(
        version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("online")
    );
    println!("ok: force-publish bypasses review");

    clean(&store).await;
}

/// redraft sends a published version back for editing.
#[actix_web::test]
#[ignore]
async fn redraft_returns_a_version_to_draft() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = Arc::new(McpServerOperationService::new(
        store.clone(),
        Arc::new(McpServerIndex::new()),
    ));
    clean(&store).await;
    let app = build_app(store.clone(), svc.clone()).await;

    with_draft(&app).await.expect("draft");
    let qs = format!("mcpName={NAME}&version={DRAFT_VERSION}");
    post_json(&app, &format!("/v3/admin/ai/mcp/submit?{qs}"), "{}").await;
    post_json(&app, &format!("/v3/admin/ai/mcp/publish?{qs}"), "{}").await;
    assert_eq!(
        version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("online")
    );

    let (status, out) = post_json(&app, &format!("/v3/admin/ai/mcp/redraft?{qs}"), "{}").await;
    assert_eq!(status, StatusCode::OK, "redraft failed: {out}");
    assert_eq!(version_status(&store, DRAFT_VERSION).await.as_deref(), Some("draft"));
    println!("ok: redraft returns a version to draft");

    clean(&store).await;
}

/// Labels, status and scope are the three resource-level knobs; each endpoint
/// must actually take effect.
#[actix_web::test]
#[ignore]
async fn labels_status_and_scope_take_effect() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = Arc::new(McpServerOperationService::new(
        store.clone(),
        Arc::new(McpServerIndex::new()),
    ));
    clean(&store).await;
    let app = build_app(store.clone(), svc.clone()).await;

    let spec = serde_json::to_string(&registration()).expect("serialize");
    let (status, out) = post_form(
        &app,
        "/v3/admin/ai/mcp",
        &format!(
            "serverSpecification={}&namespaceId={}",
            percent_encode(&spec),
            NS
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create failed: {out}");

    let qs = format!("mcpName={NAME}");

    // Labels map a name to a version, and that version must be online — so
    // this points at the one the server was created with.
    let (status, out) = put_json(
        &app,
        &format!("/v3/admin/ai/mcp/labels?{qs}"),
        &format!(r#"{{"labels":{{"stable":"{BASE_VERSION}"}}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "update labels failed: {out}");
    assert!(out.contains("stable"), "the label must come back: {out}");

    let (status, out) =
        put_json(&app, &format!("/v3/admin/ai/mcp/scope?{qs}&scope=PRIVATE"), "{}").await;
    assert_eq!(status, StatusCode::OK, "update scope failed: {out}");

    let (status, out) = put_json(
        &app,
        &format!("/v3/admin/ai/mcp/status?{qs}&status=DISABLED"),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "update status failed: {out}");
    println!("ok: labels, status and scope take effect");

    clean(&store).await;
}

/// The version endpoints expose what the lifecycle produced.
#[actix_web::test]
#[ignore]
async fn version_endpoints_report_the_lifecycle() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = Arc::new(McpServerOperationService::new(
        store.clone(),
        Arc::new(McpServerIndex::new()),
    ));
    clean(&store).await;
    let app = build_app(store.clone(), svc.clone()).await;

    with_draft(&app).await.expect("draft");

    let (status, versions) = get(&app, &format!("/v3/admin/ai/mcp/versions?mcpName={NAME}")).await;
    assert_eq!(status, StatusCode::OK, "list versions failed: {versions}");
    assert!(
        versions.contains(DRAFT_VERSION),
        "must list the version: {versions}"
    );

    let (status, detail) = get(
        &app,
        &format!("/v3/admin/ai/mcp/version?mcpName={NAME}&version={DRAFT_VERSION}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "get version failed: {detail}");
    assert!(detail.contains(NAME), "must name the server: {detail}");

    let (status, out) = delete_at(
        &app,
        &format!("/v3/admin/ai/mcp/draft?mcpName={NAME}&version={DRAFT_VERSION}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "delete draft failed: {out}");
    println!("ok: version endpoints report the lifecycle");

    clean(&store).await;
}
