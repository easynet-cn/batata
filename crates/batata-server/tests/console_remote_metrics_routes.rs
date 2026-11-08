//! Route-level regression for the separated-console metrics endpoints.
//!
//! `DeploymentMode::Console` builds `PersistenceContext::empty()`, which leaves
//! `AppState::cluster_manager` as `None`. `/v3/console/metrics` and
//! `/v3/console/prometheus/query` used the panicking `cluster_manager()`
//! accessor; with `panic = "abort"` in the release profile a single scrape
//! killed the whole process.
//!
//! These tests mount the **production** route table
//! (`batata_console::configure_v3_console_routes`) with the console-remote
//! AppState shape, so removing the `try_cluster_manager()` guard makes them
//! fail instead of silently passing.

#[path = "common/ai_store.rs"]
mod ai_store;

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

/// The console-remote datasource still needs a cluster manager for its own
/// bookkeeping, but the metrics handlers must not reach for `AppState`'s one.
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
        unimplemented!("not used by the metrics routes")
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

/// Build the console route table exactly as `main_server` does, but with the
/// console-remote AppState: no cluster manager, no raft node, auth disabled.
async fn build_console_remote_app(
    store: Arc<dyn PersistenceService>,
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
        // This is the field that is None in a separated console.
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

    test::init_service(
        App::new()
            // `Data::from` mirrors production: it registers `Data<AppState>`.
            .app_data(web::Data::from(app_state))
            .service(
                web::scope("/v3/console").configure(batata_console::configure_v3_console_routes),
            ),
    )
    .await
}

#[actix_web::test]
async fn metrics_endpoint_is_served_without_a_cluster_manager() {
    let store = TestStore::open().await;
    let app = build_console_remote_app(store.store()).await;

    let req = test::TestRequest::get()
        .uri("/v3/console/metrics")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let text = String::from_utf8(body.to_vec()).expect("metrics body is UTF-8");
    assert!(
        text.contains("batata_cluster_size 0"),
        "expected the degraded cluster gauge, got: {text}"
    );
}

#[actix_web::test]
async fn prometheus_query_is_served_without_a_cluster_manager() {
    let store = TestStore::open().await;
    let app = build_console_remote_app(store.store()).await;

    let req = test::TestRequest::get()
        .uri("/v3/console/prometheus/query")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "success");
    assert!(
        body["data"]["result"]
            .as_str()
            .unwrap_or_default()
            .contains("batata_cluster_size 0"),
        "expected the degraded cluster gauge, got: {body}"
    );
}
