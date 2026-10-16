//! Bridge from Raft state machine apply events to `NamingService` DashMap.
//!
//! When `RocksStateMachine` applies a committed `PersistentInstance*` log
//! entry, it calls into this hook so the in-memory view reflects the new
//! state immediately — on both the leader and all followers. Without this,
//! persistent instances would only live in RocksDB `CF_INSTANCES` and reads
//! would miss them until a process restart.

use std::collections::HashSet;
use std::sync::Arc;

use batata_consistency::raft::NamingApplyHook;
use batata_consistency::raft::state_machine::StoredInstance;

use crate::model::Instance;
use crate::service::NamingService;
use crate::service::build_instance_key_parts;

/// Represents a `NamingApplyHookImpl`.
pub struct NamingApplyHookImpl {
    naming: Arc<NamingService>,
}

impl NamingApplyHookImpl {
    /// Creates a new instance.
    pub fn new(naming: Arc<NamingService>) -> Self {
        Self { naming }
    }
}

fn parse_metadata(raw: &str) -> std::collections::HashMap<String, String> {
    if raw.is_empty() {
        return Default::default();
    }
    serde_json::from_str(raw).unwrap_or_default()
}

/// Composite identity of a persistent instance: `ns@@group@@service@@instanceKey`.
///
/// Used as the set key when reconciling the in-memory view against the
/// authoritative contents of `CF_INSTANCES` after a snapshot install.
fn identity(
    namespace_id: &str,
    group_name: &str,
    service_name: &str,
    instance_key: &str,
) -> String {
    format!(
        "{}@@{}@@{}@@{}",
        namespace_id, group_name, service_name, instance_key
    )
}

impl NamingApplyHook for NamingApplyHookImpl {
    fn on_snapshot_loaded(&self, instances: Vec<StoredInstance>) {
        // A snapshot replaces `CF_INSTANCES` wholesale, so the authoritative
        // set is whatever is on disk right now -- not a delta. Register every
        // entry, then drop the persistent instances the snapshot does not
        // contain. Ephemeral instances are left alone: they arrive through
        // Distro heartbeats and are not part of the Raft state.
        let mut expected: HashSet<String> = HashSet::with_capacity(instances.len());

        for stored in instances {
            let (ns, group, svc) = (
                stored.namespace_id.clone(),
                stored.group_name.clone(),
                stored.service_name.clone(),
            );
            // Mirror `register_instance`'s normalization so the storage key
            // computed here matches the one the write path produces.
            let cluster_name = if stored.cluster_name.is_empty() {
                "DEFAULT".to_string()
            } else {
                stored.cluster_name
            };
            let instance_key =
                build_instance_key_parts(&stored.ip, stored.port as i32, &cluster_name);
            expected.insert(identity(&ns, &group, &svc, &instance_key));

            let instance = Instance {
                instance_id: stored.instance_id,
                ip: stored.ip,
                port: stored.port as i32,
                weight: stored.weight,
                healthy: stored.healthy,
                enabled: stored.enabled,
                ephemeral: false,
                cluster_name,
                service_name: svc.clone(),
                metadata: parse_metadata(&stored.metadata),
            };
            self.naming.register_instance(&ns, &group, &svc, instance);
        }

        let mut removed = 0usize;
        for (ns, group, svc, instance_key) in self.naming.persistent_instance_keys() {
            if expected.contains(&identity(&ns, &group, &svc, &instance_key)) {
                continue;
            }
            // instance_key format is "ip#port#cluster" (see
            // `build_instance_key_parts`); rebuild a minimal Instance so
            // `deregister_instance` reconstructs the same key.
            let parts: Vec<&str> = instance_key.split('#').collect();
            match parts.as_slice() {
                [ip, port, cluster] => {
                    let instance = Instance {
                        ip: ip.to_string(),
                        port: port.parse::<i32>().unwrap_or(0),
                        cluster_name: cluster.to_string(),
                        ..Default::default()
                    };
                    self.naming
                        .deregister_instance(&ns, &group, &svc, &instance);
                    removed += 1;
                }
                _ => tracing::warn!(
                    "on_snapshot_loaded: unexpected instance key format: {}",
                    instance_key
                ),
            }
        }

        tracing::info!(
            "Snapshot install reconciled naming view: {} instances, {} removed as stale",
            expected.len(),
            removed
        );
    }

    fn on_register(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
        ip: &str,
        port: u16,
        weight: f64,
        healthy: bool,
        enabled: bool,
        metadata: &str,
        cluster_name: &str,
    ) {
        let instance = Instance {
            instance_id: instance_id.to_string(),
            ip: ip.to_string(),
            port: port as i32,
            weight,
            healthy,
            enabled,
            ephemeral: false,
            cluster_name: cluster_name.to_string(),
            service_name: service_name.to_string(),
            metadata: parse_metadata(metadata),
        };
        self.naming
            .register_instance(namespace_id, group_name, service_name, instance);
    }

    fn on_deregister(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
    ) {
        // instance_id format used by `build_instance_key`: "ip#port#cluster".
        // Rebuild a minimal Instance so the key reconstructs identically.
        let parts: Vec<&str> = instance_id.split('#').collect();
        let (ip, port, cluster_name) = match parts.as_slice() {
            [ip, port, cluster] => (
                ip.to_string(),
                port.parse::<i32>().unwrap_or(0),
                cluster.to_string(),
            ),
            _ => {
                tracing::warn!(
                    "on_deregister: unexpected instance_id format: {}",
                    instance_id
                );
                return;
            }
        };
        let instance = Instance {
            ip,
            port,
            cluster_name,
            ..Default::default()
        };
        self.naming
            .deregister_instance(namespace_id, group_name, service_name, &instance);
    }

    fn on_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
        ip: Option<&str>,
        port: Option<u16>,
        weight: Option<f64>,
        healthy: Option<bool>,
        enabled: Option<bool>,
        metadata: Option<&str>,
    ) {
        // Load the current instance and merge only the Some() fields — a
        // partial update must not wipe fields the caller didn't specify.
        let Some(mut existing) =
            self.naming
                .get_instance_by_key(namespace_id, group_name, service_name, instance_id)
        else {
            tracing::warn!(
                "on_update: instance not found (ns={}, group={}, svc={}, id={}) — skipping",
                namespace_id,
                group_name,
                service_name,
                instance_id
            );
            return;
        };

        if let Some(ip) = ip {
            existing.ip = ip.to_string();
        }
        if let Some(port) = port {
            existing.port = port as i32;
        }
        if let Some(weight) = weight {
            existing.weight = weight;
        }
        if let Some(healthy) = healthy {
            existing.healthy = healthy;
        }
        if let Some(enabled) = enabled {
            existing.enabled = enabled;
        }
        if let Some(metadata_str) = metadata {
            // Metadata updates are additive — merge into the existing map
            // rather than replacing. Matches Nacos Java behavior.
            let incoming = parse_metadata(metadata_str);
            for (k, v) in incoming {
                existing.metadata.insert(k, v);
            }
        }
        // Persistent instances never flip to ephemeral via update.
        existing.ephemeral = false;

        self.naming
            .register_instance(namespace_id, group_name, service_name, existing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored_instance(instance_id: &str, ip: &str, port: u16) -> StoredInstance {
        StoredInstance {
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "svc-a".to_string(),
            instance_id: instance_id.to_string(),
            ip: ip.to_string(),
            port,
            weight: 1.0,
            healthy: true,
            enabled: true,
            metadata: "{}".to_string(),
            cluster_name: "DEFAULT".to_string(),
            registered_time: 0,
            modified_time: 0,
        }
    }

    fn instance(instance_id: &str, ip: &str, port: i32, ephemeral: bool) -> Instance {
        Instance {
            instance_id: instance_id.to_string(),
            ip: ip.to_string(),
            port,
            cluster_name: "DEFAULT".to_string(),
            service_name: "svc-a".to_string(),
            ephemeral,
            ..Default::default()
        }
    }

    #[test]
    fn snapshot_install_drops_instances_the_snapshot_omits() {
        let naming = Arc::new(NamingService::new());
        let hook = NamingApplyHookImpl::new(naming.clone());

        // Pre-existing state: a persistent instance the snapshot will not
        // contain, plus an ephemeral one that must survive untouched.
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "svc-a",
            instance("stale", "10.0.0.1", 8080, false),
        );
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "svc-a",
            instance("eph", "10.0.0.2", 9090, true),
        );

        hook.on_snapshot_loaded(vec![stored_instance("fresh", "10.0.0.3", 7070)]);

        // The stale persistent instance is gone, the snapshot's instance is in.
        let keys: Vec<String> = naming
            .persistent_instance_keys()
            .into_iter()
            .map(|(_, _, _, key)| key)
            .collect();
        assert_eq!(
            keys,
            vec!["10.0.0.3#7070#DEFAULT".to_string()],
            "a snapshot install must remove persistent instances it does not contain"
        );

        // Ephemeral instances come from Distro, not from Raft -- untouched.
        let remaining =
            naming.get_instances_snapshot("public", "DEFAULT_GROUP", "svc-a", "*", false);
        assert!(
            remaining.iter().any(|i| i.ip == "10.0.0.2"),
            "ephemeral instances must survive a snapshot install"
        );
    }

    #[test]
    fn snapshot_install_keeps_instances_the_snapshot_contains() {
        let naming = Arc::new(NamingService::new());
        let hook = NamingApplyHookImpl::new(naming.clone());

        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "svc-a",
            instance("keep", "10.0.0.1", 8080, false),
        );

        // Same instance re-supplied by the snapshot: idempotent, not removed.
        hook.on_snapshot_loaded(vec![stored_instance("keep", "10.0.0.1", 8080)]);

        let keys: Vec<String> = naming
            .persistent_instance_keys()
            .into_iter()
            .map(|(_, _, _, key)| key)
            .collect();
        assert_eq!(keys, vec!["10.0.0.1#8080#DEFAULT".to_string()]);
    }
}
