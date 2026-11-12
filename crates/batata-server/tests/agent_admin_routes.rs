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

/// Build a service wired to a real in-memory Naming backend, mirroring the
/// production `AIServices::with_persistence` injection.
async fn setup_with_endpoints() -> (
    TestStore,
    Arc<A2aServerOperationService>,
    Arc<batata_ai::AiEndpointService>,
) {
    let store = TestStore::open().await;
    let naming: Arc<dyn batata_api::naming::NamingServiceProvider> =
        Arc::new(batata_naming::NamingService::new());
    let endpoints = Arc::new(batata_ai::AiEndpointService::new(naming));
    let svc = Arc::new(
        A2aServerOperationService::new(store.store()).with_endpoint_service(endpoints.clone()),
    );
    (store, svc, endpoints)
}

async fn body_value(resp: actix_web::dev::ServiceResponse) -> serde_json::Value {
    let bytes = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&bytes).to_string();
    serde_json::from_str(&body).unwrap_or_else(|e| panic!("bad JSON body {body}: {e}"))
}

/// GA (`AgentRuntimeEndpointAdminApiOpenApiITCase#testMissingDefinitionReturnsDefaultNamespaceEmptySnapshot`)
/// answers a successful empty snapshot for an unknown agent: `version` is
/// optional, the namespace defaults to `public`, and `data` is a flat
/// `RuntimeEndpointSnapshot` with one RUNTIME endpoint set.
#[actix_web::test]
async fn runtime_endpoints_without_backend_is_a_ga_empty_snapshot() {
    let (store, svc) = setup().await;
    store.clean(NAME).await;
    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/v3/admin/ai/agents/runtime-endpoints?agentName={NAME}&protocol=JSONRPC"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "missing agent must be 200");
    let body = body_value(resp).await;
    let data = &body["data"];
    assert_eq!(data["namespaceId"], "public");
    assert_eq!(data["agentName"], NAME);
    assert!(data.get("version").is_none(), "absent version is omitted");
    assert!(
        data.get("runtimeEndpointSnapshot").is_none(),
        "admin data must not be the console wrapper"
    );
    assert!(data.get("namingServiceRef").is_none());
    assert_eq!(data["callInterface"]["protocol"], "JSONRPC");
    let sets = data["callInterface"]["endpointSets"]
        .as_array()
        .expect("one endpoint set");
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0]["source"], "RUNTIME");
    assert!(sets[0]["lastUpdatedTime"].as_i64().unwrap() > 0);
    assert_eq!(sets[0]["endpoints"].as_array().unwrap().len(), 0);
    println!("ok: admin runtime-endpoints is a GA-shaped empty snapshot");

    store.clean(NAME).await;
}

/// With the Naming backend wired, the same route returns the live instances as
/// GA `Endpoint` values rather than batata's flat address/port view.
#[actix_web::test]
async fn runtime_endpoints_with_backend_returns_live_ga_endpoints() {
    let (store, svc, endpoints) = setup_with_endpoints().await;
    store.clean(NAME).await;
    endpoints
        .register_agent_endpoint_batch(
            "client-1",
            "public",
            NAME,
            VERSION,
            None,
            &[("10.0.0.7".to_string(), 8080)],
        )
        .expect("register endpoint batch");
    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/v3/admin/ai/agents/runtime-endpoints?agentName={NAME}&protocol=JSONRPC&version={VERSION}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_value(resp).await;
    let data = &body["data"];
    assert_eq!(data["version"], VERSION);
    let set = &data["callInterface"]["endpointSets"][0];
    assert_eq!(set["source"], "RUNTIME");
    assert!(set["lastUpdatedTime"].as_i64().unwrap() > 0);
    let ep = &set["endpoints"][0];
    assert_eq!(ep["uri"], "http://10.0.0.7:8080");
    assert_eq!(ep["transport"], "JSONRPC");
    assert_eq!(ep["weight"], 1.0);
    assert_eq!(ep["healthy"], true);
    assert_eq!(ep["enabled"], true);
    assert_eq!(ep["bindings"][0]["runtimeVersion"], VERSION);
    assert!(ep.get("state").is_none());
    println!("ok: admin runtime-endpoints serves live GA endpoints");

    store.clean(NAME).await;
}

/// GA validates `protocol` as required (`AgentRuntimeEndpointForm.validate`).
#[actix_web::test]
async fn runtime_endpoints_requires_protocol() {
    let (store, svc) = setup().await;
    store.clean(NAME).await;
    let app = build_app(store.store(), svc.clone()).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/v3/admin/ai/agents/runtime-endpoints?agentName={NAME}&version={VERSION}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error(), "protocol is required");
    let body = body_value(resp).await;
    assert!(
        body["data"]
            .as_str()
            .unwrap_or_default()
            .contains("protocol"),
        "error must name the missing field: {body}"
    );
    println!("ok: admin runtime-endpoints rejects a missing protocol");

    store.clean(NAME).await;
}
