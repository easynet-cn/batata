//! Route-level test for the console prompt API.
//!
//! The console layer is what the UI actually calls, so this covers the part the
//! service-level test cannot: that the endpoints are registered on the console
//! scope, and that they accept the form-encoded bodies the Nacos console UI
//! sends (reads and deletes use query parameters instead).
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-server --test prompt_console_routes -- --ignored --nocapture
//! ```

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{test, web, App};
use batata_ai::PromptOperationService;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use batata_persistence::ExternalDbPersistService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const KEY: &str = "console-route-prompt";

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

async fn clean(store: &ExternalDbPersistService) {
    let db = store.db();
    ai_resource_version::Entity::delete_many()
        .filter(ai_resource_version::Column::Name.eq(KEY))
        .exec(db)
        .await
        .expect("clean versions");
    ai_resource::Entity::delete_many()
        .filter(ai_resource::Column::Name.eq(KEY))
        .exec(db)
        .await
        .expect("clean resources");
}

/// Build an actix app exposing the real console prompt routes.
async fn build_app(
    store: Arc<ExternalDbPersistService>,
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
        health_check_manager: None,
        raft_node: None,
        server_status: Arc::new(batata_server_common::ServerStatusManager::new()),
        control_plugin: None,
        encryption_service: None,
        plugin_state_providers: vec![],
        plugin_manager: None,
        log_level_setter: None,
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

async fn setup() -> (Arc<ExternalDbPersistService>, Arc<PromptOperationService>) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = Arc::new(PromptOperationService::new(store.clone()));
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
#[ignore]
async fn draft_lifecycle_over_console_http() {
    let (store, svc) = setup().await;
    clean(&store).await;
    let app = build_app(store.clone(), svc.clone()).await;

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

    clean(&store).await;
}

/// The governance view exposes the state the UI needs to drive the lifecycle.
#[actix_web::test]
#[ignore]
async fn governance_reports_lifecycle_state() {
    let (store, svc) = setup().await;
    clean(&store).await;
    let app = build_app(store.clone(), svc.clone()).await;

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
    assert!(body.contains("editingVersion"), "must report the draft: {body}");
    assert!(body.contains("2.0.0"), "must list the version: {body}");
    assert!(body.contains("versionDetails"), "must carry versionDetails: {body}");
    assert!(body.contains("\"draft\""), "version must carry its status: {body}");
    println!("ok: governance reports lifecycle state");

    clean(&store).await;
}
