//! Route-level test for the prompt admin API.
//!
//! The service layer is covered by `batata-ai`'s `prompt_persistence` test.
//! This one covers the HTTP layer: that the lifecycle endpoints are actually
//! registered on the scope, and that form parameters reach the service.
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

const KEY: &str = "route-prompt";

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
        unimplemented!("not used by the prompt admin routes")
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

/// Build an actix app exposing the real prompt admin routes.
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

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(svc))
            .service(web::scope("/v3/admin/ai").service(batata_ai::prompt_admin_routes())),
    )
    .await
}

async fn setup() -> (TestStore, Arc<PromptOperationService>) {
    let store = TestStore::open().await;
    let svc = Arc::new(PromptOperationService::new(store.store()));
    (store, svc)
}

/// POST a form-encoded body and return (status, body).
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

/// The draft lifecycle is reachable over HTTP, and the review gate is enforced.
#[actix_web::test]
async fn draft_lifecycle_over_http() {
    let (store, svc) = setup().await;
    store.clean(KEY).await;
    let app = build_app(store.store(), svc.clone()).await;

    let base = "/v3/admin/ai/prompt";

    // Create a draft.
    let (status, body) = post_form(
        &app,
        &format!("{base}/draft"),
        &format!("promptKey={KEY}&version=1.0.0&template=hello"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");
    println!("ok: POST draft");

    // A draft cannot be published before it is submitted.
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

    // submit → publish works.
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

/// force-publish bypasses the review gate.
#[actix_web::test]
async fn force_publish_over_http() {
    let (store, svc) = setup().await;
    store.clean(KEY).await;
    let app = build_app(store.store(), svc.clone()).await;

    let base = "/v3/admin/ai/prompt";
    post_form(
        &app,
        &format!("{base}/draft"),
        &format!("promptKey={KEY}&version=2.0.0&template=draft"),
    )
    .await;

    // No submit: force-publish must still succeed.
    let (status, body) = post_form(
        &app,
        &format!("{base}/force-publish"),
        &format!("promptKey={KEY}&version=2.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "force publish failed: {body}");
    println!("ok: force publish bypasses review");

    store.clean(KEY).await;
}

/// A lifecycle action on a prompt that does not exist is refused.
#[actix_web::test]
async fn lifecycle_action_on_a_missing_prompt_is_refused() {
    let (store, svc) = setup().await;
    store.clean(KEY).await;
    let app = build_app(store.store(), svc.clone()).await;

    let (status, _) = post_form(
        &app,
        "/v3/admin/ai/prompt/publish",
        "promptKey=route-prompt-absent&version=9.9.9",
    )
    .await;
    assert!(
        status.is_client_error(),
        "an unknown prompt must be refused, got {status}"
    );
    println!("ok: unknown prompt refused");
}

/// GET a query-string endpoint and return (status, body).
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

/// `GET /governance` answers with the version summary of one prompt. These two
/// endpoints existed on the console layer only; the admin layer gained them
/// later, so they are covered here too.
#[actix_web::test]
async fn governance_and_download_are_reachable_on_admin() {
    let (store, svc) = setup().await;
    store.clean(KEY).await;
    let app = build_app(store.store(), svc.clone()).await;

    let base = "/v3/admin/ai/prompt";
    let (status, body) = post_form(
        &app,
        &format!("{base}/draft"),
        &format!("promptKey={KEY}&version=1.0.0&template=hello"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");

    // Governance: the same shape as the metadata endpoint upstream.
    let (status, body) = get(&app, &format!("{base}/governance?promptKey={KEY}")).await;
    assert_eq!(status, StatusCode::OK, "governance failed: {body}");
    assert!(body.contains(KEY), "must name the prompt: {body}");
    println!("ok: governance is reachable on admin");

    // Download: one version, as Markdown.
    let (status, body) = get(
        &app,
        &format!("{base}/version/download?promptKey={KEY}&version=1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "download failed: {body}");
    assert!(body.contains("hello"), "must carry the template: {body}");
    println!("ok: version/download is reachable on admin");

    store.clean(KEY).await;
}

/// Both read endpoints 404 for a prompt that does not exist, rather than
/// returning an empty document that looks like a match.
#[actix_web::test]
async fn governance_and_download_refuse_an_unknown_prompt() {
    let (store, svc) = setup().await;
    store.clean(KEY).await;
    let app = build_app(store.store(), svc.clone()).await;

    let base = "/v3/admin/ai/prompt";
    let (status, _) = get(&app, &format!("{base}/governance?promptKey={KEY}-absent")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an unknown prompt must 404 on governance"
    );

    let (status, _) = get(
        &app,
        &format!("{base}/version/download?promptKey={KEY}-absent&version=1.0.0"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an unknown prompt must 404 on download"
    );
    println!("ok: both read endpoints refuse an unknown prompt");
}
