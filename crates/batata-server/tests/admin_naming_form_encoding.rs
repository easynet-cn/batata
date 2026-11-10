//! V3 admin naming service write routes must accept both parameter encodings.
//!
//! Nacos maintainer-client <= 3.2.4 posts service create/update fields as an
//! `application/x-www-form-urlencoded` body (`setParamValue`). 3.3.0 GA moved
//! the same fields into the URL query string (`appendQuery`). These tests drive
//! the real production route with a real `NamingService` so removing either
//! source turns a test red instead of silently breaking the Java SDK.

#[path = "common/ai_store.rs"]
#[allow(dead_code)]
mod ai_store;

use std::sync::Arc;

use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;
use serde_json::{Value, json};

/// Cluster manager stub — the service write routes never read cluster state.
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
        unimplemented!("not used by admin naming writes")
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

async fn build_app(
    store: Arc<dyn PersistenceService>,
) -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
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

    // Production registers `Data<Arc<dyn NamingServiceProvider>>`; using
    // `Data<Arc<...>>` here (not `Data::from` of the trait object itself)
    // keeps the test wiring identical to the handler's extractor.
    let naming: Arc<dyn batata_api::naming::NamingServiceProvider> =
        Arc::new(batata_naming::service::NamingService::new());

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .app_data(web::Data::new(naming))
            .service(
                web::scope("/v3/admin/ns")
                    .service(batata_naming::api::v3::admin::service::routes()),
            ),
    )
    .await
}

async fn call<S>(app: &S, req: actix_http::Request) -> (actix_web::http::StatusCode, Value)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    let body = test::read_body(resp).await;
    (
        status,
        serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null),
    )
}

fn post_query(uri: &str) -> actix_http::Request {
    test::TestRequest::post().uri(uri).to_request()
}

fn post_form(body: &str) -> actix_http::Request {
    test::TestRequest::post()
        .uri("/v3/admin/ns/service")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload(body.to_string())
        .to_request()
}

/// The exact 3.3.0 GA `RequestUtil.toParameters(Service)` shape: all fields in
/// the URL query string, `metadata`/`selector` as JSON text.
const GA_CREATE_QUERY: &str = "/v3/admin/ns/service?namespaceId=public\
    &groupName=DEFAULT_GROUP&serviceName=ga-query-svc&metadata=%7B%7D\
    &ephemeral=true&protectThreshold=0.8\
    &selector=%7B%22type%22%3A%22none%22%7D";

#[actix_web::test]
async fn create_service_accepts_ga_330_query_params() {
    let ts = TestStore::open().await;
    let app = build_app(ts.store()).await;

    let (status, body) = call(&app, post_query(GA_CREATE_QUERY)).await;
    assert_eq!(status, 200, "GA query create: {body}");
    assert_eq!(body["code"], json!(0), "GA query create: {body}");
}

#[actix_web::test]
async fn create_service_still_accepts_legacy_form_body() {
    let ts = TestStore::open().await;
    let app = build_app(ts.store()).await;

    let (status, body) = call(
        &app,
        post_form("serviceName=legacy-svc&protectThreshold=0.5"),
    )
    .await;
    assert_eq!(status, 200, "legacy form create: {body}");
    assert_eq!(body["code"], json!(0), "legacy form create: {body}");
}

#[actix_web::test]
async fn update_service_accepts_ga_330_query_params() {
    let ts = TestStore::open().await;
    let app = build_app(ts.store()).await;

    let (status, body) = call(&app, post_form("serviceName=update-svc")).await;
    assert_eq!(status, 200, "seed create: {body}");

    let (status, body) = call(
        &app,
        test::TestRequest::put()
            .uri("/v3/admin/ns/service?serviceName=update-svc&protectThreshold=0.9")
            .to_request(),
    )
    .await;
    assert_eq!(status, 200, "GA query update: {body}");
    assert_eq!(body["code"], json!(0), "GA query update: {body}");
}

#[actix_web::test]
async fn query_params_take_precedence_over_the_body() {
    let ts = TestStore::open().await;
    let app = build_app(ts.store()).await;

    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/admin/ns/service?serviceName=query-wins")
            .insert_header(("content-type", "application/x-www-form-urlencoded"))
            .set_payload("serviceName=body-wins".to_string())
            .to_request(),
    )
    .await;
    assert_eq!(status, 200, "mixed sources create: {body}");
    assert_eq!(body["code"], json!(0), "mixed sources create: {body}");

    // Only the query-side service must have been created.
    let (_, found) = call(
        &app,
        test::TestRequest::get()
            .uri("/v3/admin/ns/service?serviceName=query-wins")
            .to_request(),
    )
    .await;
    assert_eq!(found["code"], json!(0), "query side missing: {found}");
    let (_, missing) = call(
        &app,
        test::TestRequest::get()
            .uri("/v3/admin/ns/service?serviceName=body-wins")
            .to_request(),
    )
    .await;
    assert_ne!(
        missing["code"],
        json!(0),
        "body side must not exist: {missing}"
    );
}

#[actix_web::test]
async fn missing_form_returns_a_structured_400() {
    let ts = TestStore::open().await;
    let app = build_app(ts.store()).await;

    let (status, body) = call(
        &app,
        test::TestRequest::post()
            .uri("/v3/admin/ns/service")
            .to_request(),
    )
    .await;
    assert_eq!(status, 400, "missing form: {body}");
    assert_ne!(body["code"], json!(0), "missing form: {body}");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|m| m.contains("serviceName")),
        "missing form must name the missing parameter: {body}"
    );
}
