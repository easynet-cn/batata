//! Bridge from Raft state machine apply events to `NamingService` DashMap.
//!
//! When `RocksStateMachine` applies a committed `PersistentInstance*` log
//! entry, it calls into this hook so the in-memory view reflects the new
//! state immediately — on both the leader and all followers. Without this,
//! persistent instances would only live in RocksDB `CF_INSTANCES` and reads
//! would miss them until a process restart.

use std::sync::Arc;

use batata_consistency::raft::NamingApplyHook;

use crate::model::Instance;
use crate::service::NamingService;

pub struct NamingApplyHookImpl {
    naming: Arc<NamingService>,
}

impl NamingApplyHookImpl {
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

impl NamingApplyHook for NamingApplyHookImpl {
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
        let Some(mut existing) = self.naming.get_instance_by_key(
            namespace_id,
            group_name,
            service_name,
            instance_id,
        ) else {
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
