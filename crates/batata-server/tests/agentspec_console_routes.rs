//! Route-level test for the console AgentSpec API.
//!
//! Covers the two lifecycle endpoints that were missing from the console layer:
//! `force-publish` and `redraft`.
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-server --test agentspec_console_routes -- --ignored --nocapture
//! ```

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{test, web, App};
use batata_ai::AgentSpecOperationService;
use batata_common::model::ai::agentspec::{AgentSpec, AGENTSPEC_TYPE};
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use batata_persistence::{AiResourcePersistence, ExternalDbPersistService};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NAME: &str = "console-route-agentspec";
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

/// Cluster manager stub. The AgentSpec routes never touch cluster state.
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
        unimplemented!("not used by the console agentspec routes")
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

/// Build an actix app exposing the real console AgentSpec routes.
async fn build_app(
    store: Arc<ExternalDbPersistService>,
    svc: Arc<AgentSpecOperationService>,
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

    let specs: Arc<dyn batata_common::AgentSpecService> = svc;

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(specs))
            // Mounted exactly as the server does.
            .service(web::scope("/v3/console").service(batata_console::v3::ai_agentspec::routes())),
    )
    .await
}

async fn setup() -> (Arc<ExternalDbPersistService>, Arc<AgentSpecOperationService>) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = Arc::new(AgentSpecOperationService::new(store.clone(), None, false));
    (store, svc)
}

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

async fn status_of(store: &ExternalDbPersistService, version: &str) -> Option<String> {
    store
        .ai_resource_version_find(NS, NAME, AGENTSPEC_TYPE, version)
        .await
        .expect("version lookup")
        .map(|row| row.status)
}

/// force-publish works where publish does not — the review gate is the
/// difference, so both directions are asserted.
#[actix_web::test]
#[ignore]
async fn force_publish_bypasses_review() {
    let (store, svc) = setup().await;
    clean(&store).await;

    let spec = AgentSpec {
        name: NAME.to_string(),
        namespace_id: NS.to_string(),
        description: Some("console route test".to_string()),
        content: Some("main manifest".to_string()),
        ..Default::default()
    };
    svc.create_draft(NS, NAME, None, Some("1.0.0"), Some(&spec), "tester")
        .await
        .expect("create draft");
    let app = build_app(store.clone(), svc.clone()).await;

    let base = "/v3/console/ai/agentspecs";
    let version_json = format!("{{\"agentSpecName\":\"{NAME}\",\"version\":\"1.0.0\"}}");

    let (status, _) = post_json(&app, &format!("{base}/publish"), &version_json).await;
    assert!(
        status.is_client_error(),
        "publishing an unsubmitted draft must be refused, got {status}"
    );

    let (status, body) = post_json(&app, &format!("{base}/force-publish"), &version_json).await;
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

    let spec = AgentSpec {
        name: NAME.to_string(),
        namespace_id: NS.to_string(),
        content: Some("main manifest".to_string()),
        ..Default::default()
    };
    svc.create_draft(NS, NAME, None, Some("1.0.0"), Some(&spec), "tester")
        .await
        .expect("create draft");
    let app = build_app(store.clone(), svc.clone()).await;

    let base = "/v3/console/ai/agentspecs";
    let version_json = format!("{{\"agentSpecName\":\"{NAME}\",\"version\":\"1.0.0\"}}");

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
