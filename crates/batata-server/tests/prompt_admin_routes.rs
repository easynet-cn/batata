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
use batata_common::model::ai::prompt::PromptStorage;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_persistence::model::{AiResourceInfo, AiResourceVersionInfo};
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

/// `GET /version` is the canonical version-detail read (the legacy `/detail`
/// additionally resolves labels). It must answer for an existing version and
/// refuse a missing `version` parameter with 400 rather than a silent 404.
#[actix_web::test]
async fn version_detail_is_reachable_on_admin() {
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

    // Canonical read: a specific version.
    let (status, body) = get(
        &app,
        &format!("{base}/version?promptKey={KEY}&version=1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "version read failed: {body}");
    assert!(body.contains(KEY), "must name the prompt: {body}");
    assert!(body.contains("hello"), "must carry the template: {body}");
    println!("ok: GET /version answers for an existing version");

    // Guard: missing `version` is a parameter error, never a silent 404.
    let (status, _) = get(&app, &format!("{base}/version?promptKey={KEY}")).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a missing version must be 400, got {status}"
    );
    println!("ok: GET /version refuses a missing version");

    // Unknown prompt + version → 404, not a fabricated empty document.
    let (status, _) = get(
        &app,
        &format!("{base}/version?promptKey={KEY}-absent&version=9.9.9"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an unknown prompt must 404 on version read"
    );
    println!("ok: GET /version refuses an unknown prompt");

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

// ============================================================================
// Batch 10: GA-only fields on POST /v3/admin/ai/prompt/draft
//
// GA `PromptDraftCreateForm` binds `targetVersion`, `basedOnVersion`,
// `commitMsg` and `bizTags`; `PromptAdminController.java:188-191` forwards all
// of them. `commitMsg` becomes the version-row description
// (`AiResourceManager.insertVersionRow`), `bizTags` initializes the metadata
// of a brand-new prompt, and `basedOnVersion` forks the stored template.
// ============================================================================

async fn version_row(store: &TestStore, key: &str, version: &str) -> Option<AiResourceVersionInfo> {
    store
        .store()
        .ai_resource_version_find("public", key, "prompt", version)
        .await
        .expect("version lookup")
}

async fn resource_row(store: &TestStore, key: &str) -> Option<AiResourceInfo> {
    store
        .store()
        .ai_resource_find("public", key, "prompt")
        .await
        .expect("resource lookup")
}

fn stored_template(row: &AiResourceVersionInfo) -> String {
    let raw = row.storage.as_deref().expect("version storage");
    serde_json::from_str::<PromptStorage>(raw)
        .expect("decode PromptStorage")
        .template
}

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

/// The GA form channel: commitMsg -> version desc, bizTags -> resource metadata.
#[actix_web::test]
async fn ga_draft_fields_form_channel() {
    let (store, svc) = setup().await;
    let key = "ga-prompt-fields";
    store.clean(key).await;
    let app = build_app(store.store(), svc.clone()).await;

    let form = format!(
        "promptKey={key}&targetVersion=1.0.0&template={}&description=meta+desc&commitMsg={}&bizTags={}",
        percent_encode("ga template"),
        percent_encode("ga prompt commit"),
        percent_encode("alpha,beta")
    );
    let (status, body) = post_form(&app, "/v3/admin/ai/prompt/draft", &form).await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");

    let row = version_row(&store, key, "1.0.0").await.expect("draft row");
    assert_eq!(
        row.description.as_deref(),
        Some("ga prompt commit"),
        "commitMsg must become the version-level description"
    );
    assert_eq!(stored_template(&row), "ga template");
    let storage: PromptStorage =
        serde_json::from_str(row.storage.as_deref().expect("storage")).expect("decode storage");
    assert_eq!(storage.commit_msg.as_deref(), Some("ga prompt commit"));

    let resource = resource_row(&store, key).await.expect("prompt resource");
    assert_eq!(
        resource.biz_tags.as_deref(),
        Some("alpha,beta"),
        "bizTags must initialize the metadata of a brand-new prompt"
    );
    store.clean(key).await;
}

/// Spring `@RequestParam` also accepts the query string; `AdminForm` must
/// resolve the same four GA wire keys there.
#[actix_web::test]
async fn ga_draft_fields_query_channel() {
    let (store, svc) = setup().await;
    let key = "ga-prompt-query";
    store.clean(key).await;
    let app = build_app(store.store(), svc.clone()).await;

    let query = format!(
        "promptKey={key}&targetVersion=2.0.0&template={}&commitMsg={}&bizTags={}",
        percent_encode("query template"),
        percent_encode("query commit"),
        percent_encode("gamma")
    );
    let (status, body) = post_form(&app, &format!("/v3/admin/ai/prompt/draft?{query}"), "").await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");
    let row = version_row(&store, key, "2.0.0").await.expect("draft row");
    assert_eq!(row.description.as_deref(), Some("query commit"));
    assert_eq!(stored_template(&row), "query template");
    assert_eq!(
        resource_row(&store, key)
            .await
            .expect("prompt resource")
            .biz_tags
            .as_deref(),
        Some("gamma")
    );
    store.clean(key).await;
}

/// Batch 7 sends both alias; GA `targetVersion` wins over batata `version`.
#[actix_web::test]
async fn ga_target_version_takes_priority_over_batata_alias() {
    let (store, svc) = setup().await;
    let key = "ga-prompt-alias";
    store.clean(key).await;
    let app = build_app(store.store(), svc.clone()).await;

    let form = format!(
        "promptKey={key}&version=3.0.0&targetVersion=2.0.0&template={}",
        percent_encode("alias template")
    );
    let (status, body) = post_form(&app, "/v3/admin/ai/prompt/draft", &form).await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");
    assert!(
        version_row(&store, key, "2.0.0").await.is_some(),
        "targetVersion must win"
    );
    assert!(
        version_row(&store, key, "3.0.0").await.is_none(),
        "the batata alias must not create a second version"
    );
    store.clean(key).await;
}

/// `basedOnVersion` forks the base template and bumps the patch number.
#[actix_web::test]
async fn ga_based_on_version_forks_stored_template() {
    let (store, svc) = setup().await;
    let key = "ga-prompt-fork";
    store.clean(key).await;
    let app = build_app(store.store(), svc.clone()).await;
    let base = "/v3/admin/ai/prompt";

    let first = format!(
        "promptKey={key}&targetVersion=1.0.0&template={}",
        percent_encode("fork base template")
    );
    let (status, body) = post_form(&app, &format!("{base}/draft"), &first).await;
    assert_eq!(status, StatusCode::OK, "base draft failed: {body}");

    // Submitting clears the single editing slot so a second draft can start.
    let (status, body) = post_form(
        &app,
        &format!("{base}/submit"),
        &format!("promptKey={key}&version=1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "submit failed: {body}");

    // No template in the request: the fork must copy it from version 1.0.0.
    let fork = format!(
        "promptKey={key}&basedOnVersion=1.0.0&commitMsg={}",
        percent_encode("fork commit")
    );
    let (status, body) = post_form(&app, &format!("{base}/draft"), &fork).await;
    assert_eq!(status, StatusCode::OK, "fork draft failed: {body}");
    let row = version_row(&store, key, "1.0.1").await.expect("fork row");
    assert_eq!(
        stored_template(&row),
        "fork base template",
        "a fork must reuse the base version's template"
    );
    assert_eq!(row.description.as_deref(), Some("fork commit"));
    store.clean(key).await;
}

/// An unknown base version is a structured error, not a silent empty draft.
#[actix_web::test]
async fn ga_based_on_unknown_version_is_refused() {
    let (store, svc) = setup().await;
    let key = "ga-prompt-bad-base";
    store.clean(key).await;
    let app = build_app(store.store(), svc.clone()).await;

    let form = format!(
        "promptKey={key}&basedOnVersion=9.9.9&template={}",
        percent_encode("unused")
    );
    let (status, body) = post_form(&app, "/v3/admin/ai/prompt/draft", &form).await;
    assert!(
        status.is_client_error(),
        "an unknown base version must be refused, got {status}: {body}"
    );
    assert!(
        body.contains("Base version"),
        "the 400 must name the missing base version, not a generic template error: {body}"
    );
    assert!(
        version_row(&store, key, "9.9.10").await.is_none(),
        "no fork row may be written on a missing base"
    );
    store.clean(key).await;
}

/// With none of the GA fields present, behaviour is exactly the historical one.
#[actix_web::test]
async fn absent_ga_fields_preserve_historical_behaviour() {
    let (store, svc) = setup().await;
    let key = "ga-prompt-plain";
    store.clean(key).await;
    let app = build_app(store.store(), svc.clone()).await;

    let (status, body) = post_form(
        &app,
        "/v3/admin/ai/prompt/draft",
        &format!("promptKey={key}&version=1.0.0&template=plain"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");
    let row = version_row(&store, key, "1.0.0").await.expect("draft row");
    assert_eq!(row.description, None);
    assert_eq!(stored_template(&row), "plain");
    assert_eq!(
        resource_row(&store, key).await.expect("resource").biz_tags,
        None
    );
    store.clean(key).await;
}
