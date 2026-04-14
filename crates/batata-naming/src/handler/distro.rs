//! Naming-specific distro protocol handler for ephemeral instance synchronization
//!
//! This handler bridges the naming service with the core distro protocol,
//! allowing ephemeral service instances to be synced across cluster nodes.

use std::sync::Arc;

use batata_core::service::distro::{DistroData, DistroDataHandler, DistroDataType};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::service::NamingService;

// Re-export DistroProtocol for convenience
pub use batata_core::service::distro::DistroProtocol;

/// Instance data serialized for distro sync
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroInstanceData {
    pub namespace: String,
    pub group_name: String,
    pub service_name: String,
    pub instances: Vec<DistroInstance>,
}

/// Single instance for distro sync
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroInstance {
    pub instance_id: String,
    pub ip: String,
    pub port: i32,
    pub weight: f64,
    pub healthy: bool,
    pub enabled: bool,
    pub ephemeral: bool,
    pub cluster_name: String,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Default implementation for naming instance data
pub struct NamingInstanceDistroHandler {
    local_address: String,
    naming_service: Arc<NamingService>,
}

impl NamingInstanceDistroHandler {
    pub fn new(local_address: String, naming_service: Arc<NamingService>) -> Self {
        Self {
            local_address,
            naming_service,
        }
    }

    /// Parse service key to (namespace, group_name, service_name)
    fn parse_service_key(key: &str) -> Option<(String, String, String)> {
        let parts: Vec<&str> = key.split("@@").collect();
        if parts.len() == 3 {
            Some((
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
            ))
        } else {
            None
        }
    }
}

#[tonic::async_trait]
impl DistroDataHandler for NamingInstanceDistroHandler {
    fn data_type(&self) -> DistroDataType {
        DistroDataType::NamingInstance
    }

    async fn get_all_keys(&self) -> Vec<String> {
        // Get all service keys from naming service
        self.naming_service.get_all_service_keys()
    }

    async fn get_data(&self, key: &str) -> Option<DistroData> {
        // Parse the service key
        let (namespace, group_name, service_name) = Self::parse_service_key(key)?;

        // Snapshot instances (zero-copy Arc clones). We filter ephemerals
        // before any deep-clone, so persistent instances cost nothing here.
        let snapshot = self.naming_service.get_instances_snapshot(
            &namespace,
            &group_name,
            &service_name,
            "",    // all clusters
            false, // include unhealthy
        );

        // Only sync ephemeral instances. Clone only the survivors.
        let ephemeral_instances: Vec<DistroInstance> = snapshot
            .iter()
            .filter(|inst| inst.ephemeral)
            .map(|inst| DistroInstance {
                instance_id: inst.instance_id.clone(),
                ip: inst.ip.clone(),
                port: inst.port,
                weight: inst.weight,
                healthy: inst.healthy,
                enabled: inst.enabled,
                ephemeral: inst.ephemeral,
                cluster_name: inst.cluster_name.clone(),
                metadata: inst.metadata.clone(),
            })
            .collect();

        // Always return data even when instances list is empty.
        // An empty list signals to remote nodes that all ephemeral instances
        // have been deregistered and should be removed on their side.
        let data = DistroInstanceData {
            namespace,
            group_name,
            service_name,
            instances: ephemeral_instances,
        };

        let content = serde_json::to_vec(&data).ok()?;

        // Use service revision as version instead of Utc::now().
        // This gives a stable version that only changes when data actually changes,
        // enabling accurate verify comparisons between cluster nodes.
        let revision = self.naming_service.get_service_revision(key);

        Some(DistroData {
            data_type: DistroDataType::NamingInstance,
            key: key.to_string(),
            content,
            version: revision,
            source: self.local_address.clone(),
        })
    }

    async fn process_sync_data(&self, data: DistroData) -> Result<(), String> {
        debug!("Processing naming instance sync data: key={}", data.key);

        // Parse the content
        let instance_data: DistroInstanceData =
            serde_json::from_slice(&data.content).map_err(|e| format!("Failed to parse: {}", e))?;

        let instances: Vec<crate::Instance> = instance_data
            .instances
            .into_iter()
            .map(|inst| crate::Instance {
                instance_id: inst.instance_id,
                ip: inst.ip,
                port: inst.port,
                weight: inst.weight,
                healthy: inst.healthy,
                enabled: inst.enabled,
                ephemeral: inst.ephemeral,
                cluster_name: inst.cluster_name,
                service_name: instance_data.service_name.clone(),
                metadata: inst.metadata,
            })
            .collect();

        // Merge-and-reconcile: use the instance keys from the sync data as the
        // authoritative set from the source node. Add/update instances present in
        // the sync, remove instances that are in our local store AND match keys
        // in the sync set's "namespace" (i.e., came from the same source).
        // Since distro sync is per-service, and each sync contains ALL instances
        // the source node knows about for this service, we can safely replace.
        // But we must NOT remove instances registered locally by this node's
        // own clients -- only merge from remote.
        let count = instances.len();
        self.naming_service.merge_remote_instances(
            &instance_data.namespace,
            &instance_data.group_name,
            &instance_data.service_name,
            instances,
        );

        info!(
            "Synced {} instances for service {}//{}/{} (merge)",
            count, instance_data.namespace, instance_data.group_name, instance_data.service_name
        );
        Ok(())
    }

    async fn process_verify_data(&self, data: &DistroData) -> Result<bool, String> {
        // Verify data by checking if our local version matches
        if let Some(local_data) = self.get_data(&data.key).await {
            // Compare versions
            Ok(local_data.version >= data.version)
        } else {
            // We don't have this data, need sync
            Ok(false)
        }
    }

    async fn get_snapshot(&self) -> Vec<DistroData> {
        // Get all service keys and their data
        let keys = self.get_all_keys().await;
        let mut snapshot = Vec::new();

        for key in keys {
            if let Some(data) = self.get_data(&key).await {
                snapshot.push(data);
            }
        }

        snapshot
    }

    async fn remove_data(&self, key: &str) -> Result<(), String> {
        // Called by DistroProtocol::cleanup_non_responsible_keys when the
        // local node is no longer responsible for this service_key after a
        // cluster membership change. We only drop remotely-synced ephemeral
        // instances — instances registered by local gRPC clients must be
        // preserved (they are tracked by connection_instances and will be
        // cleaned up when the client disconnects). Persistent instances
        // live in Raft and must not be touched by the Distro layer.
        //
        // Use merge_remote_instances with an empty list: it only removes
        // instances marked with `_distro_remote=true`, leaving locally-
        // registered instances intact. This matches Nacos 3.x behavior
        // where gRPC-registered instances are accepted at any node and
        // never deleted by distro responsibility changes.
        let Some((namespace, group_name, service_name)) = Self::parse_service_key(key) else {
            return Err(format!("Invalid service key: {}", key));
        };
        self.naming_service.merge_remote_instances(
            &namespace,
            &group_name,
            &service_name,
            Vec::new(),
        );
        debug!(
            "Removed remote ephemeral instances for non-responsible key: {}",
            key
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_core::service::distro::{DistroData, DistroDataType};

    fn build_sync_data(namespace: &str, group: &str, service: &str, ip: &str) -> DistroData {
        let content = DistroInstanceData {
            namespace: namespace.to_string(),
            group_name: group.to_string(),
            service_name: service.to_string(),
            instances: vec![DistroInstance {
                instance_id: format!("{}-{}", service, ip),
                ip: ip.to_string(),
                port: 8080,
                weight: 1.0,
                healthy: true,
                enabled: true,
                ephemeral: true,
                cluster_name: "DEFAULT".to_string(),
                metadata: std::collections::HashMap::new(),
            }],
        };
        DistroData {
            data_type: DistroDataType::NamingInstance,
            key: format!("{}@@{}@@{}", namespace, group, service),
            content: serde_json::to_vec(&content).unwrap(),
            version: 1,
            source: "10.0.0.2:8848".to_string(),
        }
    }

    /// Gap A — after dispossession via `remove_data`, the service's
    /// remote-synced ephemeral instances must be gone from the naming store.
    #[tokio::test]
    async fn test_dispossession_via_remove_data() {
        let naming = Arc::new(NamingService::new());
        let handler = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), naming.clone());

        // Simulate: we received a sync for svc-foo from another node.
        let data = build_sync_data("public", "DEFAULT_GROUP", "svc-foo", "192.168.1.1");
        handler.process_sync_data(data).await.unwrap();

        let key = "public@@DEFAULT_GROUP@@svc-foo";
        let before = naming.get_instances_snapshot("public", "DEFAULT_GROUP", "svc-foo", "", false);
        assert_eq!(before.len(), 1, "sync must have populated one instance");
        assert_eq!(before[0].ip, "192.168.1.1");

        // Peer verify-failure / membership change: dispossess.
        handler.remove_data(key).await.unwrap();

        let after = naming.get_instances_snapshot("public", "DEFAULT_GROUP", "svc-foo", "", false);
        assert!(
            after.is_empty(),
            "dispossession must drop remote ephemeral instances; found {:?}",
            after.iter().map(|i| i.ip.clone()).collect::<Vec<_>>()
        );
    }

    /// Gap B — a 50-item batch (one item per service) must populate all 50
    /// services when applied through the handler's `process_sync_data`.
    #[tokio::test]
    async fn test_batch_of_50_items_all_applied() {
        let naming = Arc::new(NamingService::new());
        let handler = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), naming.clone());

        const N: usize = 50;
        for i in 0..N {
            let service = format!("svc-{:02}", i);
            let ip = format!("192.168.1.{}", i + 1);
            let data = build_sync_data("public", "DEFAULT_GROUP", &service, &ip);
            handler
                .process_sync_data(data)
                .await
                .expect("each item must apply");
        }

        let keys = handler.get_all_keys().await;
        assert_eq!(keys.len(), N, "all {} services must be present", N);

        for i in 0..N {
            let service = format!("svc-{:02}", i);
            let instances =
                naming.get_instances_snapshot("public", "DEFAULT_GROUP", &service, "", false);
            assert_eq!(
                instances.len(),
                1,
                "service {} must have one instance",
                service
            );
            assert_eq!(instances[0].ip, format!("192.168.1.{}", i + 1));
            assert!(instances[0].ephemeral);
            assert_eq!(instances[0].cluster_name, "DEFAULT");
        }
    }

    /// Verify-failure flow end-to-end on the naming layer: dispossess drops
    /// previously-synced data, then a fresh sync repopulates cleanly.
    #[tokio::test]
    async fn test_dispossess_then_resync_is_clean() {
        let naming = Arc::new(NamingService::new());
        let handler = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), naming.clone());

        let d1 = build_sync_data("public", "DEFAULT_GROUP", "svc-x", "10.1.1.1");
        handler.process_sync_data(d1).await.unwrap();

        let key = "public@@DEFAULT_GROUP@@svc-x";
        handler.remove_data(key).await.unwrap();
        assert!(
            naming
                .get_instances_snapshot("public", "DEFAULT_GROUP", "svc-x", "", false)
                .is_empty()
        );

        let d2 = build_sync_data("public", "DEFAULT_GROUP", "svc-x", "10.1.1.2");
        handler.process_sync_data(d2).await.unwrap();
        let after = naming.get_instances_snapshot("public", "DEFAULT_GROUP", "svc-x", "", false);
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].ip, "10.1.1.2", "re-sync must populate the new IP");
    }
}
