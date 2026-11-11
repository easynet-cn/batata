//! AI ADMIN write routes must accept the Nacos 3.3.0 GA parameter encoding.
//!
//! GA `nacos-maintainer-client-3.3.0.jar` calls `ClientHttpProxy.postForm` /
//! `putForm` for POST/PUT and fills a `setParamValue` map, so **every**
//! `namespaceId` / `mcpName` / `agentName` / `version` / `scope` / `labels`
//! arrives in an `application/x-www-form-urlencoded` body. Batata's handlers
//! used to extract `web::Query<T>` only, which produced an empty form object
//! and a structured 400.
//!
//! The strongest guard here is equivalence: each endpoint is called once with
//! the parameters in a form body and once in the query string, and both must
//! return the same status and body. Real success paths (MCP lifecycle, status,
//! scope, labels and agent lifecycle) prove the equivalence is not merely two
//! identical failures.
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run the same cases
//! against a real database. See `common/ai_store.rs`.

#[path = "common/ai_store.rs"]
#[allow(dead_code)]
mod ai_store;

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_ai::{
    A2aServerOperationService, McpServerIndex, McpServerOperationService,
    model::{McpCapability, McpServerRegistration, McpServerType, McpTransport},
};
use batata_common::model::ai::a2a::AgentCard;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;
use serde_json::{Value, json};

const NS: &str = "public";
const MCP_NAME: &str = "ga-form-mcp";
const AGENT_NAME: &str = "ga-form-agent";
const BASE_VERSION: &str = "1.0.0";
const DRAFT_VERSION: &str = "2.0.0";
/// A version that never exists, so form/query equivalence can be asserted
/// without the first call mutating the state the second call observes.
const MISSING_VERSION: &str = "9.9.9";
/// Agents created through the GA flat admin form.
const FLAT_AGENT: &str = "ga-flat-agent";
const FLAT_VERSION: &str = "3.0.0";
const JSON_AGENT: &str = "ga-json-agent";
const JSON_VERSION: &str = "4.0.0";
/// A first draft created without any pre-existing Agent metadata.
const FIRST_AGENT: &str = "ga-first-draft-agent";
const FIRST_JSON_AGENT: &str = "ga-first-json-agent";
const FIRST_VERSION: &str = "5.0.0";

/// Cluster manager stub. The AI admin routes never touch cluster state.
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
        unimplemented!("not used by the AI admin routes")
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

/// Build an app exposing the real MCP **and** agent admin routes, exactly as
/// `crates/batata-server/src/api/v3/admin/ai/route.rs` mounts them.
async fn build_app(
    store: Arc<dyn PersistenceService>,
    mcp: Arc<McpServerOperationService>,
    agent: Arc<A2aServerOperationService>,
) -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    let mut configuration = Configuration::default();
    configuration.typed.core.auth.console.enabled = false;
    configuration.typed.core.auth.admin.enabled = false;

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

    let mcp_trait: Arc<dyn batata_common::McpServerService> = mcp;
    let agent_trait: Arc<dyn batata_common::A2aAgentService> = agent;

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(mcp_trait))
            .app_data(web::Data::new(agent_trait))
            .service(
                web::scope("/v3/admin/ai")
                    .service(batata_ai::mcp_admin_routes())
                    .service(batata_ai::api::agent::admin_routes()),
            ),
    )
    .await
}

/// Percent-encode a form value so JSON survives the body.
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

/// One request with an explicit method, content type and body.
async fn call<S>(
    app: &S,
    method: &str,
    uri: &str,
    content_type: &str,
    body: &str,
) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let builder = match method {
        "PUT" => test::TestRequest::put(),
        _ => test::TestRequest::post(),
    };
    let req = builder
        .uri(uri)
        .insert_header(("content-type", content_type))
        .set_payload(body.to_string())
        .to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    (
        status,
        String::from_utf8_lossy(&test::read_body(resp).await).to_string(),
    )
}

/// The GA encoding: all parameters in a urlencoded body, no query string.
async fn post_form<S>(app: &S, uri: &str, body: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    call(app, "POST", uri, "application/x-www-form-urlencoded", body).await
}

async fn put_form<S>(app: &S, uri: &str, body: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    call(app, "PUT", uri, "application/x-www-form-urlencoded", body).await
}

/// Batata's own SDK encoding: parameters in the query string, empty JSON body.
async fn post_query<S>(app: &S, uri: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    call(app, "POST", uri, "application/json", "{}").await
}

async fn put_query<S>(app: &S, uri: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    call(app, "PUT", uri, "application/json", "{}").await
}

/// Assert that form-body and query-string encodings are indistinguishable.
fn assert_same(label: &str, form: &(StatusCode, String), query: &(StatusCode, String)) {
    assert_eq!(
        form.0, query.0,
        "{label}: status differs (form={:?}, query={:?})",
        form.1, query.1
    );
    assert_eq!(
        form.1, query.1,
        "{label}: body differs (form status={:?}, query status={:?})",
        form.0, query.0
    );
}

fn mcp_registration(version: &str) -> McpServerRegistration {
    McpServerRegistration {
        name: MCP_NAME.to_string(),
        display_name: MCP_NAME.to_string(),
        description: "GA form encoding fixture".to_string(),
        namespace: NS.to_string(),
        version: version.to_string(),
        endpoint: "http://localhost:8080".to_string(),
        server_type: McpServerType::Http,
        transport: McpTransport::default(),
        capabilities: vec![McpCapability::Tool],
        tools: vec![],
        resources: vec![],
        prompts: vec![],
        metadata: Default::default(),
        tags: vec![],
        auto_fetch_tools: true,
        health_check: None,
    }
}

/// Create the server and a draft version, both using the SDK form encoding.
async fn with_mcp_draft<S>(app: &S) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let spec = serde_json::to_string(&mcp_registration(BASE_VERSION)).expect("serialize");
    let (status, out) = post_form(
        app,
        "/v3/admin/ai/mcp",
        &format!(
            "serverSpecification={}&namespaceId={NS}",
            percent_encode(&spec)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create server failed: {out}");

    let spec = serde_json::to_string(&mcp_registration(DRAFT_VERSION)).expect("serialize");
    let (status, out) = post_form(
        app,
        "/v3/admin/ai/mcp/draft",
        &format!(
            "serverSpecification={}&namespaceId={NS}&mcpName={MCP_NAME}&version={DRAFT_VERSION}",
            percent_encode(&spec)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {out}");
    (status, out)
}

fn agent_card_named(name: &str, version: &str) -> AgentCard {
    AgentCard {
        name: name.to_string(),
        display_name: name.to_string(),
        version: version.to_string(),
        ..Default::default()
    }
}

fn agent_card(version: &str) -> AgentCard {
    agent_card_named(AGENT_NAME, version)
}

/// Serialized `AgentCardDetailInfo` of one stored agent version.
async fn stored_agent_card(store: &TestStore, name: &str, version: &str) -> Option<String> {
    store
        .store()
        .ai_resource_version_find(NS, name, "agent", version)
        .await
        .expect("version lookup")
        .and_then(|row| row.storage)
}

async fn agent_version_status_for(store: &TestStore, name: &str, version: &str) -> Option<String> {
    store
        .store()
        .ai_resource_version_find(NS, name, "agent", version)
        .await
        .expect("version lookup")
        .map(|row| row.status)
}

async fn mcp_version_status(store: &TestStore, version: &str) -> Option<String> {
    store
        .store()
        .ai_resource_version_find(NS, MCP_NAME, "mcp", version)
        .await
        .expect("version lookup")
        .map(|row| row.status)
}

async fn agent_version_status(store: &TestStore, version: &str) -> Option<String> {
    store
        .store()
        .ai_resource_version_find(NS, AGENT_NAME, "agent", version)
        .await
        .expect("version lookup")
        .map(|row| row.status)
}

/// The GA form body must drive the real MCP lifecycle, not just avoid a 400.
#[actix_web::test]
async fn mcp_lifecycle_accepts_ga_form_body() {
    let store = TestStore::open().await;
    let mcp = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    let agent = Arc::new(A2aServerOperationService::new(store.store()));
    let app = build_app(store.store(), mcp, agent).await;

    with_mcp_draft(&app).await;
    assert_eq!(
        mcp_version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("draft")
    );

    let form_body = format!("namespaceId={NS}&mcpName={MCP_NAME}&version={DRAFT_VERSION}");
    let query = format!("?{form_body}");

    // submit: draft -> reviewing
    let (status, out) = post_form(&app, "/v3/admin/ai/mcp/submit", &form_body).await;
    assert_eq!(status, StatusCode::OK, "GA form submit failed: {out}");
    assert_eq!(
        mcp_version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("reviewing")
    );

    // publish: reviewing -> online (query encoding, the regression guard)
    let (status, out) = post_query(&app, &format!("/v3/admin/ai/mcp/publish{query}")).await;
    assert_eq!(status, StatusCode::OK, "query publish failed: {out}");
    assert_eq!(
        mcp_version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("online")
    );

    // The remaining transitions cannot all succeed from one state. Use a
    // version that never exists so neither call mutates state: a working dual
    // channel returns the same "version not found" domain error twice, while
    // a broken form channel returns "mcpName and version are required" 400.
    let missing_body = format!("namespaceId={NS}&mcpName={MCP_NAME}&version={MISSING_VERSION}");
    let missing_query = format!("?{missing_body}");
    for path in [
        "/v3/admin/ai/mcp/force-publish",
        "/v3/admin/ai/mcp/redraft",
        "/v3/admin/ai/mcp/online",
        "/v3/admin/ai/mcp/offline",
        "/v3/admin/ai/mcp/submit",
        "/v3/admin/ai/mcp/publish",
    ] {
        let form = post_form(&app, path, &missing_body).await;
        let via_query = post_query(&app, &format!("{path}{missing_query}")).await;
        assert_same(&format!("MCP {path}"), &form, &via_query);
        assert!(
            !form.1.contains("required"),
            "{path}: form parameters were dropped: {}",
            form.1
        );
    }
}

/// `enabled`, `scope` and `labels` all travel in the GA form body.
#[actix_web::test]
async fn mcp_status_scope_and_labels_accept_ga_form_body() {
    let store = TestStore::open().await;
    let mcp = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    let agent = Arc::new(A2aServerOperationService::new(store.store()));
    let app = build_app(store.store(), mcp, agent).await;
    with_mcp_draft(&app).await;

    let labels = format!(r#"{{"stable":"{BASE_VERSION}"}}"#);
    let (status, out) = put_form(
        &app,
        "/v3/admin/ai/mcp/labels",
        &format!(
            "namespaceId={NS}&mcpName={MCP_NAME}&labels={}",
            percent_encode(&labels)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GA form labels failed: {out}");
    assert!(out.contains("stable"), "the label must come back: {out}");

    let (status, out) = put_form(
        &app,
        "/v3/admin/ai/mcp/scope",
        &format!("namespaceId={NS}&mcpName={MCP_NAME}&scope=PRIVATE"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GA form scope failed: {out}");

    let (status, out) = put_form(
        &app,
        "/v3/admin/ai/mcp/status",
        &format!("namespaceId={NS}&mcpName={MCP_NAME}&enabled=false"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GA form status failed: {out}");
    let resource_status = store
        .store()
        .ai_resource_find(NS, MCP_NAME, "mcp")
        .await
        .expect("resource lookup")
        .and_then(|row| row.status);
    assert_eq!(resource_status.as_deref(), Some("disable"));

    // Query and JSON-body encodings must stay byte-identical to the form ones.
    let query = format!("?namespaceId={NS}&mcpName={MCP_NAME}&scope=PRIVATE");
    let form = put_form(
        &app,
        "/v3/admin/ai/mcp/scope",
        &format!("namespaceId={NS}&mcpName={MCP_NAME}&scope=PRIVATE"),
    )
    .await;
    let via_query = put_query(&app, &format!("/v3/admin/ai/mcp/scope{query}")).await;
    assert_same("MCP scope", &form, &via_query);
}

/// A JSON object body is another supported encoding (used by the console).
#[actix_web::test]
async fn json_object_body_is_equivalent_to_the_form_body() {
    let store = TestStore::open().await;
    let mcp = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    let agent = Arc::new(A2aServerOperationService::new(store.store()));
    let app = build_app(store.store(), mcp, agent).await;
    with_mcp_draft(&app).await;

    let payload = json!({
        "namespaceId": NS,
        "mcpName": MCP_NAME,
        "version": MISSING_VERSION,
    })
    .to_string();
    let via_json = call(
        &app,
        "POST",
        "/v3/admin/ai/mcp/submit",
        "application/json",
        &payload,
    )
    .await;
    let via_form = post_form(
        &app,
        "/v3/admin/ai/mcp/submit",
        &format!("namespaceId={NS}&mcpName={MCP_NAME}&version={MISSING_VERSION}"),
    )
    .await;
    // A missing version makes both calls deterministic domain errors; if
    // either channel were dropped the message would be "required" instead.
    assert_same("MCP submit JSON vs form", &via_form, &via_json);
    assert!(
        !via_json.1.contains("required"),
        "JSON object parameters were dropped: {}",
        via_json.1
    );
}

/// The query string wins when both encodings carry the same key, matching
/// `batata-naming`'s `merge_form` precedence.
#[actix_web::test]
async fn query_wins_over_the_form_body() {
    let store = TestStore::open().await;
    let mcp = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    let agent = Arc::new(A2aServerOperationService::new(store.store()));
    let app = build_app(store.store(), mcp, agent).await;
    with_mcp_draft(&app).await;

    // The body names a version that does not exist; the query names the draft.
    // If the query wins, submit succeeds. If the body wins, it is a 400.
    let (status, out) = call(
        &app,
        "POST",
        &format!("/v3/admin/ai/mcp/submit?version={DRAFT_VERSION}"),
        "application/x-www-form-urlencoded",
        &format!("namespaceId={NS}&mcpName={MCP_NAME}&version=9.9.9"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "query must win: {out}");
    assert_eq!(
        mcp_version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("reviewing")
    );
}

/// Missing parameters still produce a structured 400, not a panic or a 200.
#[actix_web::test]
async fn an_empty_form_is_rejected_with_a_structured_error() {
    let store = TestStore::open().await;
    let mcp = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    let agent = Arc::new(A2aServerOperationService::new(store.store()));
    let app = build_app(store.store(), mcp, agent).await;

    let (status, out) = post_form(&app, "/v3/admin/ai/mcp/submit", "").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "expected 400, got {out}");
    assert!(out.contains("required"), "structured 400 expected: {out}");

    let (status, out) = put_form(&app, "/v3/admin/ai/agents/labels", "").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "expected 400, got {out}");
    assert!(out.contains("required"), "structured 400 expected: {out}");
}

/// Agent version transitions and labels/scope use the same dual channel.
#[actix_web::test]
async fn agent_lifecycle_and_labels_accept_ga_form_body() {
    let store = TestStore::open().await;
    let mcp = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    let agent = Arc::new(A2aServerOperationService::new(store.store()));
    let app = build_app(store.store(), mcp, agent.clone()).await;

    agent
        .register_agent(&agent_card(BASE_VERSION), NS, "manual")
        .await
        .expect("register agent");
    agent
        .create_agent_draft(NS, &agent_card(DRAFT_VERSION), false, None)
        .await
        .expect("create agent draft");
    assert_eq!(
        agent_version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("draft")
    );

    let form_body = format!("namespaceId={NS}&agentName={AGENT_NAME}&version={DRAFT_VERSION}");
    let query = format!("?{form_body}");

    let (status, out) = post_form(&app, "/v3/admin/ai/agents/submit", &form_body).await;
    assert_eq!(status, StatusCode::OK, "GA form agent submit failed: {out}");
    assert_eq!(
        agent_version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("reviewing")
    );

    let (status, out) = post_query(&app, &format!("/v3/admin/ai/agents/publish{query}")).await;
    assert_eq!(status, StatusCode::OK, "query agent publish failed: {out}");
    assert_eq!(
        agent_version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("online")
    );

    let missing_body = format!("namespaceId={NS}&agentName={AGENT_NAME}&version={MISSING_VERSION}");
    let missing_query = format!("?{missing_body}");
    for path in [
        "/v3/admin/ai/agents/force-publish",
        "/v3/admin/ai/agents/redraft",
        "/v3/admin/ai/agents/online",
        "/v3/admin/ai/agents/offline",
        "/v3/admin/ai/agents/submit",
        "/v3/admin/ai/agents/publish",
    ] {
        let form = post_form(&app, path, &missing_body).await;
        let via_query = post_query(&app, &format!("{path}{missing_query}")).await;
        assert_same(&format!("agent {path}"), &form, &via_query);
        assert!(
            !form.1.contains("required"),
            "{path}: form parameters were dropped: {}",
            form.1
        );
    }

    // `labels` arrives as a JSON string inside the form body on GA.
    let labels = format!(r#"{{"stable":"{BASE_VERSION}"}}"#);
    let (status, out) = put_form(
        &app,
        "/v3/admin/ai/agents/labels",
        &format!(
            "namespaceId={NS}&agentName={AGENT_NAME}&labels={}",
            percent_encode(&labels)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GA form agent labels failed: {out}");
    assert!(
        out.contains("stable"),
        "the GA form labels parameter must be applied: {out}"
    );

    let scope_form = format!("namespaceId={NS}&agentName={AGENT_NAME}&scope=PUBLIC");
    let (status, out) = put_form(&app, "/v3/admin/ai/agents/scope", &scope_form).await;
    assert_eq!(status, StatusCode::OK, "GA form agent scope failed: {out}");
    let via_query = put_query(&app, &format!("/v3/admin/ai/agents/scope?{scope_form}")).await;
    let form_again = put_form(&app, "/v3/admin/ai/agents/scope", &scope_form).await;
    assert_same("agent scope", &form_again, &via_query);

    let parsed: Value = serde_json::from_str(&out).unwrap_or_else(|_| json!({}));
    assert!(parsed.is_object(), "scope response must be JSON: {out}");
}

/// `POST /draft` reads its flat admin parameters from the GA form body too.
///
/// `overwrite` is only observable through a second draft creation: with
/// `overwrite=false` the service refuses to replace the editing version, with
/// `overwrite=true` it succeeds. If the form parameters were dropped, both
/// calls would behave as `false` and the success path could never be reached.
#[actix_web::test]
async fn mcp_create_draft_reads_ga_form_parameters() {
    let store = TestStore::open().await;
    let mcp = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    let agent = Arc::new(A2aServerOperationService::new(store.store()));
    let app = build_app(store.store(), mcp, agent).await;
    with_mcp_draft(&app).await;

    let spec = serde_json::to_string(&mcp_registration(DRAFT_VERSION)).expect("serialize");
    let form = |overwrite: &str| {
        format!(
            "serverSpecification={}&namespaceId={NS}&mcpName={MCP_NAME}&version={DRAFT_VERSION}&overwrite={overwrite}",
            percent_encode(&spec)
        )
    };

    let (status, out) = post_form(&app, "/v3/admin/ai/mcp/draft", &form("false")).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "overwrite=false must be honoured: {out}"
    );
    assert!(out.contains("overwrite"), "domain error expected: {out}");

    let (status, out) = post_form(&app, "/v3/admin/ai/mcp/draft", &form("true")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "overwrite=true from the GA form body must be honoured: {out}"
    );
    assert_eq!(
        mcp_version_status(&store, DRAFT_VERSION).await.as_deref(),
        Some("draft")
    );
}

/// Build the MCP + agent admin app for the GA flat-form Agent write tests.
macro_rules! agent_form_app {
    ($store:ident, $app:ident, $svc:ident) => {
        let $store = TestStore::open().await;
        let mcp = Arc::new(McpServerOperationService::new(
            $store.store(),
            Arc::new(McpServerIndex::new()),
        ));
        let agent = Arc::new(A2aServerOperationService::new($store.store()));
        // These fixtures deliberately pre-register an Agent so the tests cover
        // the existing-resource branch. The first-draft initialization branch
        // has its own tests below, which use a fresh store with no registration.
        let $svc = agent.clone();
        let $app = build_app($store.store(), mcp, agent).await;
    };
}

/// A GA `AgentDraftCreateForm` body must reach `create_agent_draft` with every
/// mapped field, not just avoid a 415/400. The stored card is the proof.
#[actix_web::test]
async fn agent_flat_form_creates_a_draft_with_mapped_fields() {
    agent_form_app!(store, app, agent_svc);
    agent_svc
        .register_agent(&agent_card_named(FLAT_AGENT, BASE_VERSION), NS, "manual")
        .await
        .expect("register agent");

    let form = format!(
        "namespaceId={NS}&agentName={FLAT_AGENT}&version={FLAT_VERSION}&displayName={}&description={}&iconUrl={}&provider={}&tags={}&extensions={}&author=alice&changeDescription={}&basedOnVersion={BASE_VERSION}",
        percent_encode("GA Draft"),
        percent_encode("flat form description"),
        percent_encode("https://example.com/icon.png"),
        percent_encode(r#"{"name":"acme","url":"https://acme.example"}"#),
        percent_encode(r#"["coding","writing"]"#),
        percent_encode(r#"{"team":"ai","seats":3}"#),
        percent_encode("initial draft"),
    );
    let (status, out) = post_form(&app, "/v3/admin/ai/agents/draft", &form).await;
    assert_eq!(status, StatusCode::OK, "GA flat draft failed: {out}");
    assert_eq!(
        agent_version_status_for(&store, FLAT_AGENT, FLAT_VERSION)
            .await
            .as_deref(),
        Some("draft")
    );

    let storage = stored_agent_card(&store, FLAT_AGENT, FLAT_VERSION)
        .await
        .expect("stored card");
    for needle in [
        "\"displayName\":\"GA Draft\"",
        "\"description\":\"flat form description\"",
        "\"iconUrl\":\"https://example.com/icon.png\"",
        "\"organization\":\"acme\"",
        "\"tags\":[\"coding\",\"writing\"]",
        "\"team\":\"ai\"",
        "\"seats\":\"3\"",
        "\"ga.author\":\"alice\"",
        "\"ga.changeDescription\":\"initial draft\"",
        "\"ga.basedOnVersion\":\"1.0.0\"",
    ] {
        assert!(storage.contains(needle), "{needle} missing from {storage}");
    }
}

/// Batata's own nested `AgentCard` JSON wire must still register an agent.
#[actix_web::test]
async fn nested_json_agent_card_wire_is_preserved() {
    agent_form_app!(store, app, agent_svc);
    let _ = &agent_svc;

    let card =
        serde_json::to_string(&agent_card_named(JSON_AGENT, JSON_VERSION)).expect("serialize card");
    let (status, out) = call(
        &app,
        "POST",
        &format!("/v3/admin/ai/agents?namespaceId={NS}"),
        "application/json",
        &card,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "nested JSON register failed: {out}");
    let storage = stored_agent_card(&store, JSON_AGENT, JSON_VERSION)
        .await
        .expect("stored card");
    assert!(storage.contains(JSON_AGENT), "card name missing: {storage}");
}

/// `PUT /draft` uses the same flat GA form and persists the mapped changes.
#[actix_web::test]
async fn agent_flat_form_updates_the_editing_draft() {
    agent_form_app!(store, app, agent_svc);
    agent_svc
        .register_agent(&agent_card_named(FLAT_AGENT, BASE_VERSION), NS, "manual")
        .await
        .expect("register agent");

    let card =
        serde_json::to_string(&agent_card_named(FLAT_AGENT, FLAT_VERSION)).expect("serialize card");
    let (status, out) = call(
        &app,
        "POST",
        &format!("/v3/admin/ai/agents/draft?namespaceId={NS}"),
        "application/json",
        &card,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "JSON draft setup failed: {out}");

    let form = format!(
        "namespaceId={NS}&agentName={FLAT_AGENT}&version={FLAT_VERSION}&description={}&changeDescription={}",
        percent_encode("updated description"),
        percent_encode("edit one"),
    );
    let (status, out) = put_form(&app, "/v3/admin/ai/agents/draft", &form).await;
    assert_eq!(status, StatusCode::OK, "GA flat draft update failed: {out}");
    let storage = stored_agent_card(&store, FLAT_AGENT, FLAT_VERSION)
        .await
        .expect("stored card");
    assert!(
        storage.contains("\"description\":\"updated description\""),
        "mapped description missing: {storage}"
    );
    assert!(
        storage.contains("\"ga.changeDescription\":\"edit one\""),
        "preserved GA metadata missing: {storage}"
    );
}

/// GA `AgentUpdateForm` has no `version`; `PUT /agents` must not demand one.
#[actix_web::test]
async fn agent_flat_form_update_does_not_require_a_version() {
    agent_form_app!(store, app, agent_svc);
    let _ = &agent_svc;

    let card =
        serde_json::to_string(&agent_card_named(JSON_AGENT, JSON_VERSION)).expect("serialize");
    let (status, out) = call(
        &app,
        "POST",
        &format!("/v3/admin/ai/agents?namespaceId={NS}"),
        "application/json",
        &card,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "JSON register failed: {out}");

    let (status, out) = put_form(
        &app,
        "/v3/admin/ai/agents",
        &format!(
            "namespaceId={NS}&agentName={JSON_AGENT}&description={}&status=ONLINE",
            percent_encode("updated by GA form")
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GA flat update failed: {out}");
    assert!(
        !out.contains("version is required"),
        "unexpected 400: {out}"
    );
}

/// The GA ARD `callInterfaces` field is accepted (GA's
/// `requireInitialDraftContent` needs it non-null) and persisted on the version
/// definition; it round-trips through the draft response.
#[actix_web::test]
async fn ga_call_interfaces_are_accepted_and_persisted_at_the_route() {
    agent_form_app!(store, app, agent_svc);
    let _ = &agent_svc;

    let form = format!(
        "namespaceId={NS}&agentName={FLAT_AGENT}&version={FLAT_VERSION}&callInterfaces={}",
        percent_encode(r#"[{"protocol":"JSONRPC"}]"#),
    );
    let (status, out) = post_form(&app, "/v3/admin/ai/agents/draft", &form).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "callInterfaces must be accepted, got {out}"
    );
    assert!(
        out.contains("callInterfaces"),
        "the accepted draft must echo callInterfaces: {out}"
    );
    assert!(
        out.contains("JSONRPC"),
        "the call interface protocol must be preserved: {out}"
    );
    assert!(
        stored_agent_card(&store, FLAT_AGENT, FLAT_VERSION)
            .await
            .is_some(),
        "an accepted ARD draft must be persisted"
    );
}

/// Missing identity parameters are still a structured 400, and the query
/// keeps precedence over the GA form body.
#[actix_web::test]
async fn agent_flat_form_validation_and_query_precedence() {
    agent_form_app!(store, app, agent_svc);

    let (status, out) = post_form(&app, "/v3/admin/ai/agents/draft", "").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "expected 400, got {out}");
    assert!(out.contains("agentName is required"), "expected 400: {out}");

    let (status, out) = post_form(
        &app,
        "/v3/admin/ai/agents/draft",
        &format!("namespaceId={NS}&agentName={FLAT_AGENT}"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "expected 400, got {out}");
    assert!(out.contains("version is required"), "expected 400: {out}");

    let (status, out) = put_form(
        &app,
        "/v3/admin/ai/agents/draft",
        &format!("namespaceId={NS}&agentName={FLAT_AGENT}"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "expected 400, got {out}");
    assert!(out.contains("version is required"), "expected 400: {out}");

    agent_svc
        .register_agent(
            &agent_card_named("query-wins-agent", BASE_VERSION),
            NS,
            "manual",
        )
        .await
        .expect("register agent");
    let (status, out) = call(
        &app,
        "POST",
        "/v3/admin/ai/agents/draft?agentName=query-wins-agent",
        "application/x-www-form-urlencoded",
        &format!("namespaceId={NS}&agentName=body-loses-agent&version={FLAT_VERSION}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "query precedence failed: {out}");
    assert!(
        stored_agent_card(&store, "query-wins-agent", FLAT_VERSION)
            .await
            .is_some(),
        "the query agentName must win"
    );
    assert!(
        stored_agent_card(&store, "body-loses-agent", FLAT_VERSION)
            .await
            .is_none(),
        "the body agentName must not be used"
    );
}

/// `overwrite` is a GA form parameter, not a query-only flag: a second draft
/// with the same identity must fail at `false` and succeed at `true`.
#[actix_web::test]
async fn agent_flat_form_overwrite_is_honoured() {
    agent_form_app!(store, app, agent_svc);
    agent_svc
        .register_agent(&agent_card_named(FLAT_AGENT, BASE_VERSION), NS, "manual")
        .await
        .expect("register agent");

    let base = format!("namespaceId={NS}&agentName={FLAT_AGENT}&version={FLAT_VERSION}");
    let (status, out) = post_form(&app, "/v3/admin/ai/agents/draft", &base).await;
    assert_eq!(status, StatusCode::OK, "first draft failed: {out}");

    let (status, out) = post_form(
        &app,
        "/v3/admin/ai/agents/draft",
        &format!("{base}&overwrite=false"),
    )
    .await;
    assert_ne!(
        status,
        StatusCode::OK,
        "overwrite=false must be honoured: {out}"
    );
    assert!(out.contains("overwrite"), "domain error expected: {out}");

    let (status, out) = post_form(
        &app,
        "/v3/admin/ai/agents/draft",
        &format!("{base}&overwrite=true"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "overwrite=true from the GA form body must be honoured: {out}"
    );
    assert_eq!(
        agent_version_status_for(&store, FLAT_AGENT, FLAT_VERSION)
            .await
            .as_deref(),
        Some("draft")
    );
}

/// GA `AgentOperationService.createValidatedDraft` initializes the Agent
/// directory when `findMeta` returns null. A first `POST /agents/draft` must
/// therefore work on a fresh namespace with no prior `register_agent`.
#[actix_web::test]
async fn first_ga_flat_form_draft_initializes_agent_metadata() {
    let store = TestStore::open().await;
    let mcp = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    let agent = Arc::new(A2aServerOperationService::new(store.store()));
    let app = build_app(store.store(), mcp, agent).await;

    let form = format!(
        "namespaceId={NS}&agentName={FIRST_AGENT}&version={FIRST_VERSION}&displayName={}&description={}",
        percent_encode("First Draft"),
        percent_encode("initialized by first draft"),
    );
    let (status, out) = post_form(&app, "/v3/admin/ai/agents/draft", &form).await;
    assert_eq!(status, StatusCode::OK, "first GA draft failed: {out}");

    let resource = store
        .store()
        .ai_resource_find(NS, FIRST_AGENT, "agent")
        .await
        .expect("resource lookup")
        .expect("first draft must initialize the Agent metadata row");
    assert_eq!(resource.resource_type, "agent");
    assert_eq!(resource.namespace_id, NS);
    assert_eq!(resource.status.as_deref(), Some("enable"));
    assert_eq!(resource.scope, "PRIVATE");
    assert_eq!(
        resource.description.as_deref(),
        Some("initialized by first draft")
    );

    let ext: Value = serde_json::from_str(resource.ext.as_deref().unwrap_or("{}"))
        .expect("resource extension JSON");
    let id = ext["id"].as_str().expect("agent id").to_string();
    assert!(!id.is_empty(), "a stable agent id must be initialized");
    assert_eq!(ext["registration_type"].as_str(), Some("manual"));

    // The CAS after initialization must persist the editing-version marker.
    let version_index: Value =
        serde_json::from_str(resource.version_info.as_deref().unwrap_or("{}"))
            .expect("version index JSON");
    assert_eq!(
        version_index["editingVersion"].as_str(),
        Some(FIRST_VERSION)
    );

    assert_eq!(
        agent_version_status_for(&store, FIRST_AGENT, FIRST_VERSION)
            .await
            .as_deref(),
        Some("draft")
    );
    let storage = stored_agent_card(&store, FIRST_AGENT, FIRST_VERSION)
        .await
        .expect("stored card");
    let detail: Value = serde_json::from_str(&storage).expect("stored detail JSON");
    assert_eq!(detail["id"].as_str(), Some(id.as_str()));
    assert_eq!(detail["registrationType"].as_str(), Some("manual"));
    assert_eq!(detail["version"].as_str(), Some(FIRST_VERSION));
}

/// The batata-native nested `AgentCard` wire must use the same initialization
/// path; before batch 6 this returned an internal "agent not found" error.
#[actix_web::test]
async fn first_nested_json_draft_initializes_agent_metadata() {
    let store = TestStore::open().await;
    let mcp = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    let agent = Arc::new(A2aServerOperationService::new(store.store()));
    let app = build_app(store.store(), mcp, agent).await;

    let card = serde_json::to_string(&agent_card_named(FIRST_JSON_AGENT, FIRST_VERSION))
        .expect("serialize card");
    let (status, out) = call(
        &app,
        "POST",
        &format!("/v3/admin/ai/agents/draft?namespaceId={NS}"),
        "application/json",
        &card,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "first JSON draft failed: {out}");
    assert!(
        store
            .store()
            .ai_resource_find(NS, FIRST_JSON_AGENT, "agent")
            .await
            .expect("resource lookup")
            .is_some(),
        "the nested JSON wire must initialize Agent metadata too"
    );
    assert_eq!(
        agent_version_status_for(&store, FIRST_JSON_AGENT, FIRST_VERSION)
            .await
            .as_deref(),
        Some("draft")
    );
}

/// Initialization must not skip the version index: a second first-draft
/// attempt on the initialized Agent has to honour `overwrite` and persist the
/// replacement through the optimistic-lock CAS.
#[actix_web::test]
async fn initialized_draft_version_index_is_persisted() {
    let store = TestStore::open().await;
    let mcp = Arc::new(McpServerOperationService::new(
        store.store(),
        Arc::new(McpServerIndex::new()),
    ));
    let agent = Arc::new(A2aServerOperationService::new(store.store()));
    let app = build_app(store.store(), mcp, agent).await;

    let base = format!("namespaceId={NS}&agentName={FIRST_AGENT}&version={FIRST_VERSION}");
    let (status, out) = post_form(&app, "/v3/admin/ai/agents/draft", &base).await;
    assert_eq!(status, StatusCode::OK, "initial draft failed: {out}");

    let (status, out) = post_form(
        &app,
        "/v3/admin/ai/agents/draft",
        &format!("{base}&overwrite=false"),
    )
    .await;
    assert_ne!(status, StatusCode::OK, "overwrite=false must fail: {out}");
    assert!(out.contains("overwrite"), "domain error expected: {out}");

    let (status, out) = post_form(
        &app,
        "/v3/admin/ai/agents/draft",
        &format!("{base}&overwrite=true"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "overwrite=true failed: {out}");
    assert_eq!(
        agent_version_status_for(&store, FIRST_AGENT, FIRST_VERSION)
            .await
            .as_deref(),
        Some("draft")
    );
}
