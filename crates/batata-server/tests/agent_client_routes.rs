//! Route-level test for the RAD agent client API (`/v3/client/ai/agents`).
//!
//! Upstream anchors: `com.alibaba.nacos.ai.controller.AgentClientController`
//! registers five endpoints on `Constants.Agent.CLIENT_PATH`. Batata ports them
//! in `batata-ai/src/api/agent_client.rs`, and until now only the *version
//! range* arithmetic behind `discover` had unit tests — the handlers
//! themselves, the parameter names on the wire and the register/discover/
//! deregister round trip had none.
//!
//! Two upstream facts worth knowing before reading the cases below:
//! - `AgentEndpointRegistrationForm` carries `runtimeVersion` plus a
//!   `versionRange`, and `endpoints` is a **JSON string** inside a form-encoded
//!   body. Batata accepts that form *and* a JSON body, and reads `version` as
//!   an alias for `runtimeVersion`.
//! - `AgentClientController#deregisterEndpoints` delegates to
//!   `AgentHttpClientLifecycleService.deregister(clientId, requestModule, ns,
//!   agentName, protocol)`, which drops whatever *that client* published.
//!   Batata's Naming path carries no publisher identity, so the client's own
//!   publication reservations are consulted alongside the catalog — see
//!   [`deregister_reaches_a_runtime_version_that_was_never_published`].
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run the same cases
//! against a real database. See `common/ai_store.rs`.

#[path = "common/ai_store.rs"]
mod ai_store;

use std::sync::Arc;

use actix_web::http::{Method, StatusCode};
use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_ai::model::{AgentCapabilities, AgentCard, AgentSkill};
use batata_ai::{
    A2aServerOperationService, AiEndpointService, DEFAULT_MAX_PUBLICATIONS_PER_CLIENT,
};
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NS: &str = "public";

/// Cluster manager stub. Nothing on this path reads cluster state.
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
        unimplemented!("not used by the agent client routes")
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

/// One published agent card, at `version`.
fn card(name: &str, version: &str) -> AgentCard {
    AgentCard {
        name: name.to_string(),
        display_name: name.to_string(),
        description: format!("desc-{version}"),
        version: version.to_string(),
        url: "http://localhost:8080".to_string(),
        protocol_version: "1.0".to_string(),
        capabilities: AgentCapabilities {
            streaming: true,
            tool_use: true,
            ..Default::default()
        },
        skills: vec![AgentSkill {
            name: "coding".to_string(),
            description: "code generation".to_string(),
            proficiency: 90,
            examples: vec![],
        }],
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        preferred_transport: None,
        provider: None,
        documentation_url: None,
        icon_url: None,
        supports_authenticated_extended_card: None,
        metadata: Default::default(),
        tags: vec![],
        ..Default::default()
    }
}

/// Body for `POST /v3/client/ai/agents/endpoints`.
///
/// `namespaceId` is omitted rather than sent empty so the test exercises the
/// handler's normalisation branch instead of the query parser's.
fn registration(agent: &str, version: &str, endpoints: &[(&str, u16)]) -> serde_json::Value {
    serde_json::json!({
        "agentName": agent,
        "version": version,
        "protocol": "a2a",
        "endpoints": endpoints
            .iter()
            .map(|(address, port)| serde_json::json!({"address": address, "port": port}))
            .collect::<Vec<_>>(),
    })
}

/// Build an app exposing the real client agent routes, with the default
/// per-client publication cap.
async fn build_app(
    store: Arc<dyn PersistenceService>,
) -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    build_app_with_limit(store, DEFAULT_MAX_PUBLICATIONS_PER_CLIENT).await
}

/// Build an app whose per-client publication cap is `limit`.
///
/// `AiEndpointService` needs a `NamingServiceProvider`; `NamingService` is a
/// plain in-memory structure, so the round trip is exercised without booting a
/// naming cluster.
async fn build_app_with_limit(
    store: Arc<dyn PersistenceService>,
    limit: usize,
) -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    let mut configuration = Configuration::default();
    // `secured!` gates every handler; the default config has console auth on.
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
        persistence: Some(store.clone()),
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

    let naming: Arc<dyn batata_api::naming::NamingServiceProvider> =
        Arc::new(batata_naming::NamingService::new());
    let endpoints = Arc::new(AiEndpointService::with_publication_limit(naming, limit));
    let agents: Arc<dyn batata_common::A2aAgentService> =
        Arc::new(A2aServerOperationService::new(store));

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            // `Data::new`, not `Data::from`: the handlers take
            // `Data<Arc<...>>`, so the registered type is the `Arc` itself.
            .app_data(web::Data::new(endpoints))
            .app_data(web::Data::new(agents))
            // Mounted exactly as `api/v3/client/route.rs` does.
            .service(web::scope("/v3/client/ai").service(batata_ai::agent_client_routes())),
    )
    .await
}

/// Dispatch a request and return `(status, body)`.
async fn call<S>(app: &S, req: actix_http::Request) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    (
        status,
        String::from_utf8_lossy(&test::read_body(resp).await).to_string(),
    )
}

async fn get<S>(app: &S, uri: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    call(app, test::TestRequest::get().uri(uri).to_request()).await
}

/// PUT /endpoints/heartbeat — refresh one client and everything it published.
///
/// Upstream returns `ClientLivenessInfo` from
/// `AgentHttpClientLifecycleService#heartbeat`; the three intervals are what a
/// client schedules its next beat against, so a wrong default silently changes
/// how fast a dead publisher is reaped.
#[actix_web::test]
async fn heartbeat_returns_the_client_liveness_contract() {
    let store = TestStore::open().await;
    let app = build_app(store.store()).await;

    let (status, body) = call(
        &app,
        test::TestRequest::with_uri("/v3/client/ai/agents/endpoints/heartbeat")
            .method(Method::PUT)
            .insert_header(("clientId", "client-1"))
            .insert_header(("requestModule", "nacos-sdk"))
            .to_request(),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "heartbeat must succeed: {body}");
    // `ClientLivenessInfo` is camelCase on the wire.
    assert!(body.contains("\"heartbeatIntervalMillis\":5000"), "{body}");
    assert!(body.contains("\"unhealthyTimeoutMillis\":15000"), "{body}");
    assert!(body.contains("\"expireTimeoutMillis\":30000"), "{body}");
}

/// POST /endpoints then GET / — the publish/discover round trip.
#[actix_web::test]
async fn registered_endpoints_are_discoverable_by_exact_version() {
    const AGENT: &str = "client-agent-discover";
    let store = TestStore::open().await;
    store.clean(AGENT).await;
    let app = build_app(store.store()).await;

    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/client/ai/agents/endpoints")
            .set_json(registration(
                AGENT,
                "1.0.0",
                &[("10.0.0.1", 8080), ("10.0.0.2", 9090)],
            ))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register must succeed: {body}");

    let (status, body) = get(
        &app,
        &format!("/v3/client/ai/agents?agentName={AGENT}&version=1.0.0&namespaceId={NS}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "discover must succeed: {body}");

    assert!(body.contains("\"version\":\"1.0.0\""), "{body}");
    assert!(
        body.contains(&format!("\"namespaceId\":\"{NS}\"")),
        "{body}"
    );
    for (address, port) in [("10.0.0.1", 8080u16), ("10.0.0.2", 9090u16)] {
        assert!(
            body.contains(&format!("\"address\":\"{address}\"")),
            "{body}"
        );
        assert!(body.contains(&format!("\"port\":{port}")), "{body}");
    }

    store.clean(AGENT).await;
}

/// GET / without a version resolves the `latest` label.
///
/// Upstream `AgentDiscoveryApplicationService#resolveVersion` defaults the
/// label to `latest` and only 404s when the agent carries none, so a client
/// that omits the parameter gets the current version rather than a 400.
#[actix_web::test]
async fn discover_without_a_version_resolves_the_latest_label() {
    const AGENT: &str = "client-agent-discover";
    let store = TestStore::open().await;
    store.clean(AGENT).await;

    let agents = A2aServerOperationService::new(store.store());
    agents
        .register_agent(&card(AGENT, "1.0.0"), NS, "manual")
        .await
        .expect("register 1.0.0");
    agents
        .update_agent_card(&card(AGENT, "1.5.0"), NS, "manual")
        .await
        .expect("publish 1.5.0");

    let app = build_app(store.store()).await;
    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/client/ai/agents/endpoints")
            .set_json(registration(AGENT, "1.5.0", &[("10.0.0.4", 7070)]))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register must succeed: {body}");

    let (status, body) = get(
        &app,
        &format!("/v3/client/ai/agents?agentName={AGENT}&namespaceId={NS}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "discover must succeed: {body}");
    assert!(
        body.contains("\"version\":\"1.5.0\""),
        "the latest label must be resolved: {body}"
    );
    assert!(body.contains("\"address\":\"10.0.0.4\""), "{body}");

    store.clean(AGENT).await;
}

/// GET / without a version 404s when the agent has no `latest` label.
///
/// Upstream's `resolveVersion` throws not-found rather than falling back to
/// some other version, which is the same choice the range path already makes.
#[actix_web::test]
async fn discover_without_a_version_reports_not_found_when_there_is_no_latest_label() {
    const AGENT: &str = "client-agent-no-latest";
    let store = TestStore::open().await;
    store.clean(AGENT).await;
    let app = build_app(store.store()).await;

    let (status, body) = get(
        &app,
        &format!("/v3/client/ai/agents?agentName={AGENT}&namespaceId={NS}"),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an agent with no latest version must not be served: {body}"
    );
    assert!(body.contains("no latest version"), "{body}");

    store.clean(AGENT).await;
}

/// GET / with a version range resolves against what is actually published.
#[actix_web::test]
async fn discover_resolves_a_version_range_against_published_versions() {
    const AGENT: &str = "client-agent-range";
    let store = TestStore::open().await;
    store.clean(AGENT).await;

    let agents = A2aServerOperationService::new(store.store());
    agents
        .register_agent(&card(AGENT, "1.0.0"), NS, "manual")
        .await
        .expect("register 1.0.0");
    agents
        .update_agent_card(&card(AGENT, "1.5.0"), NS, "manual")
        .await
        .expect("publish 1.5.0");

    let app = build_app(store.store()).await;
    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/client/ai/agents/endpoints")
            .set_json(registration(AGENT, "1.5.0", &[("10.0.0.9", 7070)]))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register must succeed: {body}");

    // `1.x` is a range, so it is resolved against the published catalog and
    // must land on 1.5.0 — the only version that has endpoints.
    let (status, body) = get(
        &app,
        &format!("/v3/client/ai/agents?agentName={AGENT}&version=1.x&namespaceId={NS}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "discover must succeed: {body}");
    assert!(body.contains("\"version\":\"1.5.0\""), "{body}");
    assert!(body.contains("\"address\":\"10.0.0.9\""), "{body}");

    store.clean(AGENT).await;
}

/// GET / with a range nothing satisfies is 404, not a silent substitution.
#[actix_web::test]
async fn discover_reports_not_found_when_no_published_version_matches_the_range() {
    const AGENT: &str = "client-agent-range";
    let store = TestStore::open().await;
    store.clean(AGENT).await;

    let agents = A2aServerOperationService::new(store.store());
    agents
        .register_agent(&card(AGENT, "1.0.0"), NS, "manual")
        .await
        .expect("register 1.0.0");

    let app = build_app(store.store()).await;
    let (status, body) = get(
        &app,
        &format!("/v3/client/ai/agents?agentName={AGENT}&version=9.x&namespaceId={NS}"),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an unsatisfiable range must not fall back to another version: {body}"
    );
    assert!(body.contains("no published version"), "{body}");

    store.clean(AGENT).await;
}

/// DELETE /endpoints clears every published version of the agent.
#[actix_web::test]
async fn deregister_removes_the_endpoints_of_every_published_version() {
    const AGENT: &str = "client-agent-dereg";
    let store = TestStore::open().await;
    store.clean(AGENT).await;

    let agents = A2aServerOperationService::new(store.store());
    agents
        .register_agent(&card(AGENT, "1.0.0"), NS, "manual")
        .await
        .expect("register 1.0.0");
    agents
        .update_agent_card(&card(AGENT, "1.5.0"), NS, "manual")
        .await
        .expect("publish 1.5.0");

    let app = build_app(store.store()).await;
    for version in ["1.0.0", "1.5.0"] {
        let (status, body) = call(
            &app,
            test::TestRequest::post()
                .uri("/v3/client/ai/agents/endpoints")
                .set_json(registration(AGENT, version, &[("10.0.0.1", 8080)]))
                .to_request(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "register {version}: {body}");
    }

    let (status, body) = call(
        &app,
        test::TestRequest::with_uri(&format!(
            "/v3/client/ai/agents/endpoints?agentName={AGENT}&namespaceId={NS}&protocol=a2a"
        ))
        .method(Method::DELETE)
        .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "deregister must succeed: {body}");

    for version in ["1.0.0", "1.5.0"] {
        let (status, body) = get(
            &app,
            &format!("/v3/client/ai/agents?agentName={AGENT}&version={version}&namespaceId={NS}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "discover {version}: {body}");
        assert!(
            body.contains("\"endpoints\":[]"),
            "deregister must clear {version}: {body}"
        );
    }

    store.clean(AGENT).await;
}

/// DELETE /endpoints reaches a runtime version that is not in the catalog.
///
/// Upstream removes *the client's* publication, so it works for a version that
/// was registered as a runtime endpoint but never published. Batata's Naming
/// path carries no publisher identity, so the client's reservations are the
/// record of what it published — without them this version would survive.
#[actix_web::test]
async fn deregister_reaches_a_runtime_version_that_was_never_published() {
    const AGENT: &str = "client-agent-dereg";
    let store = TestStore::open().await;
    store.clean(AGENT).await;

    let agents = A2aServerOperationService::new(store.store());
    agents
        .register_agent(&card(AGENT, "1.0.0"), NS, "manual")
        .await
        .expect("register 1.0.0");

    let app = build_app(store.store()).await;
    // Published: 1.0.0. Endpoint published under an unpublished 3.0.0.
    let (status, body) = call(
        &app,
        publish(AGENT, "3.0.0", &[("10.0.0.7", 6060)], "client-1"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register must succeed: {body}");

    let (status, body) = call(
        &app,
        test::TestRequest::with_uri(&format!(
            "/v3/client/ai/agents/endpoints?agentName={AGENT}&namespaceId={NS}"
        ))
        .method(Method::DELETE)
        .insert_header(("clientId", "client-1"))
        .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "deregister must succeed: {body}");

    let (status, body) = get(
        &app,
        &format!("/v3/client/ai/agents?agentName={AGENT}&version=3.0.0&namespaceId={NS}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "discover: {body}");
    assert!(
        body.contains("\"endpoints\":[]"),
        "an unpublished runtime version must be deregistered too: {body}"
    );

    store.clean(AGENT).await;
}

/// POST /endpoints accepts upstream's form encoding.
///
/// `AgentEndpointRegistrationForm` is a form whose `endpoints` field is a JSON
/// string of upstream `Endpoint`s (`uri`, not `address` + `port`), so a real
/// Java client cannot reach a handler that only reads JSON.
#[actix_web::test]
async fn register_accepts_the_upstream_form_encoding() {
    const AGENT: &str = "client-agent-form";
    let store = TestStore::open().await;
    store.clean(AGENT).await;
    let app = build_app(store.store()).await;

    let form = serde_urlencoded::to_string([
        ("namespaceId", NS),
        ("agentName", AGENT),
        ("runtimeVersion", "1.0.0"),
        ("protocol", "a2a"),
        (
            "endpoints",
            r#"[{"uri":"http://10.0.0.5:8080/a2a"},{"address":"10.0.0.6","port":9090}]"#,
        ),
    ])
    .expect("encode form");

    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/client/ai/agents/endpoints")
            .insert_header(("content-type", "application/x-www-form-urlencoded"))
            .set_payload(form)
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "form register must succeed: {body}");

    let (status, body) = get(
        &app,
        &format!("/v3/client/ai/agents?agentName={AGENT}&version=1.0.0&namespaceId={NS}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "discover: {body}");
    // The uri's host and port become the endpoint; the path is dropped.
    assert!(body.contains("\"address\":\"10.0.0.5\""), "{body}");
    assert!(body.contains("\"port\":8080"), "{body}");
    assert!(body.contains("\"address\":\"10.0.0.6\""), "{body}");
    assert!(body.contains("\"port\":9090"), "{body}");

    store.clean(AGENT).await;
}

/// A batch that advertises a version range serves a concrete version in it.
///
/// Upstream binds each endpoint to a `RuntimeVersionBinding` and admits it when
/// the binding's range covers the requested version, so a publisher can serve
/// a whole minor line from one runtime version.
#[actix_web::test]
async fn a_registered_version_range_serves_a_concrete_version_in_it() {
    const AGENT: &str = "client-agent-range-binding";
    let store = TestStore::open().await;
    store.clean(AGENT).await;
    let app = build_app(store.store()).await;

    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/client/ai/agents/endpoints")
            .set_json(serde_json::json!({
                "agentName": AGENT,
                "version": "1.5.0",
                "versionRange": "1.x",
                "protocol": "a2a",
                "endpoints": [{"address": "10.0.0.8", "port": 7070}],
            }))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register must succeed: {body}");

    // Published under 1.5.0, requested as 1.2.0: inside the advertised range.
    let (status, body) = get(
        &app,
        &format!("/v3/client/ai/agents?agentName={AGENT}&version=1.2.0&namespaceId={NS}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "discover: {body}");
    assert!(
        body.contains("\"address\":\"10.0.0.8\""),
        "1.x must cover 1.2.0: {body}"
    );

    // Outside the range: the endpoints must not be served.
    let (status, body) = get(
        &app,
        &format!("/v3/client/ai/agents?agentName={AGENT}&version=2.0.0&namespaceId={NS}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "discover: {body}");
    assert!(
        body.contains("\"endpoints\":[]"),
        "1.x must not cover 2.0.0: {body}"
    );

    store.clean(AGENT).await;
}

/// GET /search lists the published catalog.
#[actix_web::test]
async fn search_lists_the_published_catalog() {
    const AGENT: &str = "client-agent-search";
    let store = TestStore::open().await;
    store.clean(AGENT).await;

    let agents = A2aServerOperationService::new(store.store());
    agents
        .register_agent(&card(AGENT, "1.0.0"), NS, "manual")
        .await
        .expect("register");

    let app = build_app(store.store()).await;
    // Upstream names the filter `agentNameContains`; `agentName` is accepted
    // as an alias.
    for query in ["agentNameContains", "agentName"] {
        let (status, body) = get(
            &app,
            &format!("/v3/client/ai/agents/search?{query}={AGENT}&namespaceId={NS}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "search via {query}: {body}");
        assert!(
            body.contains(&format!("\"agentName\":\"{AGENT}\"")),
            "search via {query} must find the agent: {body}"
        );
        assert!(body.contains("\"latestVersion\":\"1.0.0\""), "{body}");
    }

    store.clean(AGENT).await;
}

/// An omitted namespace falls back to the default one.
///
/// Upstream `NamespaceUtil.processNamespaceParameter` maps blank to `public`;
/// Batata's `normalize_namespace` must agree, or a client that omits the
/// parameter writes to a namespace it can never read back.
#[actix_web::test]
async fn an_omitted_namespace_falls_back_to_the_default_one() {
    const AGENT: &str = "client-agent-default-ns";
    let store = TestStore::open().await;
    store.clean(AGENT).await;
    let app = build_app(store.store()).await;

    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/client/ai/agents/endpoints")
            .set_json(registration(AGENT, "2.0.0", &[("10.0.0.3", 8081)]))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register must succeed: {body}");

    let (status, body) = get(
        &app,
        &format!("/v3/client/ai/agents?agentName={AGENT}&version=2.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "discover must succeed: {body}");
    assert!(body.contains("\"namespaceId\":\"public\""), "{body}");
    assert!(body.contains("\"address\":\"10.0.0.3\""), "{body}");

    store.clean(AGENT).await;
}

// ============================================================================
// Publication capacity gate
//
// Upstream `AgentRuntimePublicationCapacityGate` caps how many Runtime
// Endpoint entries one publisher Client may own. It rejects with
// `NacosException.OVER_THRESHOLD` (HTTP 503) carrying business code 50103.
// ============================================================================

/// POST /endpoints with a `clientId` header, as a real publisher sends it.
fn publish(
    agent: &str,
    version: &str,
    endpoints: &[(&str, u16)],
    client_id: &str,
) -> actix_http::Request {
    test::TestRequest::post()
        .uri("/v3/client/ai/agents/endpoints")
        .insert_header(("clientId", client_id))
        .set_json(registration(agent, version, endpoints))
        .to_request()
}

/// A Client at capacity cannot grow.
#[actix_web::test]
async fn a_client_at_capacity_cannot_grow_its_publications() {
    const AGENT_A: &str = "capacity-agent-a";
    const AGENT_B: &str = "capacity-agent-b";
    let store = TestStore::open().await;
    store.clean(AGENT_A).await;
    store.clean(AGENT_B).await;
    let app = build_app_with_limit(store.store(), 2).await;

    let (status, body) = call(
        &app,
        publish(
            AGENT_A,
            "1.0.0",
            &[("10.0.0.1", 8080), ("10.0.0.2", 8080)],
            "client-1",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the first batch fits: {body}");

    // A second agent version from the same client is growth -> rejected.
    let (status, body) = call(
        &app,
        publish(AGENT_B, "1.0.0", &[("10.0.0.3", 8080)], "client-1"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "OVER_THRESHOLD maps to 503: {body}"
    );
    assert!(
        body.contains("50103"),
        "business code must be 50103: {body}"
    );
    assert!(body.contains("publication limit of 2"), "{body}");

    store.clean(AGENT_A).await;
    store.clean(AGENT_B).await;
}

/// Refreshing what a Client already published is admitted even when full.
///
/// Upstream's `ensureCapacity` only rejects when
/// `requestedPublicationCount > existingPublicationCount`, so a full client can
/// still re-publish the same size.
#[actix_web::test]
async fn replacing_a_publication_is_allowed_at_capacity() {
    const AGENT: &str = "capacity-agent-replace";
    let store = TestStore::open().await;
    store.clean(AGENT).await;
    let app = build_app_with_limit(store.store(), 2).await;

    for attempt in 1..=3 {
        let (status, body) = call(
            &app,
            publish(
                AGENT,
                "1.0.0",
                &[("10.0.0.1", 8080), ("10.0.0.2", 8080)],
                "client-1",
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "re-publishing the same key must stay admitted (attempt {attempt}): {body}"
        );
    }

    store.clean(AGENT).await;
}

/// Two different Clients have independent budgets.
#[actix_web::test]
async fn the_capacity_cap_is_per_client() {
    const AGENT: &str = "capacity-agent-two-clients";
    let store = TestStore::open().await;
    store.clean(AGENT).await;
    let app = build_app_with_limit(store.store(), 1).await;

    let (status, body) = call(
        &app,
        publish(AGENT, "1.0.0", &[("10.0.0.1", 8080)], "client-1"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "client-1 fills its budget: {body}");

    let (status, body) = call(
        &app,
        publish(AGENT, "1.0.0", &[("10.0.0.2", 8080)], "client-2"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "client-2 has its own budget: {body}"
    );

    store.clean(AGENT).await;
}

/// Without a `clientId` there is no Client to charge, so the batch is admitted.
///
/// Upstream's gate takes the same branch (`client == null` -> `run()`). Batata
/// keeps the header optional, unlike upstream's `validateStatefulHeaders`.
#[actix_web::test]
async fn a_request_without_a_client_id_is_not_charged() {
    const AGENT: &str = "capacity-agent-anonymous";
    let store = TestStore::open().await;
    store.clean(AGENT).await;
    let app = build_app_with_limit(store.store(), 1).await;

    for port in [8080u16, 8081, 8082] {
        let (status, body) = call(
            &app,
            test::TestRequest::post()
                .uri("/v3/client/ai/agents/endpoints")
                .set_json(registration(AGENT, "1.0.0", &[("10.0.0.1", port)]))
                .to_request(),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "an unattributable batch must be admitted: {body}"
        );
    }

    store.clean(AGENT).await;
}

/// DELETE /endpoints releases the reservation, so the budget frees up.
#[actix_web::test]
async fn deregistering_frees_the_client_budget() {
    const AGENT: &str = "capacity-agent-release";
    const OTHER: &str = "capacity-agent-release-other";
    let store = TestStore::open().await;
    store.clean(AGENT).await;
    store.clean(OTHER).await;

    // DELETE enumerates catalog versions, so the agent must be published.
    let agents = A2aServerOperationService::new(store.store());
    agents
        .register_agent(&card(AGENT, "1.0.0"), NS, "manual")
        .await
        .expect("register 1.0.0");

    let app = build_app_with_limit(store.store(), 2).await;
    let (status, body) = call(
        &app,
        publish(
            AGENT,
            "1.0.0",
            &[("10.0.0.1", 8080), ("10.0.0.2", 8080)],
            "client-1",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "fills the budget: {body}");

    let (status, body) = call(
        &app,
        test::TestRequest::with_uri(&format!(
            "/v3/client/ai/agents/endpoints?agentName={AGENT}&namespaceId={NS}&protocol=a2a"
        ))
        .method(Method::DELETE)
        .insert_header(("clientId", "client-1"))
        .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "delete must succeed: {body}");

    // The slot is free again.
    let (status, body) = call(
        &app,
        publish(OTHER, "1.0.0", &[("10.0.0.3", 8080)], "client-1"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "deregistration must release the reservation: {body}"
    );

    store.clean(AGENT).await;
    store.clean(OTHER).await;
}
