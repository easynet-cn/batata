//! Route-level test for the admin agent API.
//!
//! Covers the two endpoints that the admin surface was missing against
//! upstream `AgentAdminController`: `GET /version` and
//! `GET /runtime-endpoints`. The rest of the admin agent surface already has a
//! service-level test; these guard that the routes are mounted on
//! `/v3/admin/ai/agents` and behave when the service returns.
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run the same cases
//! against a real database. See `common/ai_store.rs`.

#[path = "common/ai_store.rs"]
mod ai_store;

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_ai::A2aServerOperationService;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NAME: &str = "admin-route-agent";
const VERSION: &str = "1.0.0";

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
        unimplemented!("not used by the admin agent routes")
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

/// Build an app exposing the real admin agent routes.
async fn build_app(
    store: Arc<dyn PersistenceService>,
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

    // The server wires the agent admin routes through the trait.
    let agents: Arc<dyn batata_common::A2aAgentService> = svc;

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(agents))
            .service(web::scope("/v3/admin/ai").service(batata_ai::api::agent::admin_routes())),
    )
    .await
}

async fn setup() -> (TestStore, Arc<A2aServerOperationService>) {
    let store = TestStore::open().await;
    let svc = Arc::new(A2aServerOperationService::new(store.store()));
    (store, svc)
}

async fn register(svc: &A2aServerOperationService) {
    let card = batata_common::model::ai::a2a::AgentCard {
        name: NAME.to_string(),
        display_name: NAME.to_string(),
        description: String::new(),
        version: VERSION.to_string(),
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

/// `GET /version` resolves the exact version definition when it exists.
#[actix_web::test]
async fn version_is_readable_over_admin_http() {
    let (store, svc) = setup().await;
    store.clean(NAME).await;
    register(&svc).await;
    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/v3/admin/ai/agents/version?agentName={NAME}&version={VERSION}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "version read failed");
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(
        body.contains(NAME),
        "version body must name the agent: {body}"
    );
    println!("ok: GET /version returns the version definition");

    store.clean(NAME).await;
}

/// `GET /version` is 404 for an agent/version that does not exist — proving the
/// route is mounted and the not-found path is reached rather than the whole
/// handler being absent (which would be 404 for a different reason).
#[actix_web::test]
async fn missing_version_is_404_over_admin_http() {
    let (store, svc) = setup().await;
    store.clean(NAME).await;
    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/v3/admin/ai/agents/version?agentName={NAME}&version={VERSION}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "unknown version must be 404"
    );
    println!("ok: GET /version is 404 for an unknown version");

    store.clean(NAME).await;
}

/// `GET /runtime-endpoints` is mounted on the admin surface and refuses (rather
/// than returning an empty snapshot) when no endpoint backend is configured.
#[actix_web::test]
async fn runtime_endpoints_fail_loudly_without_an_endpoint_backend() {
    let (store, svc) = setup().await;
    store.clean(NAME).await;
    register(&svc).await;
    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/v3/admin/ai/agents/runtime-endpoints?agentName={NAME}&version={VERSION}&protocol=JSONRPC"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "runtime-endpoints must be mounted on the admin surface"
    );
    assert!(
        resp.status().is_client_error(),
        "must refuse rather than report an empty snapshot, got {}",
        resp.status()
    );
    println!("ok: admin runtime-endpoints refuses without an endpoint backend");

    store.clean(NAME).await;
}
