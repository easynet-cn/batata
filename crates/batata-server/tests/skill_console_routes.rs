//! Route-level test for the console skill API.
//!
//! The console layer is what the UI calls. These cover the four endpoints that
//! were missing there: `force-publish`, `redraft`, `upload/precheck` and
//! `upload/batch`. The two upload ones are `multipart/form-data` upstream,
//! unlike the JSON every other skill write uses.
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-server --test skill_console_routes -- --ignored --nocapture
//! ```

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{test, web, App};
use batata_ai::SkillOperationService;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_common::model::ai::skill::{Skill, SKILL_TYPE};
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use batata_persistence::{AiResourcePersistence, ExternalDbPersistService};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NAME: &str = "console-route-skill";
const NS: &str = "public";

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
        unimplemented!("not used by the console skill routes")
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

/// Build an actix app exposing the real console skill routes.
async fn build_app(
    store: Arc<ExternalDbPersistService>,
    svc: Arc<SkillOperationService>,
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

    let skills: Arc<dyn batata_common::SkillService> = svc;

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(skills))
            // Mounted exactly as the server does: console routes live under
            // `/v3/console`, and the module adds its own `/ai/...` prefix.
            .service(web::scope("/v3/console").service(batata_console::v3::ai_skill::routes())),
    )
    .await
}

async fn setup() -> (Arc<ExternalDbPersistService>, Arc<SkillOperationService>) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = Arc::new(SkillOperationService::new(store.clone(), None, false));
    (store, svc)
}

/// POST a JSON body, as the console UI does for every non-upload write.
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
    let body = test::read_body(resp).await;
    (status, String::from_utf8_lossy(&body).to_string())
}

/// POST one file as `multipart/form-data`, as the three upload endpoints require.
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
        b"Content-Disposition: form-data; name=\"file\"; filename=\"skills.zip\"\r\n",
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

/// The skill under test: a manifest carrying the name, which is what both
/// `create_draft` and the ZIP writer need.
fn test_skill() -> Skill {
    Skill {
        name: NAME.to_string(),
        namespace_id: NS.to_string(),
        description: Some("console route test".to_string()),
        skill_md: Some(format!("---\nname: {NAME}\n---\n")),
        ..Default::default()
    }
}

/// A one-skill ZIP, built with the same writer the service uses to export.
fn one_skill_zip() -> Vec<u8> {
    batata_common::model::ai::skill_zip::skill_to_zip_bytes(&test_skill()).expect("build zip")
}

async fn status_of(store: &ExternalDbPersistService, version: &str) -> Option<String> {
    store
        .ai_resource_version_find(NS, NAME, SKILL_TYPE, version)
        .await
        .expect("version lookup")
        .map(|row| row.status)
}

/// force-publish bypasses the review gate: it works where publish does not.
#[actix_web::test]
#[ignore]
async fn force_publish_bypasses_review() {
    let (store, svc) = setup().await;
    clean(&store).await;
    svc.create_draft(
        NS,
        NAME,
        None,
        Some("1.0.0"),
        Some(&test_skill()),
        "tester",
    )
    .await
    .expect("create draft");
    let app = build_app(store.clone(), svc.clone()).await;

    let base = "/v3/console/ai/skills";

    // A draft cannot be published normally.
    let (status, _) = post_json(
        &app,
        &format!("{base}/publish"),
        &format!("{{\"skillName\":\"{NAME}\",\"version\":\"1.0.0\"}}"),
    )
    .await;
    assert!(
        status.is_client_error(),
        "publishing an unsubmitted draft must be refused, got {status}"
    );

    // force-publish succeeds anyway — that is its whole purpose.
    let (status, body) = post_json(
        &app,
        &format!("{base}/force-publish"),
        &format!("{{\"skillName\":\"{NAME}\",\"version\":\"1.0.0\"}}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "force publish failed: {body}");
    assert_eq!(status_of(&store, "1.0.0").await.as_deref(), Some("online"));
    println!("ok: force-publish bypasses review");

    clean(&store).await;
}

/// redraft moves a published version back to draft.
#[actix_web::test]
#[ignore]
async fn redraft_returns_a_version_to_draft() {
    let (store, svc) = setup().await;
    clean(&store).await;
    svc.create_draft(
        NS,
        NAME,
        None,
        Some("1.0.0"),
        Some(&test_skill()),
        "tester",
    )
    .await
    .expect("create draft");
    let app = build_app(store.clone(), svc.clone()).await;

    let base = "/v3/console/ai/skills";
    let version_json = format!("{{\"skillName\":\"{NAME}\",\"version\":\"1.0.0\"}}");

    post_json(&app, &format!("{base}/submit"), &version_json).await;
    let (status, body) = post_json(&app, &format!("{base}/publish"), &version_json).await;
    assert_eq!(status, StatusCode::OK, "publish failed: {body}");
    assert_eq!(status_of(&store, "1.0.0").await.as_deref(), Some("online"));

    let (status, body) = post_json(&app, &format!("{base}/redraft"), &version_json).await;
    assert_eq!(status, StatusCode::OK, "redraft failed: {body}");
    assert_eq!(status_of(&store, "1.0.0").await.as_deref(), Some("draft"));
    println!("ok: redraft returns a version to draft");

    clean(&store).await;
}

/// upload/precheck reports what an upload would do, and persists nothing.
#[actix_web::test]
#[ignore]
async fn upload_precheck_reports_without_persisting() {
    let (store, _svc) = setup().await;
    clean(&store).await;
    let svc = Arc::new(SkillOperationService::new(store.clone(), None, false));
    let app = build_app(store.clone(), svc.clone()).await;

    let (status, body) =
        post_zip(&app, "/v3/console/ai/skills/upload/precheck", one_skill_zip()).await;
    assert_eq!(status, StatusCode::OK, "precheck failed: {body}");
    assert!(body.contains(NAME), "must name the skill: {body}");
    println!("ok: upload/precheck reports");

    // The point of a precheck: nothing was created.
    assert!(
        store
            .ai_resource_find(NS, NAME, SKILL_TYPE)
            .await
            .expect("lookup")
            .is_none(),
        "precheck must not create anything"
    );
    println!("ok: precheck persisted nothing");

    clean(&store).await;
}

/// Downloading a version counts it. The counter is what the console sorts by,
/// and it must survive being asked for twice.
#[actix_web::test]
#[ignore]
async fn download_counts_the_download() {
    let (store, svc) = setup().await;
    clean(&store).await;
    let app = build_app(store.clone(), svc.clone()).await;

    let (status, body) =
        post_zip(&app, "/v3/console/ai/skills/upload/batch", one_skill_zip()).await;
    assert_eq!(status, StatusCode::OK, "batch upload failed: {body}");

    let read_count = || async {
        store
            .ai_resource_find(NS, NAME, SKILL_TYPE)
            .await
            .expect("lookup")
            .map(|row| row.download_count)
    };
    assert_eq!(read_count().await, Some(0), "a fresh skill starts at zero");

    // The upload decides the version, so ask rather than assume.
    let version = store
        .ai_resource_version_list(NS, NAME, SKILL_TYPE)
        .await
        .expect("list versions")
        .into_iter()
        .next()
        .expect("the upload created a version")
        .version;
    let uri = format!("/v3/console/ai/skills/version/download?skillName={NAME}&version={version}");
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "download failed");

    assert_eq!(read_count().await, Some(1), "one download is counted");

    let req = test::TestRequest::get().uri(&uri).to_request();
    test::call_service(&app, req).await;
    assert_eq!(read_count().await, Some(2), "the counter accumulates");
    println!("ok: download counts the download");

    clean(&store).await;
}

/// upload/batch uploads the skills in the archive.
#[actix_web::test]
#[ignore]
async fn upload_batch_creates_the_skill() {
    let (store, svc) = setup().await;
    clean(&store).await;
    let app = build_app(store.clone(), svc.clone()).await;

    let (status, body) =
        post_zip(&app, "/v3/console/ai/skills/upload/batch", one_skill_zip()).await;
    assert_eq!(status, StatusCode::OK, "batch upload failed: {body}");
    assert!(body.contains(NAME), "must report the skill: {body}");

    assert!(
        store
            .ai_resource_find(NS, NAME, SKILL_TYPE)
            .await
            .expect("lookup")
            .is_some(),
        "the uploaded skill must exist"
    );
    println!("ok: upload/batch creates the skill");

    clean(&store).await;
}
