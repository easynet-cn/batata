use std::sync::Arc;

use actix_web::{App, test, web};

use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo, MemberState};

// wiremock for OIDC provider mock in integration tests
#[cfg(test)]
use wiremock;
use batata_naming::service::NamingService;

use crate::acl::AclService;
use crate::agent::ConsulAgentService;
use crate::catalog::ConsulCatalogService;
use crate::config_entry::ConsulConfigEntryService;
use crate::connect::ConsulConnectService;
use crate::connect_ca::ConsulConnectCAService;
use crate::event::ConsulEventService;
use crate::health::ConsulHealthService;
use crate::kv::ConsulKVService;
use crate::operator::ConsulOperatorService;
use crate::query::ConsulQueryService;
use crate::session::ConsulSessionService;
use crate::snapshot::ConsulSnapshotService;

/// Minimal standalone-shaped ClusterManager for HTTP route tests.
///
/// Reports a single local member so the cluster-aware Consul handlers
/// (e.g. `/v1/agent/members`, `/v1/status/leader`) behave like a
/// standalone deployment.
struct TestClusterManager {
    address: String,
}

impl TestClusterManager {
    fn new() -> Self {
        Self {
            address: "127.0.0.1:8848".to_string(),
        }
    }
    fn member(&self) -> ExtendedMemberInfo {
        ExtendedMemberInfo {
            ip: "127.0.0.1".to_string(),
            port: 8848,
            address: self.address.clone(),
            state: MemberState::Up,
            extend_info: std::collections::BTreeMap::new(),
        }
    }
}

impl ClusterManager for TestClusterManager {
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
        Some(self.address.clone())
    }
    fn local_address(&self) -> &str {
        &self.address
    }
    fn member_count(&self) -> usize {
        1
    }
    fn all_members_extended(&self) -> Vec<ExtendedMemberInfo> {
        vec![self.member()]
    }
    fn healthy_members_extended(&self) -> Vec<ExtendedMemberInfo> {
        vec![self.member()]
    }
    fn get_member(&self, address: &str) -> Option<ExtendedMemberInfo> {
        (address == self.address).then(|| self.member())
    }
    fn get_self_member(&self) -> ExtendedMemberInfo {
        self.member()
    }
    fn health_summary(&self) -> ClusterHealthSummary {
        ClusterHealthSummary {
            total: 1,
            up: 1,
            ..Default::default()
        }
    }
    fn refresh_self(&self) {}
    fn is_self(&self, address: &str) -> bool {
        address == self.address
    }
    fn update_member_state(&self, _address: &str, _state: &str) -> Result<String, String> {
        Ok("UP".to_string())
    }
}

/// Create a test app with all in-memory services configured.
/// Uses `crate::api::v1::routes()` for route registration.
fn test_app() -> actix_web::App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    let naming_service = Arc::new(NamingService::new());
    let registry = Arc::new(batata_naming::InstanceCheckRegistry::with_naming_service(
        naming_service,
    ));
    let kv_service = ConsulKVService::new();
    let session_service = ConsulSessionService::new();
    let check_index = Arc::new(crate::check_index::ConsulCheckIndex::new());
    let health_service = ConsulHealthService::new(registry.clone(), check_index.clone());
    let naming_store = Arc::new(crate::naming_store::ConsulNamingStore::new());
    let agent_service = ConsulAgentService::new(naming_store.clone(), registry, check_index);
    let acl_service = AclService::disabled();
    let index_provider = crate::index_provider::ConsulIndexProvider::new();
    let event_service = ConsulEventService::new(index_provider.clone());
    let snapshot_service = ConsulSnapshotService::new();
    let query_service = ConsulQueryService::new();
    let catalog_service = ConsulCatalogService::new(naming_store.clone());
    let config_entry_service = ConsulConfigEntryService::new();
    let connect_service = ConsulConnectService::new()
        .with_config_entry_service(Arc::new(config_entry_service.clone()));
    let connect_ca_service = ConsulConnectCAService::new();
    let cluster_manager: Arc<dyn ClusterManager> = Arc::new(TestClusterManager::new());
    let operator_service =
        ConsulOperatorService::with_datacenter(cluster_manager.clone(), "dc1".to_string());
    let coordinate_service = crate::coordinate::ConsulCoordinateService::new();

    App::new()
        .app_data(web::Data::new(kv_service))
        .app_data(web::Data::new(session_service))
        .app_data(web::Data::new(health_service))
        .app_data(web::Data::new(agent_service))
        .app_data(web::Data::new(acl_service))
        .app_data(web::Data::new(event_service))
        .app_data(web::Data::new(snapshot_service))
        .app_data(web::Data::new(query_service))
        .app_data(web::Data::new(catalog_service))
        .app_data(web::Data::new(config_entry_service))
        .app_data(web::Data::new(connect_service))
        .app_data(web::Data::new(connect_ca_service))
        .app_data(web::Data::new(operator_service))
        .app_data(web::Data::new(cluster_manager))
        .app_data(web::Data::from(naming_store))
        .app_data(web::Data::new(
            crate::model::ConsulDatacenterConfig::default(),
        ))
        .app_data(web::Data::new(crate::peering::ConsulPeeringService::new()))
        .app_data(web::Data::new(
            crate::namespace::ConsulNamespaceService::new(
                crate::index_provider::ConsulIndexProvider::new(),
            ),
        ))
        .app_data(web::Data::new(
            crate::partition::ConsulPartitionService::new(
                crate::index_provider::ConsulIndexProvider::new(),
            ),
        ))
        .app_data(web::Data::new(
            crate::index_provider::ConsulIndexProvider::new(),
        ))
        .app_data(web::Data::new(coordinate_service))
        .service(crate::api::v1::routes())
}

async fn create_test_app() -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    test::init_service(test_app()).await
}

// ========================================================================
// KV Store HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_kv_put_and_get() {
    let app = create_test_app().await;

    // PUT a key
    let req = test::TestRequest::put()
        .uri("/v1/kv/http-test/key1")
        .set_payload("hello-world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // GET the key
    let req = test::TestRequest::get()
        .uri("/v1/kv/http-test/key1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["Key"], "http-test/key1");
}

#[actix_web::test]
async fn test_http_kv_get_nonexistent() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/kv/nonexistent-http-key")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_http_kv_delete() {
    let app = create_test_app().await;

    // PUT then DELETE
    let req = test::TestRequest::put()
        .uri("/v1/kv/http-del/key1")
        .set_payload("to-delete")
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::delete()
        .uri("/v1/kv/http-del/key1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Verify deleted
    let req = test::TestRequest::get()
        .uri("/v1/kv/http-del/key1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_http_kv_keys_only() {
    let app = create_test_app().await;

    // PUT some keys
    for k in &["http-keys/a", "http-keys/b", "http-keys/c"] {
        let req = test::TestRequest::put()
            .uri(&format!("/v1/kv/{}", k))
            .set_payload("v")
            .to_request();
        test::call_service(&app, req).await;
    }

    // GET with ?keys
    let req = test::TestRequest::get()
        .uri("/v1/kv/http-keys/?keys")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
    let keys = body.as_array().unwrap();
    assert!(keys.len() >= 3);
}

#[actix_web::test]
async fn test_http_kv_cas() {
    let app = create_test_app().await;

    // PUT initial value
    let req = test::TestRequest::put()
        .uri("/v1/kv/http-cas/key1")
        .set_payload("initial")
        .to_request();
    test::call_service(&app, req).await;

    // GET to find the modify index
    let req = test::TestRequest::get()
        .uri("/v1/kv/http-cas/key1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let modify_index = body[0]["ModifyIndex"].as_u64().unwrap();

    // CAS with correct index should succeed
    let req = test::TestRequest::put()
        .uri(&format!("/v1/kv/http-cas/key1?cas={}", modify_index))
        .set_payload("updated")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = test::read_body(resp).await;
    assert_eq!(body, "true");

    // CAS with old index should fail
    let req = test::TestRequest::put()
        .uri(&format!("/v1/kv/http-cas/key1?cas={}", modify_index))
        .set_payload("should-fail")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = test::read_body(resp).await;
    assert_eq!(body, "false");
}

// ========================================================================
// Health Check HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_register_and_get_check() {
    let app = create_test_app().await;

    // Register a check
    let check_json = serde_json::json!({
        "Name": "http-check-test",
        "CheckID": "http-chk-1",
        "TTL": "30s",
        "Status": "passing"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/check/register")
        .set_json(&check_json)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // List agent checks
    let req = test::TestRequest::get()
        .uri("/v1/agent/checks")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_object());
    assert!(body.get("http-chk-1").is_some());
}

#[actix_web::test]
async fn test_http_check_pass_warn_fail() {
    let app = create_test_app().await;

    // Register check
    let check_json = serde_json::json!({
        "Name": "status-check",
        "CheckID": "http-status-chk",
        "TTL": "30s"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/check/register")
        .set_json(&check_json)
        .to_request();
    test::call_service(&app, req).await;

    // Pass
    let req = test::TestRequest::put()
        .uri("/v1/agent/check/pass/http-status-chk")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Warn
    let req = test::TestRequest::put()
        .uri("/v1/agent/check/warn/http-status-chk")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Fail
    let req = test::TestRequest::put()
        .uri("/v1/agent/check/fail/http-status-chk")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_deregister_check() {
    let app = create_test_app().await;

    // Register
    let check_json = serde_json::json!({
        "Name": "dereg-check",
        "CheckID": "http-dereg-chk",
        "TTL": "30s"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/check/register")
        .set_json(&check_json)
        .to_request();
    test::call_service(&app, req).await;

    // Deregister
    let req = test::TestRequest::put()
        .uri("/v1/agent/check/deregister/http-dereg-chk")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

// ========================================================================
// Health State HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_health_state_any() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/health/state/any")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_health_service_cross_peer_returns_empty() {
    let app = create_test_app().await;

    // Cross-peer query for a peering that doesn't exist should return 200 with
    // an empty list (Consul returns no error when no instances are available).
    let req = test::TestRequest::get()
        .uri("/v1/health/service/web?peer-name=nonexistent-peer")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
    assert!(body.is_empty());
}

#[actix_web::test]
async fn test_http_health_service_peer_and_sameness_group_conflict() {
    let app = create_test_app().await;

    // peer-name and sameness-group are mutually exclusive → 400
    let req = test::TestRequest::get()
        .uri("/v1/health/service/web?peer-name=p1&sameness-group=sg1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

// ========================================================================
// Health Stream SSE HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_health_stream_service_returns_sse() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/health/stream/web")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.contains("text/event-stream"),
        "expected text/event-stream, got {}",
        content_type
    );

    // Read the first SSE event from the streaming body.
    use actix_web::body::MessageBody;
    let body = resp.into_body();
    let mut body = std::pin::pin!(body);
    let first = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        std::future::poll_fn(|cx| body.as_mut().poll_next(cx)),
    )
    .await
    .expect("timed out waiting for first SSE event")
    .expect("stream ended unexpectedly")
    .expect("body chunk error");

    let text = String::from_utf8_lossy(&first);
    // SSE format: ": X-Consul-Index: N\ndata: [...]\n\n"
    assert!(text.contains("X-Consul-Index"), "missing index comment: {}", text);
    assert!(text.starts_with("data:") || text.contains("data:"), "missing data field: {}", text);

    // The data payload must be valid JSON.
    let data_line = text
        .lines()
        .find(|l| l.starts_with("data:"))
        .expect("no data: line");
    let json_str = data_line.trim_start_matches("data:").trim();
    let _parsed: serde_json::Value =
        serde_json::from_str(json_str).expect("SSE data is not valid JSON");
}

#[actix_web::test]
async fn test_http_health_stream_all_returns_sse() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/health/stream")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/event-stream"));
}

#[actix_web::test]
async fn test_http_health_stream_service_with_passing_filter() {
    let app = create_test_app().await;

    // ?passing=true should be accepted and still return SSE
    let req = test::TestRequest::get()
        .uri("/v1/health/stream/web?passing=true")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/event-stream"));
}

// ========================================================================
// Peering import + cross-peer health integration tests
// ========================================================================

#[actix_web::test]
async fn test_http_peering_import_rejects_unknown_peer() {
    let app = create_test_app().await;

    let req = test::TestRequest::post()
        .uri("/v1/internal/peering/no-such-peer/import")
        .set_json(serde_json::json!([]))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_http_peering_import_and_cross_peer_health() {
    use crate::peering::{PeeringImportedCheck, PeeringImportedService};

    let app = create_test_app().await;

    // 1. Generate a peering token (creates Pending peering).
    let req = test::TestRequest::post()
        .uri("/v1/peering/token")
        .set_json(serde_json::json!({ "PeerName": "peer-b" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let token_resp: serde_json::Value = test::read_body_json(resp).await;
    let token = token_resp["PeeringToken"].as_str().unwrap().to_string();

    // 2. Establish the peering (becomes Active).
    let req = test::TestRequest::post()
        .uri("/v1/peering/establish")
        .set_json(serde_json::json!({ "PeerName": "peer-b", "PeeringToken": token }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 3. Import a service instance via the internal API.
    let instances = vec![PeeringImportedService {
        service_name: "web".to_string(),
        service_id: "web-1".to_string(),
        address: "10.0.0.20".to_string(),
        port: 9090,
        tags: vec!["prod".to_string()],
        meta: Default::default(),
        datacenter: "dc-remote".to_string(),
        node: "remote-node-1".to_string(),
        node_address: "10.0.0.20".to_string(),
        checks: vec![PeeringImportedCheck {
            check_id: "check-web-1".to_string(),
            name: "web health".to_string(),
            status: "passing".to_string(),
            output: "ok".to_string(),
        }],
        imported_at: "2026-01-01T00:00:00Z".to_string(),
    }];
    let req = test::TestRequest::post()
        .uri("/v1/internal/peering/peer-b/import")
        .set_json(instances)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let import_resp: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(import_resp["Imported"], 1);

    // 4. Cross-peer health query returns the imported instance.
    let req = test::TestRequest::get()
        .uri("/v1/health/service/web?peer-name=peer-b")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Effective datacenter header comes from the peering's remote datacenter
    // (encoded in the token by the generator, which is the local DC "dc1" in
    // this single-service test flow).
    let effective_dc = resp
        .headers()
        .get("X-Consul-Effective-Datacenter")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(effective_dc, "dc1");

    let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
    assert_eq!(body.len(), 1);
    let entry = &body[0];
    assert_eq!(entry["Service"]["ID"], "web-1");
    assert_eq!(entry["Service"]["Service"], "web");
    assert_eq!(entry["Service"]["Address"], "10.0.0.20");
    assert_eq!(entry["Service"]["Port"], 9090);
    assert_eq!(entry["Service"]["PeerName"], "peer-b");
    assert_eq!(entry["Node"]["Node"], "remote-node-1");
    assert_eq!(entry["Node"]["Address"], "10.0.0.20");
    assert_eq!(entry["Checks"][0]["Status"], "passing");

    // 5. A non-imported service returns empty.
    let req = test::TestRequest::get()
        .uri("/v1/health/service/db?peer-name=peer-b")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
    assert!(body.is_empty());
}

// ========================================================================
// Peering Phase 2: automatic replication integration test
// ========================================================================

#[actix_web::test]
async fn test_peering_replicate_from_real_server() {
    use crate::peering::{ConsulPeeringService, Peering, PeeringRemoteInfo, PeeringState};
    use std::sync::Arc;

    // 1. Start the test app as a real listening HTTP server (acts as remote peer).
    //    Bind to port 0 to get a free port.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let remote_addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());

    actix_web::rt::spawn(async move {
        let server = actix_web::HttpServer::new(test_app)
            .workers(1)
            .listen(listener)
            .unwrap()
            .run();
        let _ = server.await;
    });
    // Give the server a moment to start accepting connections.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // 2. Register a service on the remote peer via its agent API.
    let client = reqwest::Client::new();
    let reg = serde_json::json!({
        "ID": "web-repl-1",
        "Name": "web-repl",
        "Tags": ["v1"],
        "Address": "10.0.0.55",
        "Port": 7777,
    });
    let resp = client
        .put(format!("http://{}/v1/agent/service/register", remote_addr))
        .json(&reg)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 3. Create a local peering service with an Active peering pointing to the
    //    remote server's address.
    let peering_service = Arc::new(ConsulPeeringService::new());
    peering_service.peerings_for_test().insert(
        "remote-peer".to_string(),
        Peering {
            id: uuid::Uuid::new_v4().to_string(),
            name: "remote-peer".to_string(),
            partition: String::new(),
            state: PeeringState::Active,
            peer_id: uuid::Uuid::new_v4().to_string(),
            peer_server_name: String::new(),
            peer_server_addresses: vec![remote_addr],
            peer_ca_pems: Vec::new(),
            meta: Default::default(),
            stream_status: Default::default(),
            create_index: 1,
            modify_index: 1,
            remote: PeeringRemoteInfo {
                partition: "default".to_string(),
                datacenter: "dc1".to_string(),
            },
            deleted_at: None,
        },
    );

    // 4. Replicate from the remote peer.
    let count = peering_service
        .replicate_from_peer("remote-peer")
        .await
        .unwrap();
    assert!(count >= 1, "expected at least 1 replicated instance");

    // 5. Verify the imported instance matches the registered service.
    let instances = peering_service.get_imported_services("remote-peer");
    let web = instances
        .iter()
        .find(|i| i.service_name == "web-repl")
        .expect("web-repl should be replicated");
    assert_eq!(web.service_id, "web-repl-1");
    assert_eq!(web.address, "10.0.0.55");
    assert_eq!(web.port, 7777);
    assert_eq!(web.tags, vec!["v1"]);
}

// ========================================================================
// Agent HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_agent_self() {
    let app = create_test_app().await;

    let req = test::TestRequest::get().uri("/v1/agent/self").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("Config").is_some());
    assert!(body.get("Member").is_some());

    // DebugConfig.Ports.gRPC must match dc_config.grpc_port (8502 by default).
    let grpc_port = body["DebugConfig"]["Ports"]["gRPC"].as_u64();
    assert_eq!(grpc_port, Some(8502));
}

#[actix_web::test]
async fn test_http_agent_members() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/members")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
    let members = body.as_array().unwrap();
    assert!(!members.is_empty());
    // First member should have Status=1 (alive)
    assert_eq!(members[0]["Status"], 1);
}

#[actix_web::test]
async fn test_http_agent_version() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/version")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let version = body["HumanVersion"].as_str().unwrap();
    // Version should be non-empty (Consul version like "1.22.5" or Batata fallback)
    assert!(!version.is_empty());
    // Revision field carries the Batata version for debugging
    let revision = body["Revision"].as_str().unwrap();
    assert!(!revision.is_empty());
}

#[actix_web::test]
async fn test_http_agent_host() {
    let app = create_test_app().await;

    let req = test::TestRequest::get().uri("/v1/agent/host").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("Memory").is_some());
    assert!(body.get("Host").is_some());
}

#[actix_web::test]
async fn test_http_agent_metrics() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/metrics")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("Gauges").is_some());
}

#[actix_web::test]
async fn test_http_agent_service_register_and_list() {
    let app = create_test_app().await;

    // Register a service
    let svc_json = serde_json::json!({
        "Name": "http-test-web",
        "ID": "http-test-web-1",
        "Port": 8080,
        "Address": "10.0.0.1",
        "Tags": ["v1", "primary"]
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc_json)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // List services
    let req = test::TestRequest::get()
        .uri("/v1/agent/services")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_object());
}

// ========================================================================
// Session HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_session_create_and_list() {
    let app = create_test_app().await;

    // Create a session
    let session_json = serde_json::json!({
        "Name": "http-test-session",
        "TTL": "30s"
    });
    let req = test::TestRequest::put()
        .uri("/v1/session/create")
        .set_json(&session_json)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("ID").is_some());

    // List sessions
    let req = test::TestRequest::get()
        .uri("/v1/session/list")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

#[actix_web::test]
async fn test_http_session_destroy() {
    let app = create_test_app().await;

    // Create
    let session_json = serde_json::json!({
        "Name": "http-destroy-session"
    });
    let req = test::TestRequest::put()
        .uri("/v1/session/create")
        .set_json(&session_json)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let session_id = body["ID"].as_str().unwrap().to_string();

    // Destroy
    let req = test::TestRequest::put()
        .uri(&format!("/v1/session/destroy/{}", session_id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

// ========================================================================
// Event HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_event_fire_and_list() {
    let app = create_test_app().await;

    // Fire an event
    let req = test::TestRequest::put()
        .uri("/v1/event/fire/http-test-evt")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Name"], "http-test-evt");
    assert!(body.get("ID").is_some());

    // List events
    let req = test::TestRequest::get().uri("/v1/event/list").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

// ========================================================================
// Status HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_status_leader() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/status/leader")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_status_peers() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/status/peers")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

// ========================================================================
// Snapshot HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_snapshot_save_and_restore() {
    let app = create_test_app().await;

    // Save snapshot
    let req = test::TestRequest::get().uri("/v1/snapshot").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let snapshot_bytes = test::read_body(resp).await;
    assert!(!snapshot_bytes.is_empty());

    // Restore snapshot
    let req = test::TestRequest::put()
        .uri("/v1/snapshot")
        .set_payload(snapshot_bytes.to_vec())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

// ========================================================================
// Agent Maintenance HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_agent_maintenance() {
    let app = create_test_app().await;

    // Enable maintenance
    let req = test::TestRequest::put()
        .uri("/v1/agent/maintenance?enable=true&reason=testing")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Disable maintenance
    let req = test::TestRequest::put()
        .uri("/v1/agent/maintenance?enable=false")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

// ========================================================================
// Agent Join/Leave/Reload Stubs
// ========================================================================

#[actix_web::test]
async fn test_http_agent_join() {
    let app = create_test_app().await;

    let req = test::TestRequest::put()
        .uri("/v1/agent/join/10.0.0.1:8301")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_leave() {
    let app = create_test_app().await;

    let req = test::TestRequest::put().uri("/v1/agent/leave").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_force_leave() {
    let app = create_test_app().await;

    let req = test::TestRequest::put()
        .uri("/v1/agent/force-leave/node-1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_reload() {
    let app = create_test_app().await;

    let req = test::TestRequest::put()
        .uri("/v1/agent/reload")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

// ========================================================================
// Catalog HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_catalog_register_service() {
    let app = create_test_app().await;

    let reg = serde_json::json!({
        "Node": "cat-reg-node",
        "Address": "10.1.0.1",
        "Service": {
            "Service": "cat-reg-svc",
            "ID": "cat-reg-svc-1",
            "Port": 9090,
            "Tags": ["v1"]
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/catalog/register")
        .set_json(&reg)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_catalog_register_with_checks() {
    let app = create_test_app().await;

    let reg = serde_json::json!({
        "Node": "cat-chk-node",
        "Address": "10.1.0.2",
        "Service": {
            "Service": "cat-chk-svc",
            "ID": "cat-chk-svc-1",
            "Port": 8080
        },
        "Check": {
            "Name": "svc-health",
            "Status": "passing",
            "ServiceID": "cat-chk-svc-1"
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/catalog/register")
        .set_json(&reg)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_catalog_deregister_service() {
    let app = create_test_app().await;

    // Register first
    let reg = serde_json::json!({
        "Node": "cat-dereg-node",
        "Address": "10.1.0.3",
        "Service": {
            "Service": "cat-dereg-svc",
            "ID": "cat-dereg-svc-1",
            "Port": 7070
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/catalog/register")
        .set_json(&reg)
        .to_request();
    test::call_service(&app, req).await;

    // Deregister
    let dereg = serde_json::json!({
        "Node": "cat-dereg-node",
        "ServiceID": "cat-dereg-svc-1"
    });
    let req = test::TestRequest::put()
        .uri("/v1/catalog/deregister")
        .set_json(&dereg)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_catalog_deregister_nonexistent() {
    let app = create_test_app().await;

    let dereg = serde_json::json!({
        "Node": "nonexistent-node",
        "ServiceID": "nonexistent-svc"
    });
    let req = test::TestRequest::put()
        .uri("/v1/catalog/deregister")
        .set_json(&dereg)
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Deregister returns 200 even for nonexistent (idempotent)
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_catalog_list_services() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/catalog/services")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_object());
}

#[actix_web::test]
async fn test_http_catalog_list_services_with_filter() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/catalog/services?dc=dc1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_object());
}

#[actix_web::test]
async fn test_http_catalog_get_service() {
    let app = create_test_app().await;

    // Register a service first
    let reg = serde_json::json!({
        "Node": "cat-get-node",
        "Address": "10.1.0.10",
        "Service": {
            "Service": "cat-get-svc",
            "ID": "cat-get-svc-1",
            "Port": 5050
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/catalog/register")
        .set_json(&reg)
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::get()
        .uri("/v1/catalog/service/cat-get-svc")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
    let services = body.as_array().unwrap();
    assert!(!services.is_empty());
    assert_eq!(services[0]["ServiceName"], "cat-get-svc");
}

#[actix_web::test]
async fn test_http_catalog_get_service_nonexistent() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/catalog/service/no-such-service")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());
}

#[actix_web::test]
async fn test_http_catalog_list_nodes() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/catalog/nodes")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

#[actix_web::test]
async fn test_http_catalog_get_node() {
    let app = create_test_app().await;

    // Register a service so a node exists
    let reg = serde_json::json!({
        "Node": "cat-node-detail",
        "Address": "10.1.0.20",
        "Service": {
            "Service": "cat-node-svc",
            "ID": "cat-node-svc-1",
            "Port": 4040
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/catalog/register")
        .set_json(&reg)
        .to_request();
    test::call_service(&app, req).await;

    // Get node by IP-based name
    let req = test::TestRequest::get()
        .uri("/v1/catalog/node/node-10-1-0-20")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("Node").is_some());
}

#[actix_web::test]
async fn test_http_catalog_get_node_nonexistent() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/catalog/node/no-such-node-xyz")
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Consul returns 200 with null body for non-existent node (not 404)
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_null(), "Expected null body for non-existent node");
}

#[actix_web::test]
async fn test_http_catalog_list_datacenters() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/catalog/datacenters")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
    let dcs = body.as_array().unwrap();
    assert!(!dcs.is_empty());
    assert_eq!(dcs[0], "dc1");
}

#[actix_web::test]
async fn test_http_catalog_connect_service() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/catalog/connect/some-service")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_catalog_node_services() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/catalog/node-services/batata-node")
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Returns 200 with node or 404
    assert!(resp.status() == 200 || resp.status() == 404);
}

#[actix_web::test]
async fn test_http_catalog_gateway_services() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/catalog/gateway-services/my-gateway")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_catalog_ui_services() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/ui/services")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

#[actix_web::test]
async fn test_http_catalog_register_then_list() {
    let app = create_test_app().await;

    // Register
    let reg = serde_json::json!({
        "Node": "cat-list-node",
        "Address": "10.1.0.30",
        "Service": {
            "Service": "cat-list-svc",
            "ID": "cat-list-svc-1",
            "Port": 3030,
            "Tags": ["web"]
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/catalog/register")
        .set_json(&reg)
        .to_request();
    test::call_service(&app, req).await;

    // List services
    let req = test::TestRequest::get()
        .uri("/v1/catalog/services")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let services = body.as_object().unwrap();
    assert!(services.contains_key("cat-list-svc"));
}

#[actix_web::test]
async fn test_http_catalog_register_then_get() {
    let app = create_test_app().await;

    // Register
    let reg = serde_json::json!({
        "Node": "cat-rget-node",
        "Address": "10.1.0.31",
        "Service": {
            "Service": "cat-rget-svc",
            "ID": "cat-rget-svc-1",
            "Port": 3031
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/catalog/register")
        .set_json(&reg)
        .to_request();
    test::call_service(&app, req).await;

    // Get service by name
    let req = test::TestRequest::get()
        .uri("/v1/catalog/service/cat-rget-svc")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let services = body.as_array().unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0]["ServiceName"], "cat-rget-svc");
    assert_eq!(services[0]["ServicePort"], 3031);
}

#[actix_web::test]
async fn test_http_catalog_register_deregister_lifecycle() {
    let app = create_test_app().await;

    // Register
    let reg = serde_json::json!({
        "Node": "cat-life-node",
        "Address": "10.1.0.32",
        "Service": {
            "Service": "cat-life-svc",
            "ID": "cat-life-svc-1",
            "Port": 3032
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/catalog/register")
        .set_json(&reg)
        .to_request();
    test::call_service(&app, req).await;

    // Verify it exists
    let req = test::TestRequest::get()
        .uri("/v1/catalog/service/cat-life-svc")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(!body.as_array().unwrap().is_empty());

    // Deregister
    let dereg = serde_json::json!({
        "Node": "cat-life-node",
        "ServiceID": "cat-life-svc-1"
    });
    let req = test::TestRequest::put()
        .uri("/v1/catalog/deregister")
        .set_json(&dereg)
        .to_request();
    test::call_service(&app, req).await;

    // Verify it is gone
    let req = test::TestRequest::get()
        .uri("/v1/catalog/service/cat-life-svc")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.as_array().unwrap().is_empty());
}

#[actix_web::test]
async fn test_http_catalog_register_multiple_services() {
    let app = create_test_app().await;

    // Register two services
    for (name, id, port) in &[
        ("cat-multi-svc-a", "cat-multi-a-1", 4001),
        ("cat-multi-svc-b", "cat-multi-b-1", 4002),
    ] {
        let reg = serde_json::json!({
            "Node": "cat-multi-node",
            "Address": "10.1.0.40",
            "Service": {
                "Service": name,
                "ID": id,
                "Port": port
            }
        });
        let req = test::TestRequest::put()
            .uri("/v1/catalog/register")
            .set_json(&reg)
            .to_request();
        test::call_service(&app, req).await;
    }

    // List should contain both
    let req = test::TestRequest::get()
        .uri("/v1/catalog/services")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let services = body.as_object().unwrap();
    assert!(services.contains_key("cat-multi-svc-a"));
    assert!(services.contains_key("cat-multi-svc-b"));
}

// ========================================================================
// Agent Additional HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_agent_service_deregister() {
    let app = create_test_app().await;

    // Register a service
    let svc = serde_json::json!({
        "Name": "agt-dereg-svc",
        "ID": "agt-dereg-svc-1",
        "Port": 6060,
        "Address": "10.2.0.1"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc)
        .to_request();
    test::call_service(&app, req).await;

    // Deregister
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/deregister/agt-dereg-svc-1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_service_deregister_nonexistent() {
    let app = create_test_app().await;

    let req = test::TestRequest::put()
        .uri("/v1/agent/service/deregister/nonexistent-svc-xyz")
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Agent deregister returns 404 for nonexistent service
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_http_agent_get_service() {
    let app = create_test_app().await;

    // Register a service
    let svc = serde_json::json!({
        "Name": "agt-getsvc",
        "ID": "agt-getsvc-1",
        "Port": 6061,
        "Address": "10.2.0.2"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc)
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/service/agt-getsvc-1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_get_service_nonexistent() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/service/nonexistent-svc-abc")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_http_agent_service_maintenance_enable() {
    let app = create_test_app().await;

    // Register service first
    let svc = serde_json::json!({
        "Name": "agt-maint-svc",
        "ID": "agt-maint-svc-1",
        "Port": 6062,
        "Address": "10.2.0.3"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc)
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::put()
        .uri("/v1/agent/service/maintenance/agt-maint-svc-1?enable=true&reason=testing")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_service_maintenance_disable() {
    let app = create_test_app().await;

    // Register and enable maintenance
    let svc = serde_json::json!({
        "Name": "agt-maint-dis-svc",
        "ID": "agt-maint-dis-1",
        "Port": 6063,
        "Address": "10.2.0.4"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc)
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::put()
        .uri("/v1/agent/service/maintenance/agt-maint-dis-1?enable=true")
        .to_request();
    test::call_service(&app, req).await;

    // Disable maintenance
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/maintenance/agt-maint-dis-1?enable=false")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_register_multiple_services() {
    let app = create_test_app().await;

    for (name, id, port) in &[
        ("agt-multi-a", "agt-multi-a-1", 7001),
        ("agt-multi-b", "agt-multi-b-1", 7002),
        ("agt-multi-c", "agt-multi-c-1", 7003),
    ] {
        let svc = serde_json::json!({
            "Name": name,
            "ID": id,
            "Port": port,
            "Address": "10.2.0.10"
        });
        let req = test::TestRequest::put()
            .uri("/v1/agent/service/register")
            .set_json(&svc)
            .to_request();
        test::call_service(&app, req).await;
    }

    let req = test::TestRequest::get()
        .uri("/v1/agent/services")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let services = body.as_object().unwrap();
    assert!(services.len() >= 3);
}

#[actix_web::test]
async fn test_http_agent_service_with_checks() {
    let app = create_test_app().await;

    let svc = serde_json::json!({
        "Name": "agt-chk-svc",
        "ID": "agt-chk-svc-1",
        "Port": 7010,
        "Address": "10.2.0.11",
        "Check": {
            "TTL": "15s",
            "DeregisterCriticalServiceAfter": "90m"
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_health_by_id() {
    let app = create_test_app().await;

    // Register a service
    let svc = serde_json::json!({
        "Name": "agt-hid-svc",
        "ID": "agt-hid-svc-1",
        "Port": 7020,
        "Address": "10.2.0.12"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc)
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/health/service/id/agt-hid-svc-1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_health_by_id_nonexistent() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/health/service/id/nonexistent-health-id")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_http_agent_health_by_name() {
    let app = create_test_app().await;

    // Register a service
    let svc = serde_json::json!({
        "Name": "agt-hname-svc",
        "ID": "agt-hname-svc-1",
        "Port": 7030,
        "Address": "10.2.0.13"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc)
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/health/service/name/agt-hname-svc")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_health_by_name_nonexistent() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/health/service/name/nonexistent-health-name")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_http_agent_update_token() {
    let app = create_test_app().await;

    let req = test::TestRequest::put()
        .uri("/v1/agent/token/default")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_update_token_agent() {
    let app = create_test_app().await;

    let req = test::TestRequest::put()
        .uri("/v1/agent/token/agent")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_monitor() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/monitor")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_monitor_with_loglevel() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/monitor?loglevel=debug")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_check_update() {
    let app = create_test_app().await;

    // Register a TTL check first
    let check_json = serde_json::json!({
        "Name": "agt-upd-check",
        "CheckID": "agt-upd-chk-1",
        "TTL": "30s"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/check/register")
        .set_json(&check_json)
        .to_request();
    test::call_service(&app, req).await;

    // Update check status
    let update = serde_json::json!({
        "Status": "passing",
        "Output": "all good"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/check/update/agt-upd-chk-1")
        .set_json(&update)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_agent_list_checks() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/agent/checks")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_object());
}

// ========================================================================
// Internal/UI HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_internal_ui_nodes() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/ui/nodes")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

#[actix_web::test]
async fn test_http_internal_ui_node_info() {
    let app = create_test_app().await;

    // Register a service via agent so a node is known
    let svc = serde_json::json!({
        "Name": "int-node-svc",
        "ID": "int-node-svc-1",
        "Port": 8001,
        "Address": "10.3.0.1"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc)
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/ui/node/10.3.0.1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_internal_ui_node_info_nonexistent() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/ui/node/nonexistent-ui-node")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_http_internal_ui_exported_services() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/ui/exported-services")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

#[actix_web::test]
async fn test_http_internal_ui_catalog_overview() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/ui/catalog-overview")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("Nodes").is_some());
    assert!(body.get("Services").is_some());
    assert!(body.get("Checks").is_some());
}

#[actix_web::test]
async fn test_http_internal_ui_gateway_services_nodes() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/ui/gateway-services-nodes/my-gw")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_internal_ui_gateway_intentions() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/ui/gateway-intentions/my-gw")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

#[actix_web::test]
async fn test_http_internal_ui_service_topology() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/ui/service-topology/my-svc")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("Protocol").is_some());
    assert!(body.get("Upstreams").is_some());
    assert!(body.get("Downstreams").is_some());
}

#[actix_web::test]
async fn test_http_internal_ui_metrics_proxy() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/ui/metrics-proxy/test")
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Metrics proxy returns 200 with metrics URL info
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body.get("metrics_url").is_some(),
        "Response should include metrics_url"
    );
}

#[actix_web::test]
async fn test_http_internal_federation_states() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/federation-states")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

#[actix_web::test]
async fn test_http_internal_federation_state_get() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/internal/federation-state/dc1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Datacenter"], "dc1");
}

#[actix_web::test]
async fn test_http_internal_service_virtual_ip() {
    let app = create_test_app().await;

    let body = serde_json::json!({
        "ServiceName": "vip-test-svc",
        "ManualVIPs": ["10.0.0.1"]
    });
    let req = test::TestRequest::put()
        .uri("/v1/internal/service-virtual-ip")
        .set_json(&body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["ServiceName"], "vip-test-svc");
}

// ========================================================================
// Config Entry HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_config_entry_apply() {
    let app = create_test_app().await;

    let entry = serde_json::json!({
        "Kind": "service-defaults",
        "Name": "cfg-apply-svc"
    });
    let req = test::TestRequest::put()
        .uri("/v1/config")
        .set_json(&entry)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_config_entry_apply_cas() {
    let app = create_test_app().await;

    let entry = serde_json::json!({
        "Kind": "service-defaults",
        "Name": "cfg-cas-svc"
    });
    let req = test::TestRequest::put()
        .uri("/v1/config?cas=0")
        .set_json(&entry)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_http_config_entry_get() {
    let app = create_test_app().await;

    // Apply first
    let entry = serde_json::json!({
        "Kind": "service-defaults",
        "Name": "cfg-get-svc"
    });
    let req = test::TestRequest::put()
        .uri("/v1/config")
        .set_json(&entry)
        .to_request();
    test::call_service(&app, req).await;

    // Get
    let req = test::TestRequest::get()
        .uri("/v1/config/service-defaults/cfg-get-svc")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Kind"], "service-defaults");
    assert_eq!(body["Name"], "cfg-get-svc");
}

#[actix_web::test]
async fn test_http_config_entry_get_nonexistent() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/config/service-defaults/nonexistent-cfg-entry")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_http_config_entry_list() {
    let app = create_test_app().await;

    // Apply an entry first
    let entry = serde_json::json!({
        "Kind": "service-defaults",
        "Name": "cfg-list-svc"
    });
    let req = test::TestRequest::put()
        .uri("/v1/config")
        .set_json(&entry)
        .to_request();
    test::call_service(&app, req).await;

    // List by kind
    let req = test::TestRequest::get()
        .uri("/v1/config/service-defaults")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
    assert!(!body.as_array().unwrap().is_empty());
}

#[actix_web::test]
async fn test_http_config_entry_list_empty() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/config/jwt-provider")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());
}

#[actix_web::test]
async fn test_http_config_entry_delete() {
    let app = create_test_app().await;

    // Apply
    let entry = serde_json::json!({
        "Kind": "service-defaults",
        "Name": "cfg-del-svc"
    });
    let req = test::TestRequest::put()
        .uri("/v1/config")
        .set_json(&entry)
        .to_request();
    test::call_service(&app, req).await;

    // Delete
    let req = test::TestRequest::delete()
        .uri("/v1/config/service-defaults/cfg-del-svc")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Verify deleted
    let req = test::TestRequest::get()
        .uri("/v1/config/service-defaults/cfg-del-svc")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_http_config_entry_lifecycle() {
    let app = create_test_app().await;

    // Apply
    let entry = serde_json::json!({
        "Kind": "proxy-defaults",
        "Name": "global"
    });
    let req = test::TestRequest::put()
        .uri("/v1/config")
        .set_json(&entry)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Get
    let req = test::TestRequest::get()
        .uri("/v1/config/proxy-defaults/global")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // List
    let req = test::TestRequest::get()
        .uri("/v1/config/proxy-defaults")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(!body.as_array().unwrap().is_empty());

    // Delete
    let req = test::TestRequest::delete()
        .uri("/v1/config/proxy-defaults/global")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Verify deleted
    let req = test::TestRequest::get()
        .uri("/v1/config/proxy-defaults/global")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

// ========================================================================
// Status Additional HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_status_leader_response_format() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/status/leader")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_string());
    let leader = body.as_str().unwrap();
    // Leader should be in "ip:port" format
    assert!(leader.contains(':'));
}

#[actix_web::test]
async fn test_http_status_peers_response_format() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/status/peers")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
    let peers = body.as_array().unwrap();
    assert!(!peers.is_empty());
    // Each peer should be a string in "ip:port" format
    for peer in peers {
        assert!(peer.is_string());
        assert!(peer.as_str().unwrap().contains(':'));
    }
}

// ========================================================================
// Event Additional HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_event_fire_with_payload() {
    let app = create_test_app().await;

    let payload = serde_json::json!({
        "Payload": "dGVzdCBwYXlsb2Fk"
    });
    let req = test::TestRequest::put()
        .uri("/v1/event/fire/payload-evt")
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Name"], "payload-evt");
}

#[actix_web::test]
async fn test_http_event_list_filter_by_name() {
    let app = create_test_app().await;

    // Fire a named event
    let req = test::TestRequest::put()
        .uri("/v1/event/fire/filter-evt-name")
        .to_request();
    test::call_service(&app, req).await;

    // List with filter
    let req = test::TestRequest::get()
        .uri("/v1/event/list?name=filter-evt-name")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
    let events = body.as_array().unwrap();
    for evt in events {
        assert_eq!(evt["Name"], "filter-evt-name");
    }
}

#[actix_web::test]
async fn test_http_event_fire_and_list_multiple() {
    let app = create_test_app().await;

    // Fire 3 events
    for name in &["multi-evt-a", "multi-evt-b", "multi-evt-c"] {
        let req = test::TestRequest::put()
            .uri(&format!("/v1/event/fire/{}", name))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // List all events
    let req = test::TestRequest::get().uri("/v1/event/list").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let events = body.as_array().unwrap();
    assert!(events.len() >= 3);
}

// ========================================================================
// Agent Filter Expression Tests (?filter= on /agent/services and /agent/checks)
// ========================================================================

#[actix_web::test]
async fn test_http_agent_services_filter_by_name() {
    let app = create_test_app().await;

    // Register two services with different names
    for (id, name) in &[("flt-svc-1", "web"), ("flt-svc-2", "api")] {
        let svc_json = serde_json::json!({
            "ID": id,
            "Name": name,
            "Port": 8080,
            "Address": "10.0.0.1",
            "Tags": ["v1"]
        });
        let req = test::TestRequest::put()
            .uri("/v1/agent/service/register")
            .set_json(&svc_json)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // Filter by Service == "web"
    let req = test::TestRequest::get()
        .uri("/v1/agent/services?filter=Service%20%3D%3D%20%22web%22")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_object());
    let obj = body.as_object().unwrap();
    // Should only have the "web" service
    assert_eq!(obj.len(), 1);
    assert!(obj.contains_key("flt-svc-1"));
}

#[actix_web::test]
async fn test_http_agent_services_filter_by_tag() {
    let app = create_test_app().await;

    // Register services with different tags
    for (id, tags) in &[
        ("flt-tag-1", vec!["v1", "prod"]),
        ("flt-tag-2", vec!["v2", "staging"]),
        ("flt-tag-3", vec!["v1", "staging"]),
    ] {
        let svc_json = serde_json::json!({
            "ID": id,
            "Name": format!("svc-{}", id),
            "Port": 8080,
            "Address": "10.0.0.1",
            "Tags": tags
        });
        let req = test::TestRequest::put()
            .uri("/v1/agent/service/register")
            .set_json(&svc_json)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // Filter by "v1" in Tags
    let req = test::TestRequest::get()
        .uri("/v1/agent/services?filter=%22v1%22%20in%20Tags")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let obj = body.as_object().unwrap();
    // Should have flt-tag-1 and flt-tag-3 (both have "v1" tag)
    assert_eq!(obj.len(), 2);
    assert!(obj.contains_key("flt-tag-1"));
    assert!(obj.contains_key("flt-tag-3"));
}

#[actix_web::test]
async fn test_http_agent_services_filter_no_match() {
    let app = create_test_app().await;

    // Register a service
    let svc_json = serde_json::json!({
        "ID": "flt-none-1",
        "Name": "web",
        "Port": 8080,
        "Address": "10.0.0.1",
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc_json)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Filter that matches nothing
    let req = test::TestRequest::get()
        .uri("/v1/agent/services?filter=Service%20%3D%3D%20%22nonexistent%22")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let obj = body.as_object().unwrap();
    assert_eq!(obj.len(), 0);
}

#[actix_web::test]
async fn test_http_agent_services_no_filter_returns_all() {
    let app = create_test_app().await;

    // Register two services
    for id in &["flt-all-1", "flt-all-2"] {
        let svc_json = serde_json::json!({
            "ID": id,
            "Name": format!("svc-{}", id),
            "Port": 8080,
            "Address": "10.0.0.1",
        });
        let req = test::TestRequest::put()
            .uri("/v1/agent/service/register")
            .set_json(&svc_json)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // No filter = return all
    let req = test::TestRequest::get()
        .uri("/v1/agent/services")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let obj = body.as_object().unwrap();
    assert!(obj.len() >= 2);
}

#[actix_web::test]
async fn test_http_agent_checks_filter_by_status() {
    let app = create_test_app().await;

    // Register checks with different statuses
    for (id, name, status) in &[
        ("flt-chk-pass", "passing-check", "passing"),
        ("flt-chk-warn", "warning-check", "warning"),
        ("flt-chk-fail", "critical-check", "critical"),
    ] {
        let check_json = serde_json::json!({
            "ID": id,
            "Name": name,
            "Status": status,
            "Notes": "test check",
            "ServiceID": "",
            "ServiceName": ""
        });
        let req = test::TestRequest::put()
            .uri("/v1/agent/check/register")
            .set_json(&check_json)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // Filter checks by Status == "passing"
    let req = test::TestRequest::get()
        .uri("/v1/agent/checks?filter=Status%20%3D%3D%20%22passing%22")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let obj = body.as_object().unwrap();
    // Should only have the passing check
    assert!(!obj.is_empty());
    assert!(obj.contains_key("flt-chk-pass"));
}

#[actix_web::test]
async fn test_http_agent_checks_filter_by_name() {
    let app = create_test_app().await;

    // Register checks
    for (id, name) in &[
        ("flt-cn-1", "web-health"),
        ("flt-cn-2", "db-health"),
    ] {
        let check_json = serde_json::json!({
            "ID": id,
            "Name": name,
            "Status": "passing",
            "ServiceID": "",
            "ServiceName": ""
        });
        let req = test::TestRequest::put()
            .uri("/v1/agent/check/register")
            .set_json(&check_json)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // Filter by Name contains "web"
    let req = test::TestRequest::get()
        .uri("/v1/agent/checks?filter=Name%20contains%20%22web%22")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let obj = body.as_object().unwrap();
    assert!(!obj.is_empty());
    assert!(obj.contains_key("flt-cn-1"));
    assert!(!obj.contains_key("flt-cn-2"));
}

// ========================================================================
// ACL Templated Policy HTTP Tests
// ========================================================================

/// All six built-in template names.
const ALL_TEMPLATES: &[&str] = &[
    "builtin/service",
    "builtin/node",
    "builtin/dns",
    "builtin/nomad-server",
    "builtin/api-gateway",
    "builtin/nomad-client",
];

/// Templates that require a `name` variable (non-empty JSON schema).
const NAME_TEMPLATES: &[&str] = &["builtin/service", "builtin/node", "builtin/api-gateway"];

#[actix_web::test]
async fn test_http_acl_templated_policies_list() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/acl/templated-policies")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let obj = body.as_object().expect("templated policies should be a map");
    assert_eq!(obj.len(), 6, "all 6 built-in templates should be present");

    for name in ALL_TEMPLATES {
        let entry = obj
            .get(*name)
            .unwrap_or_else(|| panic!("missing template {}", name));
        assert_eq!(entry["TemplateName"], *name);
        // Schema, Template, Description must always be present (string).
        assert!(
            entry["Schema"].is_string(),
            "Schema for {} should be a string",
            name
        );
        assert!(
            entry["Template"].is_string(),
            "Template for {} should be a string",
            name
        );
        assert!(
            entry["Description"].is_string(),
            "Description for {} should be a string",
            name
        );
        assert!(
            !entry["Description"].as_str().unwrap().is_empty(),
            "Description for {} should not be empty",
            name
        );
    }

    // Templates with a schema should have a non-empty Schema.
    for name in NAME_TEMPLATES {
        assert!(
            !obj[*name]["Schema"].as_str().unwrap().is_empty(),
            "Schema for {} should be non-empty",
            name
        );
    }

    // Templates without a schema should have an empty Schema string.
    for name in &["builtin/dns", "builtin/nomad-server", "builtin/nomad-client"] {
        assert_eq!(
            obj[*name]["Schema"], "",
            "Schema for {} should be empty",
            name
        );
    }
}

async fn read_templated_policy(name: &str) -> serde_json::Value {
    let app = create_test_app().await;
    let req = test::TestRequest::get()
        .uri(&format!("/v1/acl/templated-policy/name/{}", name))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "read {} should return 200", name);
    test::read_body_json(resp).await
}

#[actix_web::test]
async fn test_http_acl_templated_policy_read_service() {
    let body = read_templated_policy("builtin/service").await;
    assert_eq!(body["TemplateName"], "builtin/service");
    assert!(!body["Schema"].as_str().unwrap().is_empty());
    assert!(body["Template"].as_str().unwrap().contains("{{.Name}}"));
    assert!(body["Template"]
        .as_str()
        .unwrap()
        .contains("{{.Name}}-sidecar-proxy"));
    assert!(body["Template"].as_str().unwrap().contains("service_prefix"));
    assert!(body["Template"].as_str().unwrap().contains("node_prefix"));
    assert!(!body["Description"].as_str().unwrap().is_empty());
}

#[actix_web::test]
async fn test_http_acl_templated_policy_read_node() {
    let body = read_templated_policy("builtin/node").await;
    assert_eq!(body["TemplateName"], "builtin/node");
    assert!(!body["Schema"].as_str().unwrap().is_empty());
    assert!(body["Template"].as_str().unwrap().contains("node \"{{.Name}}\""));
    assert!(body["Template"].as_str().unwrap().contains("service_prefix"));
    assert!(!body["Description"].as_str().unwrap().is_empty());
}

#[actix_web::test]
async fn test_http_acl_templated_policy_read_dns() {
    let body = read_templated_policy("builtin/dns").await;
    assert_eq!(body["TemplateName"], "builtin/dns");
    assert_eq!(body["Schema"], "");
    assert!(body["Template"].as_str().unwrap().contains("node_prefix"));
    assert!(body["Template"].as_str().unwrap().contains("service_prefix"));
    assert!(body["Template"].as_str().unwrap().contains("query_prefix"));
    assert!(!body["Description"].as_str().unwrap().is_empty());
}

#[actix_web::test]
async fn test_http_acl_templated_policy_read_nomad_server() {
    let body = read_templated_policy("builtin/nomad-server").await;
    assert_eq!(body["TemplateName"], "builtin/nomad-server");
    assert_eq!(body["Schema"], "");
    assert!(body["Template"].as_str().unwrap().contains("acl = \"write\""));
    assert!(body["Template"].as_str().unwrap().contains("agent_prefix"));
    assert!(body["Template"].as_str().unwrap().contains("node_prefix"));
    assert!(body["Template"].as_str().unwrap().contains("service_prefix"));
    assert!(!body["Description"].as_str().unwrap().is_empty());
}

#[actix_web::test]
async fn test_http_acl_templated_policy_read_api_gateway() {
    let body = read_templated_policy("builtin/api-gateway").await;
    assert_eq!(body["TemplateName"], "builtin/api-gateway");
    assert!(!body["Schema"].as_str().unwrap().is_empty());
    assert!(body["Template"].as_str().unwrap().contains("mesh = \"read\""));
    assert!(body["Template"].as_str().unwrap().contains("service \"{{.Name}}\""));
    assert!(!body["Description"].as_str().unwrap().is_empty());
}

#[actix_web::test]
async fn test_http_acl_templated_policy_read_nomad_client() {
    let body = read_templated_policy("builtin/nomad-client").await;
    assert_eq!(body["TemplateName"], "builtin/nomad-client");
    assert_eq!(body["Schema"], "");
    assert!(body["Template"].as_str().unwrap().contains("agent_prefix"));
    assert!(body["Template"].as_str().unwrap().contains("node_prefix"));
    assert!(body["Template"].as_str().unwrap().contains("service_prefix"));
    assert!(body["Template"].as_str().unwrap().contains("key_prefix"));
    assert!(!body["Description"].as_str().unwrap().is_empty());
}

#[actix_web::test]
async fn test_http_acl_templated_policy_read_unknown() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/acl/templated-policy/name/builtin/unknown")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

/// Preview a templated policy and return the HTTP status plus the raw body
/// as a string (works for both JSON success bodies and plain-text errors).
async fn preview_templated_policy(name: &str, body: serde_json::Value) -> (u16, String) {
    let app = create_test_app().await;
    let req = test::TestRequest::post()
        .uri(&format!("/v1/acl/templated-policy/preview/{}", name))
        .set_json(&body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status().as_u16();
    let bytes = test::read_body(resp).await;
    (
        status,
        String::from_utf8(bytes.to_vec()).unwrap_or_default(),
    )
}

#[actix_web::test]
async fn test_http_acl_templated_policy_preview_service() {
    let (status, body_str) =
        preview_templated_policy("builtin/service", serde_json::json!({"name": "api"})).await;
    assert_eq!(status, 200);
    let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    assert!(body["ID"].is_string());
    assert!(
        body["Name"]
            .as_str()
            .unwrap()
            .starts_with("synthetic-policy-")
    );
    assert!(
        body["Description"]
            .as_str()
            .unwrap()
            .contains("builtin/service")
    );
    let rules = body["Rules"].as_str().unwrap();
    assert!(rules.contains("service \"api\""));
    assert!(rules.contains("service \"api-sidecar-proxy\""));
    assert!(rules.contains("service_prefix"));
    assert!(rules.contains("node_prefix"));
    // ID should be the hash of the rules.
    assert_eq!(
        body["ID"],
        body["Name"]
            .as_str()
            .unwrap()
            .trim_start_matches("synthetic-policy-")
    );
}

#[actix_web::test]
async fn test_http_acl_templated_policy_preview_node() {
    let (status, body_str) =
        preview_templated_policy("builtin/node", serde_json::json!({"name": "web"})).await;
    assert_eq!(status, 200);
    let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    assert!(
        body["Name"]
            .as_str()
            .unwrap()
            .starts_with("synthetic-policy-")
    );
    let rules = body["Rules"].as_str().unwrap();
    assert!(rules.contains("node \"web\""));
    assert!(rules.contains("service_prefix"));
}

#[actix_web::test]
async fn test_http_acl_templated_policy_preview_dns() {
    let (status, body_str) =
        preview_templated_policy("builtin/dns", serde_json::json!({})).await;
    assert_eq!(status, 200);
    let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    assert!(
        body["Name"]
            .as_str()
            .unwrap()
            .starts_with("synthetic-policy-")
    );
    let rules = body["Rules"].as_str().unwrap();
    assert!(rules.contains("node_prefix"));
    assert!(rules.contains("service_prefix"));
    assert!(rules.contains("query_prefix"));
    // DNS has no {{.Name}} placeholder — rules should not contain it.
    assert!(!rules.contains("{{.Name}}"));
}

#[actix_web::test]
async fn test_http_acl_templated_policy_preview_nomad_server() {
    let (status, body_str) =
        preview_templated_policy("builtin/nomad-server", serde_json::json!({})).await;
    assert_eq!(status, 200);
    let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    assert!(
        body["Name"]
            .as_str()
            .unwrap()
            .starts_with("synthetic-policy-")
    );
    let rules = body["Rules"].as_str().unwrap();
    assert!(rules.contains("acl = \"write\""));
    assert!(rules.contains("agent_prefix"));
    assert!(rules.contains("service_prefix"));
}

#[actix_web::test]
async fn test_http_acl_templated_policy_preview_api_gateway() {
    let (status, body_str) = preview_templated_policy(
        "builtin/api-gateway",
        serde_json::json!({"name": "my-gateway"}),
    )
    .await;
    assert_eq!(status, 200);
    let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    assert!(
        body["Name"]
            .as_str()
            .unwrap()
            .starts_with("synthetic-policy-")
    );
    let rules = body["Rules"].as_str().unwrap();
    assert!(rules.contains("mesh = \"read\""));
    assert!(rules.contains("service \"my-gateway\""));
}

#[actix_web::test]
async fn test_http_acl_templated_policy_preview_nomad_client() {
    let (status, body_str) =
        preview_templated_policy("builtin/nomad-client", serde_json::json!({})).await;
    assert_eq!(status, 200);
    let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    assert!(
        body["Name"]
            .as_str()
            .unwrap()
            .starts_with("synthetic-policy-")
    );
    let rules = body["Rules"].as_str().unwrap();
    assert!(rules.contains("agent_prefix"));
    assert!(rules.contains("key_prefix"));
}

#[actix_web::test]
async fn test_http_acl_templated_policy_preview_service_missing_name() {
    // Service template requires a name; omitting it should fail validation.
    let (status, body_str) =
        preview_templated_policy("builtin/service", serde_json::json!({})).await;
    assert_eq!(status, 400);
    assert!(
        body_str.contains("name is required") || body_str.contains("validation error"),
        "expected validation error, got: {}",
        body_str
    );
}

#[actix_web::test]
async fn test_http_acl_templated_policy_preview_service_invalid_name() {
    // Uppercase characters are not allowed in service identity names.
    let (status, body_str) = preview_templated_policy(
        "builtin/service",
        serde_json::json!({"name": "InvalidName"}),
    )
    .await;
    assert_eq!(status, 400);
    assert!(
        body_str.contains("invalid name"),
        "expected invalid name error, got: {}",
        body_str
    );
}

#[actix_web::test]
async fn test_http_acl_templated_policy_preview_unknown() {
    let (status, _body_str) =
        preview_templated_policy("builtin/unknown", serde_json::json!({})).await;
    assert_eq!(status, 400);
}

// ========================================================================
// Partition HTTP Tests
// ========================================================================

#[actix_web::test]
async fn test_http_partition_create() {
    let app = create_test_app().await;

    let req = test::TestRequest::put()
        .uri("/v1/partition")
        .set_json(serde_json::json!({
            "Name": "test-partition",
            "Description": "A test partition"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Name"], "test-partition");
    assert_eq!(body["Description"], "A test partition");
    assert!(body["CreateIndex"].as_u64().unwrap_or(0) > 0);
    assert!(body["ModifyIndex"].as_u64().unwrap_or(0) > 0);
}

#[actix_web::test]
async fn test_http_partition_create_default_fails() {
    let app = create_test_app().await;

    let req = test::TestRequest::put()
        .uri("/v1/partition")
        .set_json(serde_json::json!({
            "Name": "default",
            "Description": "Trying to recreate default"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
}

#[actix_web::test]
async fn test_http_partition_create_duplicate_fails() {
    let app = create_test_app().await;

    // First create succeeds
    let req = test::TestRequest::put()
        .uri("/v1/partition")
        .set_json(serde_json::json!({
            "Name": "dup-partition",
            "Description": "First"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Second create with same name fails
    let req = test::TestRequest::put()
        .uri("/v1/partition")
        .set_json(serde_json::json!({
            "Name": "dup-partition",
            "Description": "Second"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
}

#[actix_web::test]
async fn test_http_partition_read() {
    let app = create_test_app().await;

    // Create a partition first
    let req = test::TestRequest::put()
        .uri("/v1/partition")
        .set_json(serde_json::json!({
            "Name": "read-test",
            "Description": "Readable partition"
        }))
        .to_request();
    let _ = test::call_service(&app, req).await;

    // Read it back
    let req = test::TestRequest::get()
        .uri("/v1/partition/read-test")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Name"], "read-test");
    assert_eq!(body["Description"], "Readable partition");
}

#[actix_web::test]
async fn test_http_partition_read_not_found() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/partition/nonexistent")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_http_partition_update() {
    let app = create_test_app().await;

    // Create a partition first
    let req = test::TestRequest::put()
        .uri("/v1/partition")
        .set_json(serde_json::json!({
            "Name": "update-test",
            "Description": "Original"
        }))
        .to_request();
    let _ = test::call_service(&app, req).await;

    // Update it
    let req = test::TestRequest::put()
        .uri("/v1/partition/update-test")
        .set_json(serde_json::json!({
            "Name": "update-test",
            "Description": "Updated description"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Name"], "update-test");
    assert_eq!(body["Description"], "Updated description");
}

#[actix_web::test]
async fn test_http_partition_delete() {
    let app = create_test_app().await;

    // Create a partition first
    let req = test::TestRequest::put()
        .uri("/v1/partition")
        .set_json(serde_json::json!({
            "Name": "delete-test",
            "Description": "Will be deleted"
        }))
        .to_request();
    let _ = test::call_service(&app, req).await;

    // Delete it (soft delete)
    let req = test::TestRequest::delete()
        .uri("/v1/partition/delete-test")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Read it back — should still exist with DeletedAt set
    let req = test::TestRequest::get()
        .uri("/v1/partition/delete-test")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Name"], "delete-test");
    assert!(
        body["DeletedAt"].as_str().is_some(),
        "DeletedAt should be set after soft delete"
    );
}

#[actix_web::test]
async fn test_http_partition_delete_default_fails() {
    let app = create_test_app().await;

    let req = test::TestRequest::delete()
        .uri("/v1/partition/default")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_http_partition_list() {
    let app = create_test_app().await;

    // Create some partitions
    let req = test::TestRequest::put()
        .uri("/v1/partition")
        .set_json(serde_json::json!({"Name": "list-alpha"}))
        .to_request();
    let _ = test::call_service(&app, req).await;

    let req = test::TestRequest::put()
        .uri("/v1/partition")
        .set_json(serde_json::json!({"Name": "list-beta"}))
        .to_request();
    let _ = test::call_service(&app, req).await;

    // List all partitions
    let req = test::TestRequest::get().uri("/v1/partitions").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let partitions = body.as_array().unwrap();
    // default + list-alpha + list-beta = 3
    assert_eq!(partitions.len(), 3);

    let names: Vec<&str> = partitions
        .iter()
        .map(|p| p["Name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"default"));
    assert!(names.contains(&"list-alpha"));
    assert!(names.contains(&"list-beta"));
}

#[actix_web::test]
async fn test_http_partition_list_excludes_deleted() {
    let app = create_test_app().await;

    // Create a partition
    let req = test::TestRequest::put()
        .uri("/v1/partition")
        .set_json(serde_json::json!({"Name": "will-delete"}))
        .to_request();
    let _ = test::call_service(&app, req).await;

    // Delete it (soft delete)
    let req = test::TestRequest::delete()
        .uri("/v1/partition/will-delete")
        .to_request();
    let _ = test::call_service(&app, req).await;

    // List should not include the deleted partition
    let req = test::TestRequest::get().uri("/v1/partitions").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let partitions = body.as_array().unwrap();
    // Only default should remain
    assert_eq!(partitions.len(), 1);
    assert_eq!(partitions[0]["Name"], "default");
}

// ========================================================================
// OIDC HTTP Tests
// ========================================================================

/// Helper: creates an OIDC auth method.
async fn create_oidc_auth_method(app: &impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
>, name: &str, discovery_url: &str) {
    let req = test::TestRequest::put()
        .uri("/v1/acl/auth-method")
        .set_json(serde_json::json!({
            "Name": name,
            "Type": "oidc",
            "Config": {
                "OIDCDiscoveryURL": discovery_url,
                "OIDCClientID": "test-client-id",
                "OIDCClientSecret": "test-client-secret",
                "AllowedRedirectURIs": ["http://localhost:8500/callback"]
            }
        }))
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status(), 200, "Creating OIDC auth method should return 200");
}

/// `POST /v1/acl/oidc/auth-url` - returns 404 when the auth method does not exist.
#[actix_web::test]
async fn test_http_oidc_auth_url_no_auth_method() {
    let app = create_test_app().await;

    let req = test::TestRequest::post()
        .uri("/v1/acl/oidc/auth-url")
        .set_json(serde_json::json!({
            "AuthMethod": "nonexistent-oidc-method",
            "RedirectURI": "http://localhost:8500/callback"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

/// `POST /v1/acl/oidc/auth-url` - returns 400 when the auth method type is not oidc.
#[actix_web::test]
async fn test_http_oidc_auth_url_invalid_method_type() {
    let app = create_test_app().await;

    // Create a jwt-type auth method (not oidc).
    let req = test::TestRequest::put()
        .uri("/v1/acl/auth-method")
        .set_json(serde_json::json!({
            "Name": "test-jwt-method",
            "Type": "jwt",
            "Config": {
                "BoundAudiences": ["test-audience"]
            }
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Requesting the oidc auth-url with a jwt-type auth method should return 400.
    let req = test::TestRequest::post()
        .uri("/v1/acl/oidc/auth-url")
        .set_json(serde_json::json!({
            "AuthMethod": "test-jwt-method",
            "RedirectURI": "http://localhost:8500/callback"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

/// `POST /v1/acl/oidc/auth-url` - successfully generates an authorization URL.
#[actix_web::test]
async fn test_http_oidc_auth_url_success() {
    // Start the mock OIDC provider.
    let mock_server = wiremock::MockServer::start().await;

    // Mock the OIDC discovery endpoint.
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/.well-known/openid-configuration"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issuer": mock_server.uri(),
            "authorization_endpoint": format!("{}/oauth2/authorize", mock_server.uri()),
            "token_endpoint": format!("{}/oauth2/token", mock_server.uri()),
            "jwks_uri": format!("{}/.well-known/jwks.json", mock_server.uri()),
            "userinfo_endpoint": format!("{}/userinfo", mock_server.uri())
        })))
        .mount(&mock_server)
        .await;

    let app = create_test_app().await;

    // Create an OIDC auth method.
    let discovery_url = format!("{}/.well-known/openid-configuration", mock_server.uri());
    create_oidc_auth_method(&app, "test-oidc-auth-url-success", &discovery_url).await;

    // Request the auth URL.
    let req = test::TestRequest::post()
        .uri("/v1/acl/oidc/auth-url")
        .set_json(serde_json::json!({
            "AuthMethod": "test-oidc-auth-url-success",
            "RedirectURI": "http://localhost:8500/callback"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let auth_url = body["AuthURL"]
        .as_str()
        .expect("Response should contain AuthURL field");
    assert!(
        auth_url.contains("/oauth2/authorize"),
        "AuthURL should contain the authorization endpoint: {}",
        auth_url
    );
    assert!(
        auth_url.contains("client_id=test-client-id"),
        "AuthURL should contain client_id: {}",
        auth_url
    );
    assert!(
        auth_url.contains("response_type=code"),
        "AuthURL should contain response_type=code: {}",
        auth_url
    );
    assert!(
        auth_url.contains("code_challenge_method=S256"),
        "AuthURL should contain PKCE challenge method: {}",
        auth_url
    );
}

/// `POST /v1/acl/oidc/callback` - returns 400 for an invalid state.
#[actix_web::test]
async fn test_http_oidc_callback_invalid_state() {
    let app = create_test_app().await;

    // Create an OIDC auth method (no mock provider needed, since state validation happens before contacting the provider).
    let req = test::TestRequest::put()
        .uri("/v1/acl/auth-method")
        .set_json(serde_json::json!({
            "Name": "test-oidc-callback-invalid-state",
            "Type": "oidc",
            "Config": {
                "OIDCDiscoveryURL": "http://localhost:0/.well-known/openid-configuration",
                "OIDCClientID": "test-client-id",
                "OIDCClientSecret": "test-client-secret",
                "AllowedRedirectURIs": ["http://localhost:8500/callback"]
            }
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Call the callback with a non-existent state.
    let req = test::TestRequest::post()
        .uri("/v1/acl/oidc/callback")
        .set_json(serde_json::json!({
            "AuthMethod": "test-oidc-callback-invalid-state",
            "State": "nonexistent-state-id",
            "Code": "test-auth-code"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

/// `POST /v1/acl/oidc/callback` - returns 400 when the code parameter is missing.
#[actix_web::test]
async fn test_http_oidc_callback_missing_code() {
    let app = create_test_app().await;

    // Create an OIDC auth method.
    let req = test::TestRequest::put()
        .uri("/v1/acl/auth-method")
        .set_json(serde_json::json!({
            "Name": "test-oidc-callback-missing-code",
            "Type": "oidc",
            "Config": {
                "OIDCDiscoveryURL": "http://localhost:0/.well-known/openid-configuration",
                "OIDCClientID": "test-client-id",
                "OIDCClientSecret": "test-client-secret",
                "AllowedRedirectURIs": ["http://localhost:8500/callback"]
            }
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Call the callback with an empty code.
    let req = test::TestRequest::post()
        .uri("/v1/acl/oidc/callback")
        .set_json(serde_json::json!({
            "AuthMethod": "test-oidc-callback-missing-code",
            "State": "some-state",
            "Code": ""
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

// ========================================================================
// Keyring HTTP Tests
// ========================================================================

/// Generates a valid AES-256 key (32 bytes base64-encoded), for HTTP tests.
fn http_test_key(seed: u8) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode([seed; 32])
}

/// `GET /v1/operator/keyring` - returns a `KeyringResponses`.
#[actix_web::test]
async fn test_http_keyring_list() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/operator/keyring")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Verify the response is a KeyringResponses (contains the Responses array).
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body.is_object(),
        "keyring list should return a JSON object"
    );
    let responses = body["Responses"]
        .as_array()
        .expect("should have Responses array");
    assert!(
        !responses.is_empty(),
        "should have at least one keyring response"
    );

    // By default return both LAN and WAN responses.
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["WAN"], false);
    assert_eq!(responses[1]["WAN"], true);
    assert_eq!(responses[0]["Datacenter"], "dc1");
    assert_eq!(responses[0]["NumNodes"], 1);
    assert!(
        responses[0]["Keys"].is_object(),
        "Keys should be a map"
    );
    assert!(
        responses[0]["PrimaryKeys"].is_object(),
        "PrimaryKeys should be a map"
    );
}

/// `GET /v1/operator/keyring?local-only=true` - local_only returns only the LAN response.
#[actix_web::test]
async fn test_http_keyring_list_local_only() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/operator/keyring?local-only=true")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let responses = body["Responses"]
        .as_array()
        .expect("should have Responses array");
    assert_eq!(
        responses.len(),
        1,
        "local-only should return only LAN response"
    );
    assert_eq!(responses[0]["WAN"], false);
}

/// `POST /v1/operator/keyring` - installs a key.
#[actix_web::test]
async fn test_http_keyring_install() {
    let app = create_test_app().await;
    let key = http_test_key(0x42);

    let req = test::TestRequest::post()
        .uri("/v1/operator/keyring")
        .set_json(serde_json::json!({ "Key": key }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Verify the response is a KeyringResponses.
    let body: serde_json::Value = test::read_body_json(resp).await;
    let responses = body["Responses"]
        .as_array()
        .expect("should have Responses array");
    assert!(!responses.is_empty());

    // Verify the new key appears in Keys.
    let keys = &responses[0]["Keys"];
    assert!(
        keys.get(&key).is_some(),
        "installed key should appear in keyring"
    );
}

/// `PUT /v1/operator/keyring` - switches the primary key.
#[actix_web::test]
async fn test_http_keyring_use() {
    let app = create_test_app().await;
    let key = http_test_key(0x99);

    // Install the key first.
    let req = test::TestRequest::post()
        .uri("/v1/operator/keyring")
        .set_json(serde_json::json!({ "Key": key }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Switches the primary key.
    let req = test::TestRequest::put()
        .uri("/v1/operator/keyring")
        .set_json(serde_json::json!({ "Key": key }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Verify the primary key has switched.
    let body: serde_json::Value = test::read_body_json(resp).await;
    let responses = body["Responses"]
        .as_array()
        .expect("should have Responses array");
    let primary_keys = &responses[0]["PrimaryKeys"];
    assert!(
        primary_keys.get(&key).is_some(),
        "new key should be in PrimaryKeys after use"
    );
}

/// `DELETE /v1/operator/keyring` - removes a non-primary key.
#[actix_web::test]
async fn test_http_keyring_remove() {
    let app = create_test_app().await;

    // Get the initial primary key.
    let req = test::TestRequest::get()
        .uri("/v1/operator/keyring")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let initial_primary = body["Responses"][0]["PrimaryKeys"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .cloned()
        .unwrap();

    // Install a new key (non-primary).
    let new_key = http_test_key(0x55);
    let req = test::TestRequest::post()
        .uri("/v1/operator/keyring")
        .set_json(serde_json::json!({ "Key": new_key }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Remove the new key (non-primary, should succeed).
    let req = test::TestRequest::delete()
        .uri("/v1/operator/keyring")
        .set_json(serde_json::json!({ "Key": new_key }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Verify the key has been removed.
    let body: serde_json::Value = test::read_body_json(resp).await;
    let keys = &body["Responses"][0]["Keys"];
    assert!(
        keys.get(&new_key).is_none(),
        "removed key should not be in keyring"
    );
    // The primary key should still exist.
    assert!(
        keys.get(&initial_primary).is_some(),
        "primary key should still be present"
    );
}

/// `DELETE /v1/operator/keyring` - removing the primary key returns 400.
#[actix_web::test]
async fn test_http_keyring_remove_primary_fails() {
    let app = create_test_app().await;

    // Get the initial primary key.
    let req = test::TestRequest::get()
        .uri("/v1/operator/keyring")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let primary_key = body["Responses"][0]["PrimaryKeys"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .cloned()
        .unwrap();

    // Attempting to remove the primary key should return 400.
    let req = test::TestRequest::delete()
        .uri("/v1/operator/keyring")
        .set_json(serde_json::json!({ "Key": primary_key }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "removing primary key should return 400"
    );
}

/// `POST /v1/operator/keyring` - invalid key format returns 400.
#[actix_web::test]
async fn test_http_keyring_invalid_key_format() {
    let app = create_test_app().await;

    // Invalid base64 key.
    let req = test::TestRequest::post()
        .uri("/v1/operator/keyring")
        .set_json(serde_json::json!({ "Key": "not-a-valid-key!!!" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    // Key with wrong length (16 bytes instead of 32).
    use base64::Engine;
    let short_key = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
    let req = test::TestRequest::post()
        .uri("/v1/operator/keyring")
        .set_json(serde_json::json!({ "Key": short_key }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

/// `POST /v1/operator/keyring?relay-factor=6` - relay_factor out of range returns 400.
#[actix_web::test]
async fn test_http_keyring_relay_factor_out_of_range() {
    let app = create_test_app().await;
    let key = http_test_key(0x33);

    let req = test::TestRequest::post()
        .uri("/v1/operator/keyring?relay-factor=6")
        .set_json(serde_json::json!({ "Key": key }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

/// `POST /v1/operator/keyring?local-only=true` - local_only on non-list operations returns 400.
#[actix_web::test]
async fn test_http_keyring_local_only_on_install_fails() {
    let app = create_test_app().await;
    let key = http_test_key(0x44);

    let req = test::TestRequest::post()
        .uri("/v1/operator/keyring?local-only=true")
        .set_json(serde_json::json!({ "Key": key }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "local-only should be rejected for non-list operations"
    );
}

#[actix_web::test]
async fn test_http_intentions_check_l7() {
    let app = create_test_app().await;

    // Create an L7 intention: allow GET on /api/* from web to api
    let create_req = test::TestRequest::post()
        .uri("/v1/connect/intentions")
        .set_json(serde_json::json!({
            "SourceName": "web",
            "DestinationName": "api",
            "Action": "deny",
            "Permissions": [{
                "Action": "allow",
                "HTTP": {
                    "PathPrefix": "/api/",
                    "Methods": ["GET"]
                }
            }]
        }))
        .to_request();
    let create_resp = test::call_service(&app, create_req).await;
    assert_eq!(create_resp.status(), 200);

    // GET /api/users → allowed
    let check_req = test::TestRequest::get()
        .uri("/v1/connect/intentions/check?source=web&destination=api&method=GET&path=/api/users")
        .to_request();
    let resp = test::call_service(&app, check_req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Allowed"], true);

    // POST /api/users → denied (method not GET)
    let check_req = test::TestRequest::get()
        .uri("/v1/connect/intentions/check?source=web&destination=api&method=POST&path=/api/users")
        .to_request();
    let resp = test::call_service(&app, check_req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Allowed"], false);

    // GET /health → denied (no permission matches)
    let check_req = test::TestRequest::get()
        .uri("/v1/connect/intentions/check?source=web&destination=api&method=GET&path=/health")
        .to_request();
    let resp = test::call_service(&app, check_req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Allowed"], false);
}

#[actix_web::test]
async fn test_http_intentions_create_invalid_permission_rejected() {
    let app = create_test_app().await;

    // Invalid HTTP method should return 400
    let req = test::TestRequest::post()
        .uri("/v1/connect/intentions")
        .set_json(serde_json::json!({
            "SourceName": "web",
            "DestinationName": "api",
            "Action": "allow",
            "Permissions": [{
                "Action": "allow",
                "HTTP": {
                    "Methods": ["FETCH"]
                }
            }]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "invalid HTTP method should be rejected"
    );
}

// ========================================================================
// ACL Login + Binding Rules HTTP Tests
// ========================================================================

/// Helper: create an auth method via HTTP.
async fn http_create_auth_method(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    name: &str,
    method_type: &str,
) {
    let req = test::TestRequest::put()
        .uri("/v1/acl/auth-method")
        .set_json(serde_json::json!({
            "Name": name,
            "Type": method_type,
        }))
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status(), 200, "auth method create should be 200");
}

/// Helper: create a binding rule via HTTP.
async fn http_create_binding_rule(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    auth_method: &str,
    bind_type: &str,
    bind_name: &str,
) {
    let req = test::TestRequest::put()
        .uri("/v1/acl/binding-rule")
        .set_json(serde_json::json!({
            "AuthMethod": auth_method,
            "BindType": bind_type,
            "BindName": bind_name,
        }))
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status(), 200, "binding rule create should be 200");
}

/// Login via HTTP and return the response body.
async fn http_login(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    auth_method: &str,
) -> serde_json::Value {
    let req = test::TestRequest::post()
        .uri("/v1/acl/login")
        .set_json(serde_json::json!({
            "AuthMethod": auth_method,
            "BearerToken": "test-token",
        }))
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status(), 200, "login should be 200");
    test::read_body_json(resp).await
}

/// `POST /v1/acl/login` - binding rule of type "policy" attaches the policy to the token.
#[actix_web::test]
async fn test_http_acl_login_applies_policy_binding_rule() {
    let app = create_test_app().await;

    // Create a policy.
    let req = test::TestRequest::put()
        .uri("/v1/acl/policy")
        .set_json(serde_json::json!({
            "Name": "login-test-policy",
            "Rules": "service_prefix \"\" { policy = \"read\" }",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Create auth method and a policy binding rule.
    http_create_auth_method(&app, "login-am", "kubernetes").await;
    http_create_binding_rule(&app, "login-am", "policy", "login-test-policy").await;

    // Login and verify the token carries the policy.
    let body = http_login(&app, "login-am").await;
    let policies = body["Policies"].as_array().expect("Policies should be an array");
    assert_eq!(policies.len(), 1, "token should have one policy from binding rule");
    assert_eq!(policies[0]["Name"], "login-test-policy");
}

/// `POST /v1/acl/login` - binding rule of type "role" attaches the role to the token.
#[actix_web::test]
async fn test_http_acl_login_applies_role_binding_rule() {
    let app = create_test_app().await;

    // Create a role.
    let req = test::TestRequest::put()
        .uri("/v1/acl/role")
        .set_json(serde_json::json!({
            "Name": "login-test-role",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Create auth method and a role binding rule.
    http_create_auth_method(&app, "login-role-am", "jwt").await;
    http_create_binding_rule(&app, "login-role-am", "role", "login-test-role").await;

    // Login and verify the token carries the role.
    let body = http_login(&app, "login-role-am").await;
    let roles = body["Roles"].as_array().expect("Roles should be an array");
    assert_eq!(roles.len(), 1, "token should have one role from binding rule");
    assert_eq!(roles[0]["Name"], "login-test-role");
}

/// `POST /v1/acl/login` - login against a nonexistent auth method returns 404.
#[actix_web::test]
async fn test_http_acl_login_nonexistent_auth_method() {
    let app = create_test_app().await;

    let req = test::TestRequest::post()
        .uri("/v1/acl/login")
        .set_json(serde_json::json!({
            "AuthMethod": "does-not-exist",
            "BearerToken": "x",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

// ========================================================================
// ACL Token/Policy/BindingRule CRUD + Replication Status Tests
// ========================================================================

/// `PUT /v1/acl/policy` then `PUT /v1/acl/policy/{id}` update should work.
#[actix_web::test]
async fn test_http_acl_policy_create_and_update() {
    let app = create_test_app().await;

    // Create policy
    let req = test::TestRequest::put()
        .uri("/v1/acl/policy")
        .set_json(serde_json::json!({
            "Name": "crud-test-policy",
            "Rules": "service_prefix \"\" { policy = \"read\" }",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let policy_id = body["ID"].as_str().expect("policy ID").to_string();

    // Update policy
    let req = test::TestRequest::put()
        .uri(&format!("/v1/acl/policy/{}", policy_id))
        .set_json(serde_json::json!({
            "Name": "crud-test-policy-updated",
            "Rules": "service_prefix \"\" { policy = \"write\" }",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Name"], "crud-test-policy-updated");
    assert_eq!(body["Rules"], "service_prefix \"\" { policy = \"write\" }");
}

/// `PUT /v1/acl/token` create then `PUT /v1/acl/token/{id}` update should work.
#[actix_web::test]
async fn test_http_acl_token_create_and_update() {
    let app = create_test_app().await;

    // Create token with bootstrap token auth
    let req = test::TestRequest::put()
        .uri("/v1/acl/token")
        .set_json(serde_json::json!({
            "Description": "crud-test-token",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let accessor_id = body["AccessorID"]
        .as_str()
        .expect("accessor ID")
        .to_string();

    // Update token
    let req = test::TestRequest::put()
        .uri(&format!("/v1/acl/token/{}", accessor_id))
        .set_json(serde_json::json!({
            "Description": "crud-test-token-updated",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["Description"], "crud-test-token-updated");
}

/// `PUT /v1/acl/binding-rule` create, update, delete lifecycle.
#[actix_web::test]
async fn test_http_acl_binding_rule_crud() {
    let app = create_test_app().await;

    // Create auth method first
    http_create_auth_method(&app, "crud-am", "jwt").await;

    // Create binding rule
    let req = test::TestRequest::put()
        .uri("/v1/acl/binding-rule")
        .set_json(serde_json::json!({
            "AuthMethod": "crud-am",
            "BindType": "service",
            "BindName": "web",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let rule_id = body["ID"].as_str().expect("rule ID").to_string();

    // Update binding rule
    let req = test::TestRequest::put()
        .uri(&format!("/v1/acl/binding-rule/{}", rule_id))
        .set_json(serde_json::json!({
            "AuthMethod": "crud-am",
            "BindType": "role",
            "BindName": "admin",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["BindType"], "role");
    assert_eq!(body["BindName"], "admin");

    // Delete binding rule
    let req = test::TestRequest::delete()
        .uri(&format!("/v1/acl/binding-rule/{}", rule_id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Verify deleted
    let req = test::TestRequest::get()
        .uri(&format!("/v1/acl/binding-rule/{}", rule_id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

/// `GET /v1/acl/replication` should return 200 with valid JSON.
#[actix_web::test]
async fn test_http_acl_replication_status() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/acl/replication")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["Enabled"].is_boolean());
    assert!(body["Running"].is_boolean());
    assert!(body["SourceType"].is_string() || body["ReplicationType"].is_string());
}

// ========================================================================
// Near / RTT Sorting Tests
// ========================================================================

/// `GET /v1/health/service/{service}?near=_agent` should return 200 and valid JSON.
#[actix_web::test]
async fn test_http_health_service_with_near_agent() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/health/service/web?near=_agent")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

/// `GET /v1/health/service/{service}?near=nonexistent` should still return 200
/// (no coordinates for the near node → results stay unsorted, not an error).
#[actix_web::test]
async fn test_http_health_service_with_near_nonexistent() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/health/service/web?near=nonexistent-node")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

/// `GET /v1/catalog/service/{service}?near=_agent` should return 200 and valid JSON.
#[actix_web::test]
async fn test_http_catalog_service_with_near_agent() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/catalog/service/web?near=_agent")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

// ========================================================================
// Connect Service Identity Tests
// ========================================================================

/// `GET /v1/connect/service/{service_id}` should return the full service identity
/// (SPIFFE ID, leaf cert, private key, CA roots) for a registered service.
#[actix_web::test]
async fn test_http_connect_service_identity_success() {
    let app = create_test_app().await;

    // Register a service
    let svc_json = serde_json::json!({
        "Name": "identity-web",
        "ID": "identity-web-1",
        "Port": 9090,
        "Address": "10.0.0.5"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc_json)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Fetch service identity
    let req = test::TestRequest::get()
        .uri("/v1/connect/service/identity-web-1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // X-Consul-Index header must be present (Consul-compatible metadata)
    assert!(resp.headers().get("X-Consul-Index").is_some());

    let body: serde_json::Value = test::read_body_json(resp).await;

    // Core identity fields
    assert_eq!(body["ServiceName"], "identity-web");
    assert_eq!(body["ServiceID"], "identity-web-1");

    // SPIFFE ID format: spiffe://{trust_domain}/ns/default/dc/{dc}/svc/{service}
    let service_uri = body["ServiceURI"].as_str().expect("ServiceURI missing");
    assert!(service_uri.starts_with("spiffe://"));
    assert!(service_uri.contains("/ns/default/"));
    assert!(service_uri.contains("/svc/identity-web"));

    // Leaf cert + private key must be PEM-encoded
    let cert_pem = body["CertPEM"].as_str().expect("CertPEM missing");
    assert!(cert_pem.contains("BEGIN CERTIFICATE"));
    assert!(cert_pem.contains("END CERTIFICATE"));

    let key_pem = body["PrivateKeyPEM"].as_str().expect("PrivateKeyPEM missing");
    assert!(key_pem.contains("BEGIN"));
    assert!(key_pem.contains("PRIVATE KEY"));

    // CA roots must be a non-empty array
    let roots = body["Roots"].as_array().expect("Roots missing");
    assert!(!roots.is_empty());
    assert!(roots[0]["RootCert"].as_str().unwrap_or("").contains("BEGIN CERTIFICATE"));

    // Trust domain and datacenter
    assert_eq!(body["TrustDomain"], "consul");
    assert_eq!(body["Datacenter"], "dc1");

    // Validity timestamps
    assert!(body["ValidAfter"].is_string());
    assert!(body["ValidBefore"].is_string());
}

/// `GET /v1/connect/service/{service_id}` for a non-existent service should return 404.
#[actix_web::test]
async fn test_http_connect_service_identity_not_found() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/connect/service/does-not-exist-123")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

/// `GET /v1/connect/service/{service_id}` SPIFFE ID must match the registered
/// service name even when the service ID differs from the name.
#[actix_web::test]
async fn test_http_connect_service_identity_spiffe_uses_service_name() {
    let app = create_test_app().await;

    // Service name differs from ID
    let svc_json = serde_json::json!({
        "Name": "billing-api",
        "ID": "billing-api-prod-42",
        "Port": 8080,
        "Address": "10.0.0.9"
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc_json)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/v1/connect/service/billing-api-prod-42")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let service_uri = body["ServiceURI"].as_str().expect("ServiceURI missing");

    // SPIFFE ID should embed the service *name* (billing-api), not the ID
    assert!(service_uri.contains("/svc/billing-api"));
    assert!(!service_uri.contains("billing-api-prod-42"));
}

// ========================================================================
// Discovery Chain Compilation Tests
// ========================================================================

/// A service-resolver with Subsets should produce one DiscoveryTarget per subset.
#[actix_web::test]
async fn test_http_discovery_chain_with_subsets() {
    let app = create_test_app().await;

    // Create a service-resolver with two subsets
    let resolver_entry = serde_json::json!({
        "Kind": "service-resolver",
        "Name": "web",
        "Subsets": {
            "v1": { "Filter": "v1 in Service.Tags" },
            "v2": { "Filter": "v2 in Service.Tags", "OnlyPassing": true }
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/config")
        .set_json(&resolver_entry)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Query the discovery chain
    let req = test::TestRequest::get()
        .uri("/v1/discovery-chain/web")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let chain = &body["Chain"];

    // Should have 3 targets: default + v1 + v2
    let targets = chain["Targets"].as_object().expect("Targets missing");
    assert_eq!(targets.len(), 3);

    // Verify subset targets exist
    let v1_id = "web.v1.default.default.dc1";
    let v2_id = "web.v2.default.default.dc1";
    assert!(targets.contains_key(v1_id), "v1 subset target missing");
    assert!(targets.contains_key(v2_id), "v2 subset target missing");

    // Verify subset filter and only_passing
    assert_eq!(targets[v1_id]["Subset"]["Filter"], "v1 in Service.Tags");
    assert_eq!(targets[v1_id]["Subset"]["OnlyPassing"], false);
    assert_eq!(targets[v2_id]["Subset"]["Filter"], "v2 in Service.Tags");
    assert_eq!(targets[v2_id]["Subset"]["OnlyPassing"], true);

    // Default target has no subset
    let default_id = "web.default.default.dc1";
    assert_eq!(targets[default_id]["ServiceSubset"], "");
}

/// DefaultSubset should make the resolver point to that subset's target.
#[actix_web::test]
async fn test_http_discovery_chain_default_subset() {
    let app = create_test_app().await;

    let resolver_entry = serde_json::json!({
        "Kind": "service-resolver",
        "Name": "api",
        "DefaultSubset": "stable",
        "Subsets": {
            "stable": { "Filter": "stable in Service.Tags" },
            "canary": { "Filter": "canary in Service.Tags" }
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/config")
        .set_json(&resolver_entry)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/v1/discovery-chain/api")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let chain = &body["Chain"];

    // Find the resolver node
    let nodes = chain["Nodes"].as_object().expect("Nodes missing");
    let resolver_node = nodes
        .values()
        .find(|n| n["Type"] == "resolver")
        .expect("resolver node missing");

    // The resolver's target should be the stable subset target
    let target_id = resolver_node["Resolver"]["Target"].as_str().unwrap();
    assert!(target_id.contains(".stable."), "resolver should target stable subset, got: {}", target_id);
}

/// Failover targets should produce resolver nodes + DiscoveryTargets for each.
#[actix_web::test]
async fn test_http_discovery_chain_with_failover() {
    let app = create_test_app().await;

    let resolver_entry = serde_json::json!({
        "Kind": "service-resolver",
        "Name": "web",
        "Failover": {
            "Targets": ["backup", "backup.team-a.dc2"]
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/config")
        .set_json(&resolver_entry)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/v1/discovery-chain/web")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let chain = &body["Chain"];

    // 1 default target + 2 failover targets = 3
    let targets = chain["Targets"].as_object().expect("Targets missing");
    assert_eq!(targets.len(), 3);

    // Failover targets should exist
    assert!(targets.contains_key("backup.default.default.dc1"));
    assert!(targets.contains_key("backup.team-a.default.dc2"));

    // Cross-DC failover target should have .alt.consul SNI
    let dc2_target = &targets["backup.team-a.default.dc2"];
    assert!(dc2_target["SNI"].as_str().unwrap().ends_with(".alt.consul"));

    // The primary resolver's failover should list the failover resolver node names
    let nodes = chain["Nodes"].as_object().expect("Nodes missing");
    // There should be 3 resolver nodes: 1 primary + 2 failover
    let resolver_nodes: Vec<_> = nodes
        .values()
        .filter(|n| n["Type"] == "resolver")
        .collect();
    assert_eq!(resolver_nodes.len(), 3);

    // Find primary resolver (has failover config)
    let primary = resolver_nodes
        .iter()
        .find(|n| n["Resolver"]["Failover"].is_object())
        .expect("primary resolver with failover missing");
    let failover_targets = primary["Resolver"]["Failover"]["Targets"]
        .as_array()
        .expect("Failover.Targets missing");
    assert_eq!(failover_targets.len(), 2);

    // Failover resolver nodes should NOT have their own failover (no recursion)
    let failover_resolvers: Vec<_> = resolver_nodes
        .iter()
        .filter(|n| n["Resolver"]["Failover"].is_null())
        .collect();
    assert_eq!(failover_resolvers.len(), 2);
}

/// MeshGateway mode from service-resolver should propagate to all targets.
#[actix_web::test]
async fn test_http_discovery_chain_mesh_gateway_mode() {
    let app = create_test_app().await;

    let resolver_entry = serde_json::json!({
        "Kind": "service-resolver",
        "Name": "web",
        "MeshGateway": { "Mode": "local" },
        "Failover": {
            "Targets": ["backup.default.dc2"]
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/config")
        .set_json(&resolver_entry)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/v1/discovery-chain/web")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let chain = &body["Chain"];

    let targets = chain["Targets"].as_object().expect("Targets missing");
    for target in targets.values() {
        assert_eq!(target["MeshGateway"]["Mode"], "local");
        // With mesh gateway mode != none, SNI should use .alt.consul
        assert!(target["SNI"].as_str().unwrap().ends_with(".alt.consul"));
    }
}

// ========================================================================
// Connect Proxy Config (Envoy Bootstrap) Tests
// ========================================================================

/// `GET /v1/connect/proxy/{service_id}` should return a valid Envoy v3
/// bootstrap configuration for a registered connect-proxy service, including
/// node identity, admin endpoint, and ADS xDS config.
#[actix_web::test]
async fn test_http_connect_proxy_config_success() {
    let app = create_test_app().await;

    // Register a connect-proxy service with a custom envoy admin port.
    let svc_json = serde_json::json!({
        "Name": "web-sidecar-proxy",
        "ID": "web-sidecar-proxy-1",
        "Kind": "connect-proxy",
        "Port": 20000,
        "Address": "127.0.0.1",
        "Proxy": {
            "DestinationServiceName": "web",
            "Config": {
                "envoy_admin_bind_port": 19001
            }
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc_json)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Fetch the Envoy bootstrap config.
    let req = test::TestRequest::get()
        .uri("/v1/connect/proxy/web-sidecar-proxy-1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("X-Consul-Index").is_some());

    let body: serde_json::Value = test::read_body_json(resp).await;

    // Node identity.
    assert_eq!(body["node"]["id"], "web-sidecar-proxy-1");
    assert_eq!(body["node"]["cluster"], "dc1");

    // Node metadata must carry Consul-specific fields.
    let meta = &body["node"]["metadata"];
    assert_eq!(meta["CONSUL_DC"], "dc1");
    assert_eq!(meta["CONSUL_SERVICE_NAME"], "web-sidecar-proxy");
    assert_eq!(meta["CONSUL_SERVICE_ID"], "web-sidecar-proxy-1");
    assert_eq!(meta["CONSUL_NAMESPACE"], "default");
    assert_eq!(meta["CONSUL_PARTITION"], "default");
    assert_eq!(meta["CONSUL_PROXY_ID"], "web-sidecar-proxy-1");
    assert_eq!(meta["CONSUL_PROXY_SERVICE_NAME"], "web-sidecar-proxy");

    // Admin endpoint must use the configured envoy_admin_bind_port.
    assert_eq!(body["admin"]["address"]["socket_address"]["address"], "127.0.0.1");
    assert_eq!(body["admin"]["address"]["socket_address"]["port_value"], 19001);

    // Dynamic resources: LDS and CDS must use ADS.
    assert_eq!(body["dynamic_resources"]["lds_config"]["resource_api_version"], "V3");
    assert_eq!(body["dynamic_resources"]["cds_config"]["resource_api_version"], "V3");
    assert_eq!(body["dynamic_resources"]["ads_config"]["api_type"], "GRPC");
    assert_eq!(
        body["dynamic_resources"]["ads_config"]["grpc_services"][0]["envoy_grpc"]["cluster_name"],
        "local_agent"
    );

    // Static resources: local_agent cluster must point at the agent gRPC port.
    let clusters = body["static_resources"]["clusters"].as_array().expect("clusters missing");
    assert!(!clusters.is_empty());
    let local_agent = &clusters[0];
    assert_eq!(local_agent["name"], "local_agent");
    assert_eq!(local_agent["connect_timeout"], "1s");
    assert_eq!(local_agent["type"], "STRICT_DNS");
    let endpoint_addr = &local_agent["load_assignment"]["endpoints"][0]["lb_endpoints"][0]
        ["endpoint"]["address"]["socket_address"];
    assert_eq!(endpoint_addr["address"], "127.0.0.1");
    // gRPC port defaults to consul_port + 2 (8500 + 2 = 8502 in tests).
    assert_eq!(endpoint_addr["port_value"], 8502);
}

/// When `envoy_admin_bind_port` is not specified in the proxy config, the
/// admin endpoint should fall back to the Consul default port 19000.
#[actix_web::test]
async fn test_http_connect_proxy_config_default_admin_port() {
    let app = create_test_app().await;

    let svc_json = serde_json::json!({
        "Name": "web-proxy",
        "ID": "web-proxy-default",
        "Kind": "connect-proxy",
        "Port": 20000,
        "Address": "127.0.0.1",
        "Proxy": {
            "DestinationServiceName": "web"
        }
    });
    let req = test::TestRequest::put()
        .uri("/v1/agent/service/register")
        .set_json(&svc_json)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/v1/connect/proxy/web-proxy-default")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["admin"]["address"]["socket_address"]["port_value"], 19000);
}

/// `GET /v1/connect/proxy/{service_id}` for a non-existent service returns 404.
#[actix_web::test]
async fn test_http_connect_proxy_config_not_found() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/v1/connect/proxy/does-not-exist-xyz")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}
