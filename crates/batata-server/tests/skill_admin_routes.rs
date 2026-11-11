//! Route-level tests for GA ADMIN Skill `commitMsg`.
//!
//! Nacos 3.3.0 GA binds `commitMsg` on `POST/PUT /v3/admin/ai/skills/draft`
//! and `POST /v3/admin/ai/skills/upload`, and stores it as the version row's
//! `versionDesc` (`SkillAdminController.java:201`,
//! `SkillOperationServiceImpl.java:1246,1668`). These tests exercise the real
//! mounted admin scope, both parameter channels (urlencoded form body and URL
//! query) plus the multipart ZIP channel, and assert the stored version row.
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run the same cases
//! against a real database. See `common/ai_store.rs`.

#[path = "common/ai_store.rs"]
mod ai_store;

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_ai::SkillOperationService;
use batata_common::model::ai::skill::{
    SKILL_TYPE, Skill, VERSION_STATUS_DRAFT, VERSION_STATUS_ONLINE,
};
use batata_common::model::ai::skill_zip;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NS: &str = "public";
const BASE: &str = "/v3/admin/ai/skills";

/// Cluster manager stub. The skill routes never touch cluster state.
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
        unimplemented!("not used by the admin skill routes")
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

async fn build_app(
    store: Arc<dyn PersistenceService>,
    svc: Arc<SkillOperationService>,
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

    let skills: Arc<dyn batata_common::SkillService> = svc;

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(skills))
            .service(web::scope("/v3/admin/ai").service(batata_ai::skill_admin_routes())),
    )
    .await
}

async fn setup() -> (TestStore, Arc<SkillOperationService>) {
    let store = TestStore::open().await;
    let svc = Arc::new(SkillOperationService::new(store.store(), None, false));
    (store, svc)
}

/// Percent-encode a form value so JSON survives the body / query string.
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

fn skill_card(name: &str, description: &str) -> String {
    serde_json::to_string(&Skill {
        name: name.to_string(),
        namespace_id: NS.to_string(),
        description: Some(description.to_string()),
        skill_md: Some(format!("---\nname: {name}\n---\n")),
        ..Default::default()
    })
    .expect("serialize skill card")
}

fn draft_form(name: &str, description: &str) -> String {
    format!(
        "namespaceId={NS}&skillName={name}&skillCard={}&targetVersion=1.0.0",
        percent_encode(&skill_card(name, description))
    )
}

async fn send<S>(
    app: &S,
    method: &str,
    uri: &str,
    content_type: &str,
    body: Vec<u8>,
) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let req = match method {
        "PUT" => test::TestRequest::put(),
        _ => test::TestRequest::post(),
    }
    .uri(uri)
    .insert_header(("content-type", content_type.to_string()))
    .set_payload(body)
    .to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    let body = test::read_body(resp).await;
    (status, String::from_utf8_lossy(&body).to_string())
}

async fn version_description(store: &TestStore, name: &str, version: &str) -> Option<String> {
    store
        .store()
        .ai_resource_version_find(NS, name, SKILL_TYPE, version)
        .await
        .expect("version lookup")
        .and_then(|row| row.description)
}

/// GA Java sends `POST /skills/draft` as an urlencoded form body.
#[actix_web::test]
async fn create_draft_form_channel_stores_commit_msg() {
    let (store, svc) = setup().await;
    let name = "admin-form-commit";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let form = format!(
        "{}&commitMsg={}",
        draft_form(name, "card description"),
        percent_encode("ga form commit")
    );
    let (status, body) = send(
        &app,
        "POST",
        &format!("{BASE}/draft"),
        "application/x-www-form-urlencoded",
        form.into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");
    assert_eq!(
        version_description(&store, name, "1.0.0").await.as_deref(),
        Some("ga form commit"),
        "commitMsg must become the version-level description"
    );
    store.clean(name).await;
}

/// Spring `@RequestParam` also accepts the query string; `AdminForm` must
/// resolve the same `commitMsg` key there (Rust SDK / console compatibility).
#[actix_web::test]
async fn create_draft_query_channel_stores_commit_msg() {
    let (store, svc) = setup().await;
    let name = "admin-query-commit";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let query = format!(
        "{}&commitMsg={}",
        draft_form(name, "card description"),
        percent_encode("ga query commit")
    );
    let (status, body) = send(
        &app,
        "POST",
        &format!("{BASE}/draft?{query}"),
        "application/x-www-form-urlencoded",
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");
    assert_eq!(
        version_description(&store, name, "1.0.0").await.as_deref(),
        Some("ga query commit"),
        "the query channel must resolve commitMsg too"
    );
    store.clean(name).await;
}

/// Absent `commitMsg` keeps batata's historical skill-card description.
#[actix_web::test]
async fn create_draft_without_commit_msg_keeps_card_description() {
    let (store, svc) = setup().await;
    let name = "admin-no-commit";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("{BASE}/draft"),
        "application/x-www-form-urlencoded",
        draft_form(name, "card description").into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");
    assert_eq!(
        version_description(&store, name, "1.0.0").await.as_deref(),
        Some("card description"),
        "absent commitMsg must not change the existing behaviour"
    );
    store.clean(name).await;
}

/// `PUT /skills/draft` updates the existing draft's version description.
#[actix_web::test]
async fn update_draft_form_channel_stores_commit_msg() {
    let (store, svc) = setup().await;
    let name = "admin-update-commit";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("{BASE}/draft"),
        "application/x-www-form-urlencoded",
        draft_form(name, "card description").into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");
    assert_eq!(
        version_description(&store, name, "1.0.0").await.as_deref(),
        Some("card description")
    );

    let form = format!(
        "namespaceId={NS}&skillName={name}&skillCard={}&commitMsg={}",
        percent_encode(&skill_card(name, "updated card description")),
        percent_encode("ga update commit")
    );
    let (status, body) = send(
        &app,
        "PUT",
        &format!("{BASE}/draft"),
        "application/x-www-form-urlencoded",
        form.into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "update draft failed: {body}");
    assert_eq!(
        version_description(&store, name, "1.0.0").await.as_deref(),
        Some("ga update commit"),
        "PUT commitMsg must replace the version description"
    );
    store.clean(name).await;
}

/// `PUT /skills/draft` must also resolve `commitMsg` from the query channel.
#[actix_web::test]
async fn update_draft_query_channel_stores_commit_msg() {
    let (store, svc) = setup().await;
    let name = "admin-update-query-commit";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("{BASE}/draft"),
        "application/x-www-form-urlencoded",
        draft_form(name, "card description").into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");

    let query = format!(
        "namespaceId={NS}&skillName={name}&skillCard={}&commitMsg={}",
        percent_encode(&skill_card(name, "updated card description")),
        percent_encode("ga query update commit")
    );
    let (status, body) = send(
        &app,
        "PUT",
        &format!("{BASE}/draft?{query}"),
        "application/x-www-form-urlencoded",
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "update draft failed: {body}");
    assert_eq!(
        version_description(&store, name, "1.0.0").await.as_deref(),
        Some("ga query update commit")
    );
    store.clean(name).await;
}

/// A one-skill ZIP whose front matter carries a description.
fn one_skill_zip(name: &str, description: &str) -> Vec<u8> {
    skill_zip::skill_to_zip_bytes(&Skill {
        name: name.to_string(),
        namespace_id: NS.to_string(),
        description: Some(description.to_string()),
        skill_md: Some(format!(
            "---\nname: {name}\ndescription: {description}\n---\n# Body\n"
        )),
        ..Default::default()
    })
    .expect("build zip")
}

async fn post_zip<S>(app: &S, uri: &str, bytes: Vec<u8>) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    const BOUNDARY: &str = "----batata-test-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"skill.zip\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/zip\r\n\r\n");
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());

    let req = test::TestRequest::post()
        .uri(uri)
        .insert_header((
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    let body = test::read_body(resp).await;
    (status, String::from_utf8_lossy(&body).to_string())
}

/// GA's ZIP upload puts every scalar parameter in the URL query
/// (`ClientHttpProxy.executeMultipartPost` line 264) and the file in the body.
#[actix_web::test]
async fn upload_query_channel_stores_commit_msg() {
    let (store, svc) = setup().await;
    let name = "admin-upload-commit";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let uri = format!(
        "{BASE}/upload?namespaceId={NS}&commitMsg={}",
        percent_encode("ga upload commit")
    );
    let (status, body) = post_zip(&app, &uri, one_skill_zip(name, "zip description")).await;
    assert_eq!(status, StatusCode::OK, "upload failed: {body}");
    assert_eq!(
        version_description(&store, name, "0.0.1").await.as_deref(),
        Some("ga upload commit"),
        "upload commitMsg must become the version-level description"
    );
    store.clean(name).await;
}

/// Without `commitMsg` the ZIP front-matter description is still used.
#[actix_web::test]
async fn upload_without_commit_msg_keeps_zip_description() {
    let (store, svc) = setup().await;
    let name = "admin-upload-no-commit";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let (status, body) = post_zip(
        &app,
        &format!("{BASE}/upload?namespaceId={NS}"),
        one_skill_zip(name, "zip description"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "upload failed: {body}");
    assert_eq!(
        version_description(&store, name, "0.0.1").await.as_deref(),
        Some("zip description"),
        "absent commitMsg must keep the ZIP description"
    );
    store.clean(name).await;
}

async fn version_status(store: &TestStore, name: &str, version: &str) -> Option<String> {
    store
        .store()
        .ai_resource_version_find(NS, name, SKILL_TYPE, version)
        .await
        .expect("version lookup")
        .map(|row| row.status)
}

/// GA `targetVersion` overrides the auto-bumped version for the uploaded draft.
#[actix_web::test]
async fn upload_target_version_override() {
    let (store, svc) = setup().await;
    let name = "admin-upload-target";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let uri = format!("{BASE}/upload?namespaceId={NS}&targetVersion=2.3.4");
    let (status, body) = post_zip(&app, &uri, one_skill_zip(name, "zip description")).await;
    assert_eq!(status, StatusCode::OK, "upload failed: {body}");
    assert_eq!(
        version_description(&store, name, "2.3.4").await.as_deref(),
        Some("zip description"),
        "targetVersion must pin the uploaded draft version"
    );
    assert!(
        version_description(&store, name, "0.0.1").await.is_none(),
        "the auto-bumped version must not be created when targetVersion is given"
    );
    store.clean(name).await;
}

/// GA `uploadAction=OVERWRITE_DRAFT` replaces an existing editing draft.
#[actix_web::test]
async fn upload_action_overwrite_draft() {
    let (store, svc) = setup().await;
    let name = "admin-upload-overwrite";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    // First upload creates the editing draft at 0.0.1.
    let (status, body) = post_zip(
        &app,
        &format!("{BASE}/upload?namespaceId={NS}"),
        one_skill_zip(name, "zip description"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "first upload failed: {body}");
    assert_eq!(
        version_description(&store, name, "0.0.1").await.as_deref(),
        Some("zip description")
    );

    // Overwrite the existing editing draft with an explicit action.
    let uri = format!("{BASE}/upload?namespaceId={NS}&uploadAction=OVERWRITE_DRAFT");
    let (status, body) = post_zip(&app, &uri, one_skill_zip(name, "zip description")).await;
    assert_eq!(status, StatusCode::OK, "overwrite upload failed: {body}");
    // The stale 0.0.1 draft is deleted; the new draft is the next patch version.
    assert!(
        version_description(&store, name, "0.0.1").await.is_none(),
        "the overwritten draft must be deleted"
    );
    assert!(
        version_description(&store, name, "0.0.2").await.is_some(),
        "the overwrite must create a new editing draft"
    );
    store.clean(name).await;
}

/// GA `uploadAction=CREATE_DRAFT` rejects when a working version already exists.
#[actix_web::test]
async fn upload_action_create_draft_rejects_existing() {
    let (store, svc) = setup().await;
    let name = "admin-upload-create-reject";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let (status, body) = post_zip(
        &app,
        &format!("{BASE}/upload?namespaceId={NS}"),
        one_skill_zip(name, "zip description"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "first upload failed: {body}");

    let uri = format!("{BASE}/upload?namespaceId={NS}&uploadAction=CREATE_DRAFT");
    let (status, body) = post_zip(&app, &uri, one_skill_zip(name, "zip description")).await;
    assert_ne!(
        status,
        StatusCode::OK,
        "CREATE_DRAFT must reject an existing working version"
    );
    assert!(
        body.contains("editing or reviewing version"),
        "rejection must name the conflicting working version: {body}"
    );
    store.clean(name).await;
}

/// GA rejects an unsupported `uploadAction` value.
#[actix_web::test]
async fn upload_action_unsupported_is_rejected() {
    let (store, svc) = setup().await;
    let name = "admin-upload-bogus-action";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let uri = format!("{BASE}/upload?namespaceId={NS}&uploadAction=BOGUS_ACTION");
    let (status, body) = post_zip(&app, &uri, one_skill_zip(name, "zip description")).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "unsupported action must 400: {body}"
    );
    assert!(
        body.contains("Unsupported uploadAction"),
        "error must name the unsupported action: {body}"
    );
    store.clean(name).await;
}

/// GA `autoPublishIfNew=true` force-publishes the first upload of a brand-new skill.
#[actix_web::test]
async fn upload_auto_publish_if_new_publishes() {
    let (store, svc) = setup().await;
    let name = "admin-upload-autopublish";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let uri = format!("{BASE}/upload?namespaceId={NS}&autoPublishIfNew=true");
    let (status, body) = post_zip(&app, &uri, one_skill_zip(name, "zip description")).await;
    assert_eq!(status, StatusCode::OK, "upload failed: {body}");
    assert_eq!(
        version_status(&store, name, "0.0.1").await.as_deref(),
        Some(VERSION_STATUS_ONLINE),
        "autoPublishIfNew must move the first version to online"
    );
    store.clean(name).await;
}

/// GA `autoPublishIfNew=false` (the default) leaves the first upload as a draft.
#[actix_web::test]
async fn upload_auto_publish_if_new_keeps_draft() {
    let (store, svc) = setup().await;
    let name = "admin-upload-keep-draft";
    store.clean(name).await;
    let app = build_app(store.store(), svc).await;

    let (status, body) = post_zip(
        &app,
        &format!("{BASE}/upload?namespaceId={NS}&autoPublishIfNew=false"),
        one_skill_zip(name, "zip description"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "upload failed: {body}");
    assert_eq!(
        version_status(&store, name, "0.0.1").await.as_deref(),
        Some(VERSION_STATUS_DRAFT),
        "autoPublishIfNew=false must keep the first version as a draft"
    );
    store.clean(name).await;
}
