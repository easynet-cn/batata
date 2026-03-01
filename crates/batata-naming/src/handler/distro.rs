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

        // Get instances from naming service
        let instances = self.naming_service.get_instances(
            &namespace,
            &group_name,
            &service_name,
            "",    // all clusters
            false, // include unhealthy
        );

        // Only sync ephemeral instances
        let ephemeral_instances: Vec<DistroInstance> = instances
            .into_iter()
            .filter(|inst| inst.ephemeral)
            .map(|inst| DistroInstance {
                instance_id: inst.instance_id,
                ip: inst.ip,
                port: inst.port,
                weight: inst.weight,
                healthy: inst.healthy,
                enabled: inst.enabled,
                ephemeral: inst.ephemeral,
                cluster_name: inst.cluster_name,
                metadata: inst.metadata,
            })
            .collect();

        if ephemeral_instances.is_empty() {
            return None;
        }

        let data = DistroInstanceData {
            namespace,
            group_name,
            service_name,
            instances: ephemeral_instances,
        };

        let content = serde_json::to_vec(&data).ok()?;

        Some(DistroData::new(
            DistroDataType::NamingInstance,
            key.to_string(),
            content,
            self.local_address.clone(),
        ))
    }

    async fn process_sync_data(&self, data: DistroData) -> Result<(), String> {
        debug!("Processing naming instance sync data: key={}", data.key);

        // Parse the content
        let instance_data: DistroInstanceData =
            serde_json::from_slice(&data.content).map_err(|e| format!("Failed to parse: {}", e))?;

        // Register all instances from the sync data
        for inst in instance_data.instances {
            let instance = crate::Instance {
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
            };

            self.naming_service.register_instance(
                &instance_data.namespace,
                &instance_data.group_name,
                &instance_data.service_name,
                instance,
            );
        }

        info!(
            "Synced instances for service {}//{}/{}",
            instance_data.namespace, instance_data.group_name, instance_data.service_name
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
}
