//! Service discovery failure recovery and failover mechanism
//!
//! Similar to Nacos FailoverReactor, provides disk-based
//! failover for service discovery.

use crate::error::Result;
use crate::naming::service_info_holder::build_service_key;
use batata_api::naming::model::ServiceInfo;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Failover reactor for service discovery
pub struct FailoverReactor {
    base_path: PathBuf,
    env_name: String,
    cached_services: Arc<RwLock<std::collections::HashMap<String, ServiceInfo>>>,
}

impl FailoverReactor {
    pub fn new(env_name: String) -> Self {
        let base_path = Self::get_failover_path(env_name.as_str());
        Self {
            base_path,
            env_name,
            cached_services: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    fn get_failover_path(env_name: &str) -> PathBuf {
        if let Ok(path) = std::env::var("BATATA_SNAPSHOT_PATH") {
            return PathBuf::from(path).join("naming");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".batata").join("naming");
        }
        PathBuf::from(".batata").join("naming")
    }

    /// Save service info to failover file
    pub fn save_failover(&self, group_name: &str, service_name: &str, service_info: &ServiceInfo) -> Result<()> {
        let key = build_service_key(group_name, service_name);
        let file_path = self.base_path.join(&key);

        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| crate::error::ClientError::Other(e.into()))?;
        }

        let json = serde_json::to_string_pretty(service_info)
            .map_err(|e| crate::error::ClientError::Other(e.into()))?;

        std::fs::write(&file_path, json)
            .map_err(|e| crate::error::ClientError::Other(e.into()))?;

        tracing::debug!("Saved failover for service: {}", key);

        Ok(())
    }

    /// Load service info from failover file
    pub fn load_failover(&self, group_name: &str, service_name: &str) -> Result<Option<ServiceInfo>> {
        let key = build_service_key(group_name, service_name);
        let file_path = self.base_path.join(&key);

        if !file_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&file_path)
            .map_err(|e| crate::error::ClientError::Other(e.into()))?;

        let service_info: ServiceInfo = serde_json::from_str(&content)
            .map_err(|e| crate::error::ClientError::Other(e.into()))?;

        Ok(Some(service_info))
    }

    /// Get cached service info
    pub async fn get_cached_service(&self, group_name: &str, service_name: &str) -> Option<ServiceInfo> {
        let key = build_service_key(group_name, service_name);
        let cached = self.cached_services.read().await;
        cached.get(&key).cloned()
    }

    /// Update cached service info
    pub async fn update_cache(&self, group_name: &str, service_name: &str, service_info: ServiceInfo) {
        let key = build_service_key(group_name, service_name);
        let mut cached = self.cached_services.write().await;
        cached.insert(key, service_info);
    }

    /// Remove cached service info
    pub async fn remove_cache(&self, group_name: &str, service_name: &str) {
        let key = build_service_key(group_name, service_name);
        let mut cached = self.cached_services.write().await;
        cached.remove(&key);
    }

    /// Clean all failover files
    pub fn clean_all_failover(&self) -> Result<()> {
        if self.base_path.exists() {
            std::fs::remove_dir_all(&self.base_path)
                .map_err(|e| crate::error::ClientError::Other(e.into()))?;
        }
        Ok(())
    }
}
