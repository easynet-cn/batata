//! Route-level test for the Kubernetes health probes.
//!
//! `/health/liveness` and `/health/readiness` are mounted on the main server
//! and are what a k8s probe hits, so a failure here is invisible in ordinary
//! API testing — the server still serves every other route.
//!
//! The reason they need a test at all: actix keys application data by the
//! whole `Data<T>` type, so a handler declaring `web::Data<Arc<AppState>>`
//! extracts a type that no server registers (they all `Data::from` an
//! `Arc<AppState>`, which yields `Data<AppState>`). That mismatch is a runtime
//! 500 and `cargo check` cannot see it. `health_readiness` declared exactly
//! that and returned 500 on every probe.
//!
//! The test mounts `batata_server::startup::http::health_routes()` — the same
//! function `main_server` uses — so the wiring under test is the production
//! wiring.

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

/// Cluster manager stub. The health probes never read cluster state.
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
        unimplemented!("not used by the health probes")
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

/// Build an app mounting the health routes exactly as `main_server` does:
/// `Data::from` on an `Arc<AppState>`, which registers `Data<AppState>`.
async fn build_app(
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

    test::init_service(
        App::new()
            // Deliberately `Data::from`, not `Data::new`: this mirrors
            // `main_server`, and the two spellings register different types.
            .app_data(web::Data::from(app_state))
            .service(batata_server::startup::health_routes()),
    )
    .await
}

#[actix_web::test]
async fn liveness_reports_up() {
    let store = TestStore::open().await;
    let app = build_app(store.store()).await;

    let req = test::TestRequest::get()
        .uri("/health/liveness")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "UP");
}

/// The regression: `health_readiness` declared `web::Data<Arc<AppState>>`,
/// a type no server registers, so actix answered 500 instead of probing.
#[actix_web::test]
async fn readiness_reports_up_when_persistence_is_healthy() {
    let store = TestStore::open().await;
    let app = build_app(store.store()).await;

    let req = test::TestRequest::get()
        .uri("/health/readiness")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "UP");
}
