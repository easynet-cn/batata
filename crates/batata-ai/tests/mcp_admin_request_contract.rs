//! Request-contract tests for the MCP admin API.
//!
//! Upstream's `McpAdminController` is a Spring controller: its form objects
//! bind a parameter from the query string or the body alike, so a client is
//! free to send either. Batata has to pick a concrete encoding per handler,
//! and getting that wrong is invisible from the inside — the handler simply
//! never sees the parameter.
//!
//! So these tests stand where a client stands: they speak the exact wire
//! format `McpMaintainerServiceImpl` produces and assert what the service
//! received. A recording service keeps them free of any database.

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{App, test, web};
use batata_common::model::ai::mcp::{
    McpServerRegistration, McpServerVersionDetail, McpServerVersionSummary,
};
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;
use tokio::sync::Mutex;

const NS: &str = "public";
const NAME: &str = "contract-mcp";

/// What the service was asked to do.
#[derive(Debug, Default)]
struct Recorded {
    /// `(namespace, name, enabled)`.
    status: Vec<(String, String, bool)>,
    /// `(namespace, name, labels)`.
    labels: Vec<(String, String, HashMap<String, String>)>,
    /// `(namespace, registration, overwrite)`.
    drafts: Vec<(String, McpServerRegistration, bool)>,
    /// `(namespace, registration)`.
    creates: Vec<(String, McpServerRegistration)>,
    /// `(namespace, name, status)`.
    version_lists: Vec<(String, String, Option<String>)>,
    /// `search` as passed through.
    searches: Vec<String>,
}

/// Records the calls the handlers make and nothing else.
struct RecordingService(Mutex<Recorded>);

impl RecordingService {
    fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(Recorded::default())))
    }

    fn detail(version: &str) -> McpServerVersionDetail {
        McpServerVersionDetail {
            summary: McpServerVersionSummary {
                version: version.to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

#[async_trait::async_trait]
impl batata_common::McpServerService for RecordingService {
    async fn create_mcp_server(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<String> {
        self.0
            .lock()
            .await
            .creates
            .push((namespace.to_string(), registration.clone()));
        Ok("id".to_string())
    }

    async fn update_mcp_server_status(
        &self,
        namespace: &str,
        name: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
        self.0
            .lock()
            .await
            .status
            .push((namespace.to_string(), name.to_string(), enabled));
        Ok(())
    }

    async fn update_mcp_server_labels(
        &self,
        namespace: &str,
        name: &str,
        labels: HashMap<String, String>,
    ) -> anyhow::Result<HashMap<String, String>> {
        self.0
            .lock()
            .await
            .labels
            .push((namespace.to_string(), name.to_string(), labels.clone()));
        Ok(labels)
    }

    async fn create_mcp_server_draft(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
        overwrite: bool,
    ) -> anyhow::Result<McpServerVersionDetail> {
        self.0
            .lock()
            .await
            .drafts
            .push((namespace.to_string(), registration.clone(), overwrite));
        Ok(Self::detail(&registration.version))
    }

    async fn list_mcp_server_versions(
        &self,
        namespace: &str,
        name: &str,
        status: Option<&str>,
        _page_no: u64,
        _page_size: u64,
    ) -> anyhow::Result<batata_common::model::Page<McpServerVersionSummary>> {
        self.0.lock().await.version_lists.push((
            namespace.to_string(),
            name.to_string(),
            status.map(str::to_string),
        ));
        Ok(batata_common::model::Page::new(0, 1, 20, vec![]))
    }

    async fn list_mcp_servers(
        &self,
        _namespace: &str,
        _name: Option<&str>,
        search_type: &str,
        _page_no: u32,
        _page_size: u32,
        _user: Option<&str>,
    ) -> batata_common::model::Page<batata_common::model::ai::mcp::McpServerBasicInfo> {
        self.0.lock().await.searches.push(search_type.to_string());
        batata_common::model::Page::new(0, 1, 20, vec![])
    }

    // Everything below is outside the contract these tests are about; a call
    // is a bug in the test, not something to answer quietly.
    async fn get_mcp_server_detail(
        &self,
        _namespace: &str,
        _id: Option<&str>,
        _name: Option<&str>,
        _version: Option<&str>,
        _user: Option<&str>,
    ) -> anyhow::Result<Option<batata_common::model::ai::mcp::McpServer>> {
        unimplemented!()
    }
    async fn update_mcp_server(
        &self,
        _namespace: &str,
        _registration: &McpServerRegistration,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }
    async fn delete_mcp_server(
        &self,
        _namespace: &str,
        _name: Option<&str>,
        _id: Option<&str>,
        _version: Option<&str>,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }
    async fn get_mcp_server_version(
        &self,
        _namespace: &str,
        _name: &str,
        _version: &str,
    ) -> anyhow::Result<Option<McpServerVersionDetail>> {
        unimplemented!()
    }
    async fn update_mcp_server_draft(
        &self,
        _namespace: &str,
        _registration: &McpServerRegistration,
    ) -> anyhow::Result<McpServerVersionDetail> {
        unimplemented!()
    }
    async fn delete_mcp_server_draft(
        &self,
        _namespace: &str,
        _name: &str,
        _version: &str,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }
    async fn submit_mcp_server_version(
        &self,
        _namespace: &str,
        _name: &str,
        _version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        unimplemented!()
    }
    async fn publish_mcp_server_version(
        &self,
        _namespace: &str,
        _name: &str,
        _version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        unimplemented!()
    }
    async fn force_publish_mcp_server_version(
        &self,
        _namespace: &str,
        _name: &str,
        _version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        unimplemented!()
    }
    async fn redraft_mcp_server_version(
        &self,
        _namespace: &str,
        _name: &str,
        _version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        unimplemented!()
    }
    async fn online_mcp_server_version(
        &self,
        _namespace: &str,
        _name: &str,
        _version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        unimplemented!()
    }
    async fn offline_mcp_server_version(
        &self,
        _namespace: &str,
        _name: &str,
        _version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        unimplemented!()
    }
    async fn update_mcp_server_scope(
        &self,
        _namespace: &str,
        _name: &str,
        _new_scope: &str,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }
    async fn search_mcp_servers(
        &self,
        _namespace: &str,
        _query: &str,
        _page_no: u64,
        _page_size: u64,
    ) -> anyhow::Result<
        batata_common::model::Page<batata_common::model::ai::search::AiResourceSearchHit>,
    > {
        unimplemented!()
    }
    async fn import_tools_from_mcp(
        &self,
        _base_url: &str,
        _endpoint: &str,
        _auth_token: Option<&str>,
        _timeout: std::time::Duration,
    ) -> anyhow::Result<Vec<batata_common::model::ai::mcp::McpTool>> {
        unimplemented!()
    }
    async fn mcp_stats(&self) -> anyhow::Result<batata_common::model::ai::mcp::McpRegistryStats> {
        unimplemented!()
    }
}

/// Cluster manager stub. These routes never touch cluster state.
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

/// Build an app exposing the real MCP admin routes, backed by `service`.
async fn build_app(
    service: Arc<RecordingService>,
) -> (
    impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    tempfile::TempDir,
) {
    let mut configuration = Configuration::default();
    configuration.typed.core.auth.console.enabled = false;

    let tmp = tempfile::TempDir::new().expect("temp dir");
    let sm = batata_consistency::RocksStateMachine::new(tmp.path())
        .await
        .expect("state machine");
    let store: Arc<dyn batata_persistence::PersistenceService> =
        Arc::new(batata_persistence::EmbeddedPersistService::from_state_machine(&sm));

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

    let mcp: Arc<dyn batata_common::McpServerService> = service;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(mcp))
            .service(web::scope("/v3/admin/ai").service(batata_ai::mcp_admin_routes())),
    )
    .await;
    (app, tmp)
}

/// Send a request and return its status plus body.
async fn call<S>(app: &S, req: actix_http::Request) -> (actix_web::http::StatusCode, String)
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

fn form(method: actix_web::http::Method, uri: &str, body: &str) -> actix_http::Request {
    test::TestRequest::with_uri(uri)
        .method(method)
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload(body.to_string())
        .to_request()
}

fn get(uri: &str) -> actix_http::Request {
    test::TestRequest::get().uri(uri).to_request()
}

/// `createMcpServer` posts `serverSpecification` as a JSON string in a form.
#[actix_web::test]
async fn create_reads_the_maintainer_client_form_encoding() {
    let service = RecordingService::new();
    let (app, _tmp) = build_app(service.clone()).await;

    let body = format!(
        "serverSpecification={}&namespaceId={}",
        urlencode(r#"{"name":"contract-mcp","version":"1.0.0"}"#),
        NS
    );
    let (status, out) = call(
        &app,
        form(actix_web::http::Method::POST, "/v3/admin/ai/mcp", &body),
    )
    .await;
    assert_eq!(status, actix_web::http::StatusCode::OK, "create: {out}");

    let recorded = service.0.lock().await;
    let (namespace, registration) = recorded.creates.first().expect("create was recorded");
    assert_eq!(namespace, NS);
    assert_eq!(registration.name, NAME);
    assert_eq!(registration.version, "1.0.0");
}

/// `createMcpServerDraft` posts the same shape, plus `mcpName` and `version`.
#[actix_web::test]
async fn draft_reads_the_maintainer_client_form_encoding() {
    let service = RecordingService::new();
    let (app, _tmp) = build_app(service.clone()).await;

    let body = format!(
        "serverSpecification={}&mcpName={}&version=2.0.0&namespaceId={}",
        urlencode(r#"{"name":"contract-mcp"}"#),
        NAME,
        NS
    );
    let (status, out) = call(
        &app,
        form(
            actix_web::http::Method::POST,
            "/v3/admin/ai/mcp/draft",
            &body,
        ),
    )
    .await;
    assert_eq!(status, actix_web::http::StatusCode::OK, "draft: {out}");

    let recorded = service.0.lock().await;
    let (namespace, registration, _) = recorded.drafts.first().expect("draft was recorded");
    assert_eq!(namespace, NS);
    assert_eq!(registration.name, NAME);
    assert_eq!(
        registration.version, "2.0.0",
        "the form-level version applies"
    );
}

/// `updateMcpServerLabels` sends the map as a JSON string *parameter*.
#[actix_web::test]
async fn labels_read_the_upstream_labels_parameter() {
    let service = RecordingService::new();
    let (app, _tmp) = build_app(service.clone()).await;

    let uri = format!(
        "/v3/admin/ai/mcp/labels?mcpName={}&labels={}",
        NAME,
        urlencode(r#"{"stable":"1.0.0"}"#)
    );
    let (status, out) = call(&app, form(actix_web::http::Method::PUT, &uri, "")).await;
    assert_eq!(status, actix_web::http::StatusCode::OK, "labels: {out}");

    let recorded = service.0.lock().await;
    let (_, name, labels) = recorded.labels.first().expect("labels were recorded");
    assert_eq!(name, NAME);
    assert_eq!(labels.get("stable").map(String::as_str), Some("1.0.0"));
}

/// The console posts the map as a body instead; both must land.
#[actix_web::test]
async fn labels_read_a_json_body() {
    let service = RecordingService::new();
    let (app, _tmp) = build_app(service.clone()).await;

    let uri = format!("/v3/admin/ai/mcp/labels?mcpName={NAME}");
    let req = test::TestRequest::put()
        .uri(&uri)
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"labels":{"beta":"2.0.0"}}"#)
        .to_request();
    let (status, out) = call(&app, req).await;
    assert_eq!(status, actix_web::http::StatusCode::OK, "labels: {out}");

    let recorded = service.0.lock().await;
    let (_, _, labels) = recorded.labels.first().expect("labels were recorded");
    assert_eq!(labels.get("beta").map(String::as_str), Some("2.0.0"));
}

/// `updateMcpServerStatus` sends `enabled=false`. Reading a `status`
/// parameter instead used to turn a disable into an enable.
#[actix_web::test]
async fn enabled_false_disables() {
    let service = RecordingService::new();
    let (app, _tmp) = build_app(service.clone()).await;

    let uri = format!("/v3/admin/ai/mcp/status?mcpName={NAME}&enabled=false");
    let (status, out) = call(&app, form(actix_web::http::Method::PUT, &uri, "")).await;
    assert_eq!(status, actix_web::http::StatusCode::OK, "status: {out}");

    let uri = format!("/v3/admin/ai/mcp/status?mcpName={NAME}&enabled=true");
    let (status, out) = call(&app, form(actix_web::http::Method::PUT, &uri, "")).await;
    assert_eq!(status, actix_web::http::StatusCode::OK, "status: {out}");

    let recorded = service.0.lock().await;
    let enabled: Vec<bool> = recorded.status.iter().map(|(_, _, e)| *e).collect();
    assert_eq!(
        enabled,
        vec![false, true],
        "enabled must reach the service as sent"
    );
}

/// The console's `status=disable` keeps working.
#[actix_web::test]
async fn the_console_disable_spelling_still_disables() {
    let service = RecordingService::new();
    let (app, _tmp) = build_app(service.clone()).await;

    let uri = format!("/v3/admin/ai/mcp/status?mcpName={NAME}&status=DISABLE");
    let (status, out) = call(&app, form(actix_web::http::Method::PUT, &uri, "")).await;
    assert_eq!(status, actix_web::http::StatusCode::OK, "status: {out}");

    let recorded = service.0.lock().await;
    assert!(!recorded.status.first().expect("recorded").2);
}

/// Upstream requires `enabled`; silence must not be read as "enable".
#[actix_web::test]
async fn status_without_enabled_is_refused() {
    let service = RecordingService::new();
    let (app, _tmp) = build_app(service.clone()).await;

    let uri = format!("/v3/admin/ai/mcp/status?mcpName={NAME}");
    let (status, _) = call(&app, form(actix_web::http::Method::PUT, &uri, "")).await;
    assert!(
        status.is_client_error(),
        "a status update without `enabled` must be refused, got {status}"
    );
    assert!(service.0.lock().await.status.is_empty());
}

/// `listMcpServerVersions` filters by status; the filter must be applied.
#[actix_web::test]
async fn versions_honour_the_status_filter() {
    let service = RecordingService::new();
    let (app, _tmp) = build_app(service.clone()).await;

    let uri = format!("/v3/admin/ai/mcp/versions?mcpName={NAME}&status=ONLINE");
    let (status, out) = call(&app, get(&uri)).await;
    assert_eq!(status, actix_web::http::StatusCode::OK, "versions: {out}");

    let recorded = service.0.lock().await;
    let (_, name, filter) = recorded.version_lists.first().expect("recorded");
    assert_eq!(name, NAME);
    assert_eq!(
        filter.as_deref(),
        Some("online"),
        "upstream lower-cases the status"
    );
}

/// An unknown status is a client error, not an empty page.
#[actix_web::test]
async fn an_unknown_status_filter_is_refused() {
    let service = RecordingService::new();
    let (app, _tmp) = build_app(service.clone()).await;

    let uri = format!("/v3/admin/ai/mcp/versions?mcpName={NAME}&status=bogus");
    let (status, _) = call(&app, get(&uri)).await;
    assert!(
        status.is_client_error(),
        "an unknown status must be refused, got {status}"
    );
    assert!(service.0.lock().await.version_lists.is_empty());
}

/// `McpListForm` defaults to `accurate` and rejects anything else.
#[actix_web::test]
async fn list_defaults_to_accurate_and_validates_search() {
    let service = RecordingService::new();
    let (app, _tmp) = build_app(service.clone()).await;

    let (status, out) = call(&app, get("/v3/admin/ai/mcp/list")).await;
    assert_eq!(status, actix_web::http::StatusCode::OK, "list: {out}");
    assert_eq!(
        service.0.lock().await.searches,
        vec!["accurate".to_string()]
    );

    let (status, _) = call(&app, get("/v3/admin/ai/mcp/list?search=fuzzy")).await;
    assert!(
        status.is_client_error(),
        "an unknown search type must be refused, got {status}"
    );

    let (status, out) = call(&app, get("/v3/admin/ai/mcp/list?search=blur")).await;
    assert_eq!(status, actix_web::http::StatusCode::OK, "list: {out}");
}

/// Percent-encode a value so JSON survives a form or query string.
fn urlencode(value: &str) -> String {
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
