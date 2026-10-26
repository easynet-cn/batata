//! Route-level test for the MCP admin API.
//!
//! The admin API used to be served by a legacy shim backed by the in-memory
//! registry. It is now backed by the `ai_resource` service, the same one the
//! console uses — so these cover that the endpoints are reachable on the new
//! implementation, and that the SDK's create encoding still works:
//! `serverSpecification` arrives as a JSON *string* inside a form body.
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run the same cases
//! against a real database. See `common/ai_store.rs`.

#[path = "common/ai_store.rs"]
mod ai_store;

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_ai::{
    McpServerIndex, McpServerOperationService,
    model::{McpCapability, McpServerRegistration, McpServerType, McpTransport},
};
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NS: &str = "public";
const NAME: &str = "admin-route-mcp";
/// The version created first, which the lifecycle tests leave alone.
const BASE_VERSION: &str = "1.0.0";
/// The version the lifecycle tests move through draft → submit → publish.
const DRAFT_VERSION: &str = "2.0.0";

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

/// Build an app exposing the real MCP admin routes.
async fn build_app(
    store: Arc<dyn PersistenceService>,
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
    let resp = test::call_service(app, test::TestRequest::get().uri(uri).to_request()).await;
    let status = resp.status();
    (
        status,
        String::from_utf8_lossy(&test::read_body(resp).await).to_string(),
    )
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
async fn create_accepts_the_sdk_form_encoding() {
    let store = TestStore::open().await;
    let svc = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));

    store.clean(NAME).await;
    let app = build_app(store.store(), svc.clone()).await;

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

    store.clean(NAME).await;
}

/// A malformed specification is rejected rather than silently creating junk.
#[actix_web::test]
async fn create_rejects_a_malformed_specification() {
    let store = TestStore::open().await;
    let svc = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));

    store.clean(NAME).await;
    let app = build_app(store.store(), svc.clone()).await;

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

    store.clean(NAME).await;
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
    (
        status,
        String::from_utf8_lossy(&test::read_body(resp).await).to_string(),
    )
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
    (
        status,
        String::from_utf8_lossy(&test::read_body(resp).await).to_string(),
    )
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
    (
        status,
        String::from_utf8_lossy(&test::read_body(resp).await).to_string(),
    )
}

/// The resource-level status (`enable` / `disable`).
async fn resource_status(store: &TestStore) -> Option<String> {
    store
        .store()
        .ai_resource_find(NS, NAME, "mcp")
        .await
        .expect("resource lookup")
        .and_then(|row| row.status)
}

/// The status a version carries.
async fn version_status(store: &TestStore, version: &str) -> Option<String> {
    store
        .store()
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

    // The maintainer client posts the draft as a form: the specification is a
    // JSON string, and the version travels beside it.
    let mut next = registration();
    next.version = DRAFT_VERSION.to_string();
    let spec = serde_json::to_string(&next).expect("serialize");
    let (status, out) = post_form(
        app,
        "/v3/admin/ai/mcp/draft",
        &format!(
            "serverSpecification={}&mcpName={}&version={}&namespaceId={}",
            percent_encode(&spec),
            NAME,
            DRAFT_VERSION,
            NS
        ),
    )
    .await;
    if status != StatusCode::OK {
        return Err(format!("create draft failed ({status}): {out}"));
    }
    Ok(())
}

/// A draft walks submit → publish, and only then is it online.
#[actix_web::test]
async fn draft_moves_through_submit_and_publish() {
    let store = TestStore::open().await;
    let svc = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    store.clean(NAME).await;
    let app = build_app(store.store(), svc.clone()).await;

    with_draft(&app).await.expect("draft");
    assert_eq!(
        version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("draft")
    );

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

    store.clean(NAME).await;
}

/// force-publish exists to skip the review gate, so publishing a draft the
/// normal way must fail while force-publish succeeds.
#[actix_web::test]
async fn force_publish_bypasses_review() {
    let store = TestStore::open().await;
    let svc = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    store.clean(NAME).await;
    let app = build_app(store.store(), svc.clone()).await;

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

    store.clean(NAME).await;
}

/// redraft sends a published version back for editing.
#[actix_web::test]
async fn redraft_returns_a_version_to_draft() {
    let store = TestStore::open().await;
    let svc = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    store.clean(NAME).await;
    let app = build_app(store.store(), svc.clone()).await;

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
    assert_eq!(
        version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("draft")
    );
    println!("ok: redraft returns a version to draft");

    store.clean(NAME).await;
}

/// Labels, status and scope are the three resource-level knobs; each endpoint
/// must actually take effect.
#[actix_web::test]
async fn labels_status_and_scope_take_effect() {
    let store = TestStore::open().await;
    let svc = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    store.clean(NAME).await;
    let app = build_app(store.store(), svc.clone()).await;

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
    // this points at the one the server was created with. Upstream sends the
    // map as a JSON string in the `labels` parameter.
    let labels = format!(r#"{{"stable":"{BASE_VERSION}"}}"#);
    let (status, out) = put_json(
        &app,
        &format!(
            "/v3/admin/ai/mcp/labels?{qs}&labels={}",
            percent_encode(&labels)
        ),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "update labels failed: {out}");
    assert!(out.contains("stable"), "the label must come back: {out}");

    let (status, out) = put_json(
        &app,
        &format!("/v3/admin/ai/mcp/scope?{qs}&scope=PRIVATE"),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "update scope failed: {out}");

    // `enabled=false` is the parameter the maintainer client sends; reading a
    // `status` string instead used to turn this into an enable.
    let (status, out) = put_json(
        &app,
        &format!("/v3/admin/ai/mcp/status?{qs}&enabled=false"),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "update status failed: {out}");
    assert_eq!(
        resource_status(&store).await.as_deref(),
        Some("disable"),
        "enabled=false must disable the resource"
    );
    println!("ok: labels, status and scope take effect");

    store.clean(NAME).await;
}

/// The version endpoints expose what the lifecycle produced.
#[actix_web::test]
async fn version_endpoints_report_the_lifecycle() {
    let store = TestStore::open().await;
    let svc = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    store.clean(NAME).await;
    let app = build_app(store.store(), svc.clone()).await;

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

    store.clean(NAME).await;
}
