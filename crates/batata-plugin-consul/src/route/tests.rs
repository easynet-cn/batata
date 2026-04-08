use std::sync::Arc;

use actix_web::{App, test, web};

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
use crate::lock::{ConsulLockService, ConsulSemaphoreService};
use crate::query::ConsulQueryService;
use crate::session::ConsulSessionService;
use crate::snapshot::ConsulSnapshotService;

/// Create a test app with all in-memory services configured.
/// Uses `crate::api::v1::routes()` for route registration.
async fn create_test_app() -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
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
    let kv_arc = Arc::new(kv_service.clone());
    let session_arc = Arc::new(session_service.clone());
    let lock_service = ConsulLockService::new(kv_arc.clone(), session_arc.clone());
    let semaphore_service = ConsulSemaphoreService::new(kv_arc, session_arc);
    let catalog_service = ConsulCatalogService::new(naming_store.clone());
    let config_entry_service = ConsulConfigEntryService::new();
    let connect_service = ConsulConnectService::new();
    let connect_ca_service = ConsulConnectCAService::new();

    test::init_service(
        App::new()
            .app_data(web::Data::new(kv_service))
            .app_data(web::Data::new(session_service))
            .app_data(web::Data::new(health_service))
            .app_data(web::Data::new(agent_service))
            .app_data(web::Data::new(acl_service))
            .app_data(web::Data::new(event_service))
            .app_data(web::Data::new(snapshot_service))
            .app_data(web::Data::new(query_service))
            .app_data(web::Data::new(lock_service))
            .app_data(web::Data::new(semaphore_service))
            .app_data(web::Data::new(catalog_service))
            .app_data(web::Data::new(config_entry_service))
            .app_data(web::Data::new(connect_service))
            .app_data(web::Data::new(connect_ca_service))
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
                crate::index_provider::ConsulIndexProvider::new(),
            ))
            .service(crate::api::v1::routes()),
    )
    .await
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
    assert!(version.contains("batata"));
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
    // Metrics proxy returns 404 (not implemented)
    assert_eq!(resp.status(), 404);
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
