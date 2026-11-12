//! Route-level guard for `GET /v3/admin/ai/a2a` version consumption.
//!
//! `AgentDetailQuery.version` was declared but the admin handler hard-coded
//! `None` when calling `A2aAgentService::get_agent_card`. With `None` the
//! config-backed service falls back to `latestPublishedVersion`, so a request
//! for an unknown version still returns 200. After the fix the handler passes
//! `q.version.as_deref()`, the service looks up that exact version and returns
//! 404 for a miss. These tests drive the real production route with the real
//! `A2aServerOperationService`.
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run against a real
//! database. See `common/ai_store.rs`.

#[path = "common/ai_store.rs"]
mod ai_store;

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_ai::A2aServerOperationService;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server::api::ai::AgentRegistry;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NAME: &str = "a2a-admin-version-agent";
const VERSION: &str = "1.0.0";

/// Cluster manager stub. The A2A admin read route never touches cluster state.
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
        unimplemented!("not used by the admin a2a read route")
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

/// Build an app exposing the legacy admin A2A routes (`a2a::routes()`), with
/// the config-backed `A2aAgentService` wired in so version lookups are real.
async fn build_app(
    store: Arc<dyn PersistenceService>,
    svc: Option<Arc<A2aServerOperationService>>,
) -> (
    impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    Arc<AgentRegistry>,
) {
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

    let agents: Option<Arc<dyn batata_common::A2aAgentService>> =
        svc.map(|service| service as Arc<dyn batata_common::A2aAgentService>);
    let registry: Arc<AgentRegistry> = Arc::new(AgentRegistry::new());

    let app = App::new()
        .app_data(web::Data::from(app_state))
        .app_data(web::Data::new(registry.clone()));
    let app = match agents {
        Some(agents) => app.app_data(web::Data::new(agents)),
        None => app,
    };

    let service = test::init_service(app.service(
        web::scope("/v3/admin/ai").service(batata_server::api::v3::admin::ai::a2a::routes()),
    ))
    .await;
    (service, registry)
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

/// The `version` query must be consumed by the handler:
///
/// * asking for the registered version succeeds (200);
/// * asking for a version that does not exist yields 404, because the service
///   actually looks it up. With the old hard-coded `None` the service falls
///   back to `latestPublishedVersion` and both requests return 200, so this
///   assertion is the mutant guard for the one-line fix.
#[actix_web::test]
async fn admin_a2a_get_consumes_version_query() {
    let (store, svc) = setup().await;
    store.clean(NAME).await;
    register(&svc).await;
    let app = build_app(store.store(), Some(svc.clone())).await.0;

    let ok_req = test::TestRequest::get()
        .uri(&format!(
            "/v3/admin/ai/a2a?agentName={NAME}&version={VERSION}"
        ))
        .to_request();
    let ok_resp = test::call_service(&app, ok_req).await;
    assert_eq!(
        ok_resp.status(),
        StatusCode::OK,
        "the registered version must be readable"
    );

    let missing_req = test::TestRequest::get()
        .uri(&format!(
            "/v3/admin/ai/a2a?agentName={NAME}&version=9.9.9-does-not-exist"
        ))
        .to_request();
    let missing_resp = test::call_service(&app, missing_req).await;
    assert_eq!(
        missing_resp.status(),
        StatusCode::NOT_FOUND,
        "an unknown version must be looked up and reported as 404"
    );

    store.clean(NAME).await;
}

/// GA's `A2aMaintainerServiceImpl.listAllVersionOfAgent` decodes
/// `Result<List<a2a.AgentVersionDetail>>`. Batch 12 made this handler share
/// `A2aAgentService::list_versions` (an ARD `Page`), so `Result.data` became a
/// JSON object and the Java SDK threw `NacosDeserializationException`. This is
/// the route-level guard for the array projection; the same projection is used
/// by the console `/v3/console/ai/a2a/version/list` endpoint.
#[actix_web::test]
async fn admin_a2a_version_list_returns_a_ga_array() {
    let (store, svc) = setup().await;
    store.clean(NAME).await;
    register(&svc).await;
    let app = build_app(store.store(), Some(svc.clone())).await.0;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/v3/admin/ai/a2a/version/list?agentName={NAME}&namespaceId=public"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0, "unexpected envelope: {body}");

    let data = &body["data"];
    assert!(
        data.is_array(),
        "GA expects Result.data to be an array: {body}"
    );
    assert!(
        data.get("pageItems").is_none() && data.get("totalCount").is_none(),
        "an ARD Page leaked into the legacy A2A endpoint: {body}"
    );
    let items = data.as_array().expect("data is an array");
    assert!(!items.is_empty(), "the registered version must be listed");
    assert_eq!(items[0]["version"], VERSION);
    assert!(
        items[0].get("isLatest").is_some() && items[0].get("latest").is_some(),
        "both GA Jackson and Rust client spellings must be present: {items:?}"
    );

    store.clean(NAME).await;
}

/// Without the config-backed service the handler falls back to the in-memory
/// registry. That branch must also answer with the GA array envelope, not an
/// ARD Page and not a raw `RegisteredAgent` list.
#[actix_web::test]
async fn admin_a2a_version_list_without_service_stays_a_ga_array() {
    let (store, svc) = setup().await;
    store.clean(NAME).await;
    register(&svc).await;
    let (app, registry) = build_app(store.store(), None).await;
    registry
        .register(batata_common::model::ai::a2a::AgentRegistrationRequest {
            card: batata_common::model::ai::a2a::AgentCard {
                name: NAME.to_string(),
                version: VERSION.to_string(),
                ..Default::default()
            },
            namespace: "public".to_string(),
        })
        .expect("register into the fallback registry");

    let req = test::TestRequest::get()
        .uri(&format!(
            "/v3/admin/ai/a2a/version/list?agentName={NAME}&namespaceId=public"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0, "unexpected envelope: {body}");
    assert!(
        body["data"].is_array(),
        "the fallback must not return a paged object: {body}"
    );
    assert!(
        body["data"].get("pageItems").is_none(),
        "an ARD Page leaked into the fallback: {body}"
    );
    let items = body["data"].as_array().expect("data is an array");
    assert_eq!(items.len(), 1, "the fallback registry entry must be listed");
    assert_eq!(items[0]["version"], VERSION);
    assert!(
        items[0].get("isLatest").is_some() && items[0].get("latest").is_some(),
        "fallback must emit GA fields, not a raw RegisteredAgent: {items:?}"
    );

    store.clean(NAME).await;
}
