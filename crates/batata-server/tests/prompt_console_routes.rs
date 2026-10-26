//! Route-level test for the console prompt API.
//!
//! The console layer is what the UI actually calls, so this covers the part the
//! service-level test cannot: that the endpoints are registered on the console
//! scope, and that they accept the form-encoded bodies the Nacos console UI
//! sends (reads and deletes use query parameters instead).
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run the same cases
//! against a real database. See `common/ai_store.rs`.

#[path = "common/ai_store.rs"]
mod ai_store;

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_ai::PromptOperationService;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const KEY: &str = "console-route-prompt";

/// Cluster manager stub. The prompt routes never touch cluster state.
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
        unimplemented!("not used by the console prompt routes")
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

/// Build an actix app exposing the real console prompt routes.
async fn build_app(
    store: Arc<dyn PersistenceService>,
    svc: Arc<PromptOperationService>,
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

    // The console reaches prompts through the trait, exactly as the server wires it.
    let prompt: Arc<dyn batata_common::PromptService> = svc;

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(prompt))
            // Mounted exactly as the server does: console routes live under
            // `/v3/console`, and the module adds its own `/ai/...` prefix.
            .service(web::scope("/v3/console").service(batata_console::v3::ai_prompt::routes())),
    )
    .await
}

async fn setup() -> (TestStore, Arc<PromptOperationService>) {
    let store = TestStore::open().await;
    let svc = Arc::new(PromptOperationService::new(store.store()));
    (store, svc)
}

/// POST a form-encoded body, as the console UI does for every write.
async fn post_form<S>(app: &S, uri: &str, form: &str) -> (StatusCode, String)
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
        .set_payload(form.to_string())
        .to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    let body = test::read_body(resp).await;
    (status, String::from_utf8_lossy(&body).to_string())
}

/// The draft lifecycle is reachable on the console scope, and writes really do
/// accept form-encoded bodies.
#[actix_web::test]
async fn draft_lifecycle_over_console_http() {
    let (store, svc) = setup().await;
    store.clean(KEY).await;
    let app = build_app(store.store(), svc.clone()).await;

    let base = "/v3/console/ai/prompt";

    let (status, body) = post_form(
        &app,
        &format!("{base}/draft"),
        // Upstream's create-draft field is `targetVersion`, not `version`.
        &format!("promptKey={KEY}&targetVersion=1.0.0&template=hello"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");
    println!("ok: POST draft (form-encoded)");

    // Publishing an unsubmitted draft must be refused.
    let (status, _) = post_form(
        &app,
        &format!("{base}/publish"),
        &format!("promptKey={KEY}&version=1.0.0"),
    )
    .await;
    assert!(
        status.is_client_error(),
        "publishing an unsubmitted draft must be refused, got {status}"
    );
    println!("ok: publish refused before submit");

    // Submit, then publish.
    let (status, body) = post_form(
        &app,
        &format!("{base}/submit"),
        &format!("promptKey={KEY}&version=1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "submit failed: {body}");

    let (status, body) = post_form(
        &app,
        &format!("{base}/publish"),
        &format!("promptKey={KEY}&version=1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "publish failed: {body}");
    println!("ok: submit then publish");

    store.clean(KEY).await;
}

/// The governance view exposes the state the UI needs to drive the lifecycle.
#[actix_web::test]
async fn governance_reports_lifecycle_state() {
    let (store, svc) = setup().await;
    store.clean(KEY).await;
    let app = build_app(store.store(), svc.clone()).await;

    post_form(
        &app,
        "/v3/console/ai/prompt/draft",
        &format!("promptKey={KEY}&targetVersion=2.0.0&template=draft"),
    )
    .await;

    let req = test::TestRequest::get()
        .uri(&format!("/v3/console/ai/prompt/governance?promptKey={KEY}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();

    // The draft is reported, and versions carry their status.
    assert!(
        body.contains("editingVersion"),
        "must report the draft: {body}"
    );
    assert!(body.contains("2.0.0"), "must list the version: {body}");
    assert!(
        body.contains("versionDetails"),
        "must carry versionDetails: {body}"
    );
    assert!(
        body.contains("\"draft\""),
        "version must carry its status: {body}"
    );
    println!("ok: governance reports lifecycle state");

    store.clean(KEY).await;
}
