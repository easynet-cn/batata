//! Shared fixtures for the in-process naming test suite.
//!
//! These helpers let naming semantics be exercised without booting a
//! `batata-server` process: `NamingService` is a plain in-memory structure, so
//! register / query / deregister round-trips can be asserted directly. Keeping
//! the fixtures here means the per-scenario test binaries (#66-#70) stay short
//! and consistent.
//!
//! Deliberately **not** provided: any HTTP/gRPC client, any RocksDB handle, any
//! global config. If a test needs those, it belongs in
//! `batata-integration-tests` (which drives a live server), not here.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use batata_api::naming::model::Instance;
use batata_naming::NamingService;
use batata_naming::service::{ClientOperationService, InstancePatch, apply_instance_patch};

/// Default namespace / group used by the fixtures unless a test overrides them.
pub const NS: &str = "public";
pub const GROUP: &str = "DEFAULT_GROUP";

/// A fresh, empty naming service backed only by memory.
pub fn service() -> NamingService {
    NamingService::new()
}

/// Build an ephemeral instance in the default `DEFAULT` cluster.
pub fn instance(ip: &str, port: i32) -> Instance {
    let mut i = Instance::new(ip.to_string(), port);
    i.cluster_name = "DEFAULT".to_string();
    i
}

/// Build an instance pinned to an explicit cluster.
pub fn instance_in_cluster(ip: &str, port: i32, cluster: &str) -> Instance {
    let mut i = Instance::new(ip.to_string(), port);
    i.cluster_name = cluster.to_string();
    i
}

/// Build a persistent (CP-path) instance.
pub fn persistent_instance(ip: &str, port: i32) -> Instance {
    let mut i = instance(ip, port);
    i.ephemeral = false;
    i
}

/// Register `count` instances `10.0.0.N:8080` and return the service.
pub fn with_instances(count: usize) -> NamingService {
    let svc = service();
    for n in 1..=count {
        svc.register_instance(NS, GROUP, "svc", instance(&format!("10.0.0.{n}"), 8080));
    }
    svc
}

/// Sorted `ip:port` list — keeps assertions independent of DashMap iteration order.
pub fn endpoints(instances: &[Instance]) -> Vec<String> {
    let mut v: Vec<String> = instances
        .iter()
        .map(|i| format!("{}:{}", i.ip, i.port))
        .collect();
    v.sort();
    v
}

/// Query helper with the namespace/group filled in from the fixtures.
pub fn query(svc: &NamingService, name: &str, cluster: &str, healthy_only: bool) -> Vec<Instance> {
    svc.get_instances(NS, GROUP, name, cluster, healthy_only)
}

/// Build a metadata map from `[("k", "v")]` pairs.
pub fn meta(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

/// Apply a partial update to one instance, mirroring
/// `PATCH /nacos/v2/ns/instance` (find → merge → re-register).
///
/// Returns `false` when nothing matches the `ip#port#cluster` triple, which is
/// what the HTTP layer turns into a 404.
pub fn patch(
    svc: &NamingService,
    name: &str,
    ip: &str,
    port: i32,
    cluster: &str,
    patch: InstancePatch,
) -> bool {
    let key = format!("{ip}#{port}#{cluster}");
    let Some(mut existing) = svc
        .get_instances_snapshot(NS, GROUP, name, "", false)
        .into_iter()
        .find(|i| i.key() == key)
        .map(|arc| (*arc).clone())
    else {
        return false;
    };
    apply_instance_patch(&mut existing, patch);
    svc.register_instance(NS, GROUP, name, existing)
}

/// Register through the AP (Distro) operation service, exactly as the gRPC
/// handler does via `ClientOperationServiceProxy`.
pub async fn register_ap(svc: &Arc<NamingService>, name: &str, instance: Instance) -> bool {
    use batata_naming::service::EphemeralClientOperationService;
    EphemeralClientOperationService::new(Arc::clone(svc), None)
        .register_instance(NS, GROUP, name, instance)
        .await
}

/// Register through the CP (Raft) operation service. With no Raft node wired
/// this exercises the documented dev-mode fallback.
pub async fn register_cp(svc: &Arc<NamingService>, name: &str, instance: Instance) -> bool {
    use batata_naming::service::PersistentClientOperationService;
    PersistentClientOperationService::new(Arc::clone(svc), None)
        .register_instance(NS, GROUP, name, instance)
        .await
}
