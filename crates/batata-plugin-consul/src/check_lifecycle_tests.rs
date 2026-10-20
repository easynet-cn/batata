//! Consul check lifecycle: TTL expiry and critical-service reaping.
//!
//! These drive the shared health-check monitors one pass at a time instead of
//! waiting on their background interval, so the assertions stay deterministic.
//! The wiring mirrors what the plugin sets up in `plugin::start`, with the Raft
//! writer left out so the suite exercises the standalone path.

use std::sync::Arc;
use std::time::Duration;

use batata_naming::healthcheck::deregister_monitor::DeregisterMonitor;
use batata_naming::healthcheck::registry::{CheckStatus, InstanceCheckRegistry};
use batata_naming::healthcheck::ttl_monitor::TtlMonitor;
use batata_plugin::PluginNamingStore;
use bytes::Bytes;

use crate::check_index::ConsulCheckIndex;
use crate::health::ConsulHealthService;
use crate::index_provider::ConsulIndexProvider;
use crate::kv::ConsulKVService;
use crate::model::CheckRegistration;
use crate::naming_store::ConsulNamingStore;
use crate::result_handler::ConsulResultHandler;
use crate::session::ConsulSessionService;

const NODE: &str = "c1";

struct Fixture {
    store: Arc<ConsulNamingStore>,
    registry: Arc<InstanceCheckRegistry>,
    health: ConsulHealthService,
    check_index: Arc<ConsulCheckIndex>,
}

impl Fixture {
    fn new() -> Self {
        let store = Arc::new(ConsulNamingStore::new());
        let handler = Arc::new(ConsulResultHandler::new(
            store.clone(),
            Arc::new(ConsulIndexProvider::new()),
        ));
        let registry = Arc::new(InstanceCheckRegistry::new(handler.clone()));
        let check_index = Arc::new(ConsulCheckIndex::new());
        handler.set_services(
            check_index.clone(),
            Arc::new(ConsulSessionService::new()),
            Arc::new(ConsulKVService::new()),
            None,
            NODE.to_string(),
        );
        let health = ConsulHealthService::new(registry.clone(), check_index.clone())
            .with_node_name(NODE.to_string());
        Self {
            store,
            registry,
            health,
            check_index,
        }
    }

    fn register_service(&self, name: &str, id: &str, address: &str, port: u16) {
        let data = serde_json::to_vec(&serde_json::json!({
            "Name": name, "ID": id, "Port": port, "Address": address
        }))
        .expect("serialize registration");
        let key = ConsulNamingStore::build_key("default", name, id);
        PluginNamingStore::register(&*self.store, &key, Bytes::from(data)).expect("register");
    }

    async fn register_ttl_check(
        &self,
        check_id: &str,
        service_id: Option<&str>,
        ttl: &str,
        deregister_after: Option<&str>,
    ) {
        let registration = CheckRegistration {
            name: check_id.to_string(),
            check_id: Some(check_id.to_string()),
            service_id: service_id.map(str::to_string),
            service_name: service_id.map(str::to_string),
            ttl: Some(ttl.to_string()),
            deregister_critical_service_after: deregister_after.map(str::to_string),
            status: Some("passing".to_string()),
            ..Default::default()
        };
        self.health
            .register_check(registration)
            .await
            .expect("register check");
        if let Some(service_id) = service_id {
            self.check_index.register_check(check_id, service_id);
        }
    }
}

/// A TTL check that stops being renewed goes critical with Consul's wording
/// (`checks.CheckTTL.getExpiredOutput`).
#[tokio::test]
async fn ttl_expiry_marks_check_critical() {
    let f = Fixture::new();
    f.register_service("ttl-svc", "ttl-1", "10.1.0.1", 8080);
    f.register_ttl_check("ttl-chk", Some("ttl-1"), "80ms", None)
        .await;

    let key = f.health.check_key("ttl-chk");
    let monitor = TtlMonitor::new(f.registry.clone());

    monitor.run_once().await;
    let (_, status) = f.registry.get_check(&key).expect("check registered");
    assert_eq!(
        status.status,
        CheckStatus::Passing,
        "a freshly reported check must not expire"
    );

    tokio::time::sleep(Duration::from_millis(120)).await;
    monitor.run_once().await;

    let (_, status) = f.registry.get_check(&key).expect("check registered");
    assert_eq!(status.status, CheckStatus::Critical);
    assert!(
        status.output.starts_with("TTL expired"),
        "Consul reports the expiry in the check output, got: {:?}",
        status.output
    );
}

/// Reporting a TTL check renews it, so it survives past its own TTL.
#[tokio::test]
async fn ttl_update_renews_before_expiry() {
    let f = Fixture::new();
    f.register_service("ttl-svc", "ttl-1", "10.1.0.2", 8080);
    f.register_ttl_check("ttl-chk", Some("ttl-1"), "150ms", None)
        .await;

    let key = f.health.check_key("ttl-chk");
    let monitor = TtlMonitor::new(f.registry.clone());

    tokio::time::sleep(Duration::from_millis(100)).await;
    f.registry
        .ttl_update(&key, CheckStatus::Passing, Some("still alive".to_string()))
        .await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    monitor.run_once().await;
    let (_, status) = f.registry.get_check(&key).expect("check registered");
    assert_eq!(
        status.status,
        CheckStatus::Passing,
        "renewing inside the TTL must keep the check alive"
    );
}

/// A service whose check stays critical past `DeregisterCriticalServiceAfter`
/// is removed along with its checks (Consul `Agent.reapServicesInternal`).
#[tokio::test]
async fn critical_service_is_deregistered_after_threshold() {
    let f = Fixture::new();
    f.register_service("dcr-svc", "dcr-1", "10.2.0.1", 9090);
    f.register_ttl_check("dcr-chk", Some("dcr-1"), "10s", Some("20ms"))
        .await;

    let key = f.health.check_key("dcr-chk");
    let monitor = DeregisterMonitor::new(f.registry.clone(), 5);

    f.registry
        .ttl_update(&key, CheckStatus::Critical, Some("boom".to_string()))
        .await;

    monitor.reap_critical_instances();
    assert_eq!(
        f.store.len(),
        1,
        "a check that only just went critical must not reap its service"
    );

    tokio::time::sleep(Duration::from_millis(60)).await;
    monitor.reap_critical_instances();

    assert_eq!(
        f.store.len(),
        0,
        "the service must be deregistered once it has been critical too long"
    );
    assert!(
        f.registry.get_check(&key).is_none(),
        "the service's checks go away with it"
    );
}

/// A check without a service (Consul node-level check) has nothing to reap —
/// even when it shares an address with a real service.
#[tokio::test]
async fn node_level_check_is_never_auto_deregistered() {
    let f = Fixture::new();
    f.register_service("victim-svc", "victim-1", "10.3.0.1", 8080);

    let registration = CheckRegistration {
        name: "node-chk".to_string(),
        check_id: Some("node-chk".to_string()),
        ip: Some("10.3.0.1".to_string()),
        port: Some(8080),
        ttl: Some("10s".to_string()),
        deregister_critical_service_after: Some("1ms".to_string()),
        status: Some("passing".to_string()),
        ..Default::default()
    };
    f.health
        .register_check(registration)
        .await
        .expect("register check");

    let key = f.health.check_key("node-chk");
    f.registry
        .ttl_update(&key, CheckStatus::Critical, Some("down".to_string()))
        .await;
    tokio::time::sleep(Duration::from_millis(20)).await;

    DeregisterMonitor::new(f.registry.clone(), 5).reap_critical_instances();

    assert_eq!(
        f.store.len(),
        1,
        "upstream skips reaping when the check owns no service; the IP:Port \
         fallback must not remove an unrelated service"
    );
}
