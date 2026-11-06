//! Console `/v3/console/cs/config/clone` contract tests.
//!
//! The embedded backend reports `id: 0` for every config and its
//! `config_find_by_ids` intentionally returns an empty list. An id-only clone
//! therefore answered `200 code=0 successCount=0` while cloning nothing, and
//! the UI reported success. These tests drive the real production route with
//! the real `LocalDataSource`/`AppState` wiring.

#[path = "common/ai_store.rs"]
mod ai_store;

use std::sync::Arc;

use actix_web::{App, test, web};
use ai_store::TestStore;
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;
use serde_json::{Value, json};

/// Cluster manager stub — the clone handlers never read cluster state.
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
        unimplemented!("not used by config clone")
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
            // Production registers `Data<AppState>` via `Data::from`.
            .app_data(web::Data::from(app_state))
            .service(batata_console::v3_routes()),
    )
    .await
}

async fn seed(store: &Arc<dyn PersistenceService>, data_id: &str, ns: &str, content: &str) {
    store
        .config_create_or_update(
            data_id,
            "DEFAULT_GROUP",
            ns,
            content,
            "",
            "tester",
            "127.0.0.1",
            "",
            "",
            "",
            "",
            "yaml",
            "",
            "",
            None,
        )
        .await
        .expect("seed source config");
}

async fn clone(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    data_id: &str,
    src_ns: Option<&str>,
    target_ns: &str,
    policy: &str,
) -> (u16, Value) {
    let mut uri =
        format!("/v3/console/cs/config/clone?targetNamespaceId={target_ns}&policy={policy}");
    if let Some(ns) = src_ns {
        uri.push_str(&format!("&namespaceId={ns}"));
    }
    let resp = test::call_service(
        app,
        test::TestRequest::post()
            .uri(&uri)
            .set_json(json!([{ "cfgId": 0, "dataId": data_id, "group": "DEFAULT_GROUP" }]))
            .to_request(),
    )
    .await;
    let status = resp.status().as_u16();
    let body = test::read_body(resp).await;
    let value = serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null);
    (status, value)
}

#[actix_web::test]
async fn clone_resolves_zero_id_by_natural_key_and_source_namespace() {
    let ts = TestStore::open().await;
    let store = ts.store();
    let app = build_app(store.clone()).await;
    let data_id = format!("clone-{}.yaml", chrono::Utc::now().timestamp_millis());
    seed(&store, &data_id, "ns-src", "key: source\n").await;

    let (status, body) = clone(&app, &data_id, Some("ns-src"), "ns-target", "ABORT").await;
    assert_eq!(status, 200, "clone must be registered: {body:?}");
    assert_eq!(body["code"], 0);
    assert_eq!(
        body["data"]["successCount"], 1,
        "cfgId=0 must fall back to (dataId, group, namespaceId): {body:?}"
    );
    assert_eq!(body["data"]["failCount"], 0);

    let target = store
        .config_find_one(&data_id, "DEFAULT_GROUP", "ns-target")
        .await
        .expect("read target")
        .unwrap_or_else(|| panic!("clone wrote nothing into the target namespace: {body:?}"));
    assert_eq!(target.content, "key: source\n");

    // Without namespaceId the server must default the source to the target
    // namespace (upstream ConsoleConfigController semantics), so cloning an
    // ns-src-only config into an empty target must not fabricate a copy.
    let missing = format!("{data_id}.missing");
    let (_, body) = clone(&app, &missing, None, "ns-empty", "ABORT").await;
    assert_eq!(body["data"]["successCount"], 0);
    assert_eq!(body["data"]["failCount"], 1);
}

#[actix_web::test]
async fn clone_honours_abort_skip_and_overwrite_policies() {
    let ts = TestStore::open().await;
    let store = ts.store();
    let app = build_app(store.clone()).await;
    let data_id = format!("policy-{}.yaml", chrono::Utc::now().timestamp_millis());
    seed(&store, &data_id, "public", "v1").await;

    let (_, first) = clone(&app, &data_id, Some("public"), "ns-policy", "ABORT").await;
    assert_eq!(first["data"]["successCount"], 1, "{first:?}");

    // ABORT: conflict must be reported, not silently ignored.
    let (_, aborted) = clone(&app, &data_id, Some("public"), "ns-policy", "ABORT").await;
    assert_eq!(aborted["data"]["successCount"], 0, "{aborted:?}");
    assert_eq!(
        aborted["data"]["failData"][0]["reason"],
        "Config already exists"
    );

    // SKIP: counted as skipped, no write.
    let (_, skipped) = clone(&app, &data_id, Some("public"), "ns-policy", "SKIP").await;
    assert_eq!(skipped["data"]["skipCount"], 1, "{skipped:?}");
    assert_eq!(skipped["data"]["successCount"], 0);

    // OVERWRITE: the new source content must replace the target content.
    seed(&store, &data_id, "public", "v2").await;
    let (_, overwritten) = clone(&app, &data_id, Some("public"), "ns-policy", "OVERWRITE").await;
    assert_eq!(overwritten["data"]["successCount"], 1, "{overwritten:?}");
    let target = store
        .config_find_one(&data_id, "DEFAULT_GROUP", "ns-policy")
        .await
        .expect("read target")
        .expect("target config");
    assert_eq!(
        target.content, "v2",
        "OVERWRITE must actually update content"
    );
}
