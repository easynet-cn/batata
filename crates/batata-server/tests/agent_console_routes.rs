//! Route-level test for the console agent API.
//!
//! Covers what the service-level test cannot: that the lifecycle endpoints are
//! registered on the console scope (`/v3/console/ai/agents`, distinct from the
//! A2A registry's `/ai/a2a`), and that writes accept the form-encoded bodies
//! the Nacos console UI sends.
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-server --test agent_console_routes -- --ignored --nocapture
//! ```

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{test, web, App};
use batata_ai::A2aServerOperationService;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use batata_persistence::ExternalDbPersistService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NAME: &str = "console-route-agent";

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

/// Cluster manager stub. The agent routes never touch cluster state.
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
        unimplemented!("not used by the console agent routes")
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

/// Build an actix app exposing the real console agent routes.
async fn build_app(
    store: Arc<ExternalDbPersistService>,
    svc: Arc<A2aServerOperationService>,
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

    let agents: Arc<dyn batata_common::A2aAgentService> = svc;

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(agents))
            // Mounted exactly as the server does: console routes live under
            // `/v3/console`, and the module adds its own `/ai/...` prefix.
            .service(web::scope("/v3/console").service(batata_console::v3::ai_agent::routes())),
    )
    .await
}

async fn setup() -> (Arc<ExternalDbPersistService>, Arc<A2aServerOperationService>) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = Arc::new(A2aServerOperationService::new(store.clone()));
    (store, svc)
}

/// Register the agent under test.
///
/// Unlike Skill and Prompt, an agent draft is a *new version of an existing
/// agent*, so the agent must exist before `/draft` can succeed.
async fn register(svc: &A2aServerOperationService) {
    let card = batata_common::model::ai::a2a::AgentCard {
        name: NAME.to_string(),
        display_name: NAME.to_string(),
        description: String::new(),
        version: "1.0.0".to_string(),
        url: String::new(),
        protocol_version: String::new(),
        capabilities: Default::default(),
        skills: vec![],
        default_input_modes: vec![],
        default_output_modes: vec![],
        preferred_transport: None,
        provider: None,
        documentation_url: None,
        icon_url: None,
        supports_authenticated_extended_card: None,
        metadata: std::collections::HashMap::new(),
        tags: vec![],
        ..Default::default()
    };
    svc.register_agent(&card, "public", "manual")
        .await
        .expect("register agent");
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

/// The agent lifecycle is reachable on `/v3/console/ai/agents`, and the review
/// gate is enforced.
#[actix_web::test]
#[ignore]
async fn draft_lifecycle_over_console_http() {
    let (store, svc) = setup().await;
    clean(&store).await;
    register(&svc).await;
    let app = build_app(store.clone(), svc.clone()).await;

    let base = "/v3/console/ai/agents";

    let (status, body) = post_form(
        &app,
        &format!("{base}/draft"),
        &format!("agentName={NAME}&version=2.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");
    println!("ok: POST draft (form-encoded)");

    // A draft cannot be published before it is submitted.
    let (status, _) = post_form(
        &app,
        &format!("{base}/publish"),
        &format!("agentName={NAME}&version=2.0.0"),
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
        &format!("agentName={NAME}&version=2.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "submit failed: {body}");

    let (status, body) = post_form(
        &app,
        &format!("{base}/publish"),
        &format!("agentName={NAME}&version=2.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "publish failed: {body}");
    println!("ok: submit then publish");

    clean(&store).await;
}

/// Versions are listed on the console scope.
#[actix_web::test]
#[ignore]
async fn versions_are_listable_over_console_http() {
    let (store, svc) = setup().await;
    clean(&store).await;
    register(&svc).await;
    let app = build_app(store.clone(), svc.clone()).await;

    let (status, body) = post_form(
        &app,
        "/v3/console/ai/agents/draft",
        &format!("agentName={NAME}&version=3.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create draft failed: {body}");

    let req = test::TestRequest::get()
        .uri(&format!("/v3/console/ai/agents/versions?agentName={NAME}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(body.contains("3.0.0"), "must list the draft: {body}");
    assert!(body.contains("1.0.0"), "must list the registered version: {body}");
    println!("ok: versions listed");

    clean(&store).await;
}
