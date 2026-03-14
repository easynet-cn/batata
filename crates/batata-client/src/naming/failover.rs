//! Service discovery failure recovery and failover mechanism
//!
//! Similar to Nacos FailoverReactor, provides disk-based
//! failover for service discovery.

use crate::error::Result;
use crate::naming::service_info_holder::build_service_key;
use batata_api::naming::model::Service;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Failover reactor for service discovery
pub struct FailoverReactor {
    base_path: PathBuf,
    #[allow(dead_code)]
    env_name: String,
    cached_services: Arc<RwLock<std::collections::HashMap<String, Service>>>,
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

    fn get_failover_path(_env_name: &str) -> PathBuf {
        if let Ok(path) = std::env::var("BATATA_SNAPSHOT_PATH") {
            return PathBuf::from(path).join("naming");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".batata").join("naming");
        }
        PathBuf::from(".batata").join("naming")
    }

    /// Save service info to failover file
    pub fn save_failover(
        &self,
        group_name: &str,
        service_name: &str,
        service_info: &Service,
    ) -> Result<()> {
        let key = build_service_key(group_name, service_name);
        let file_path = self.base_path.join(&key);

        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| crate::error::ClientError::Other(e.into()))?;
        }

        let json = serde_json::to_string_pretty(service_info)
            .map_err(|e| crate::error::ClientError::Other(e.into()))?;

        std::fs::write(&file_path, json).map_err(|e| crate::error::ClientError::Other(e.into()))?;

        tracing::debug!("Saved failover for service: {}", key);

        Ok(())
    }

    /// Load service info from failover file
    pub fn load_failover(&self, group_name: &str, service_name: &str) -> Result<Option<Service>> {
        let key = build_service_key(group_name, service_name);
        let file_path = self.base_path.join(&key);

        if !file_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&file_path)
            .map_err(|e| crate::error::ClientError::Other(e.into()))?;

        let service_info: Service = serde_json::from_str(&content)
            .map_err(|e| crate::error::ClientError::Other(e.into()))?;

        Ok(Some(service_info))
    }

    /// Get cached service info
    pub async fn get_cached_service(
        &self,
        group_name: &str,
        service_name: &str,
    ) -> Option<Service> {
        let key = build_service_key(group_name, service_name);
        let cached = self.cached_services.read().await;
        cached.get(&key).cloned()
    }

    /// Update cached service info
    pub async fn update_cache(&self, group_name: &str, service_name: &str, service_info: Service) {
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

    /// Get the base path (for testing)
    #[cfg(test)]
    fn base_path(&self) -> &PathBuf {
        &self.base_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_api::naming::model::Instance;
    use tempfile::TempDir;

    fn create_test_reactor(temp_dir: &TempDir) -> FailoverReactor {
        FailoverReactor {
            base_path: temp_dir.path().join("naming"),
            env_name: "test".to_string(),
            cached_services: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    fn create_test_service() -> Service {
        Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![Instance {
                ip: "10.0.0.1".to_string(),
                port: 8080,
                healthy: true,
                weight: 1.0,
                enabled: true,
                ephemeral: true,
                cluster_name: "DEFAULT".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn test_save_and_load_failover() {
        let temp_dir = TempDir::new().unwrap();
        let reactor = create_test_reactor(&temp_dir);
        let service = create_test_service();

        // Save failover
        reactor
            .save_failover("DEFAULT_GROUP", "test-service", &service)
            .unwrap();

        // Load failover
        let loaded = reactor
            .load_failover("DEFAULT_GROUP", "test-service")
            .unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.name, "test-service");
        assert_eq!(loaded.hosts.len(), 1);
        assert_eq!(loaded.hosts[0].ip, "10.0.0.1");
        assert_eq!(loaded.hosts[0].port, 8080);
    }

    #[test]
    fn test_load_failover_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let reactor = create_test_reactor(&temp_dir);

        let loaded = reactor
            .load_failover("DEFAULT_GROUP", "non-existent")
            .unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_save_failover_overwrites() {
        let temp_dir = TempDir::new().unwrap();
        let reactor = create_test_reactor(&temp_dir);

        let mut service1 = create_test_service();
        service1.hosts[0].ip = "10.0.0.1".to_string();

        let mut service2 = create_test_service();
        service2.hosts[0].ip = "10.0.0.2".to_string();

        reactor
            .save_failover("DEFAULT_GROUP", "test-service", &service1)
            .unwrap();
        reactor
            .save_failover("DEFAULT_GROUP", "test-service", &service2)
            .unwrap();

        let loaded = reactor
            .load_failover("DEFAULT_GROUP", "test-service")
            .unwrap()
            .unwrap();
        assert_eq!(loaded.hosts[0].ip, "10.0.0.2");
    }

    #[tokio::test]
    async fn test_update_and_get_cached_service() {
        let temp_dir = TempDir::new().unwrap();
        let reactor = create_test_reactor(&temp_dir);
        let service = create_test_service();

        // Initially empty
        assert!(
            reactor
                .get_cached_service("DEFAULT_GROUP", "test-service")
                .await
                .is_none()
        );

        // Update cache
        reactor
            .update_cache("DEFAULT_GROUP", "test-service", service.clone())
            .await;

        // Get cached
        let cached = reactor
            .get_cached_service("DEFAULT_GROUP", "test-service")
            .await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().name, "test-service");
    }

    #[tokio::test]
    async fn test_remove_cache() {
        let temp_dir = TempDir::new().unwrap();
        let reactor = create_test_reactor(&temp_dir);
        let service = create_test_service();

        reactor
            .update_cache("DEFAULT_GROUP", "test-service", service)
            .await;
        assert!(
            reactor
                .get_cached_service("DEFAULT_GROUP", "test-service")
                .await
                .is_some()
        );

        reactor.remove_cache("DEFAULT_GROUP", "test-service").await;
        assert!(
            reactor
                .get_cached_service("DEFAULT_GROUP", "test-service")
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_get_cached_service_wrong_key() {
        let temp_dir = TempDir::new().unwrap();
        let reactor = create_test_reactor(&temp_dir);
        let service = create_test_service();

        reactor
            .update_cache("DEFAULT_GROUP", "test-service", service)
            .await;

        // Different service name
        assert!(
            reactor
                .get_cached_service("DEFAULT_GROUP", "other-service")
                .await
                .is_none()
        );

        // Different group
        assert!(
            reactor
                .get_cached_service("OTHER_GROUP", "test-service")
                .await
                .is_none()
        );
    }

    #[test]
    fn test_clean_all_failover() {
        let temp_dir = TempDir::new().unwrap();
        let reactor = create_test_reactor(&temp_dir);
        let service = create_test_service();

        // Save some failover data
        reactor
            .save_failover("DEFAULT_GROUP", "svc1", &service)
            .unwrap();
        reactor
            .save_failover("DEFAULT_GROUP", "svc2", &service)
            .unwrap();

        // Verify files exist
        assert!(reactor.base_path().exists());

        // Clean all
        reactor.clean_all_failover().unwrap();

        // Verify cleaned
        assert!(!reactor.base_path().exists());
    }

    #[test]
    fn test_clean_all_failover_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let reactor = create_test_reactor(&temp_dir);

        // Should not error even if directory doesn't exist
        reactor.clean_all_failover().unwrap();
    }

    #[test]
    fn test_save_failover_with_multiple_instances() {
        let temp_dir = TempDir::new().unwrap();
        let reactor = create_test_reactor(&temp_dir);

        let mut service = create_test_service();
        service.hosts.push(Instance {
            ip: "10.0.0.2".to_string(),
            port: 8081,
            healthy: false,
            weight: 2.0,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            ..Default::default()
        });

        reactor
            .save_failover("DEFAULT_GROUP", "test-service", &service)
            .unwrap();

        let loaded = reactor
            .load_failover("DEFAULT_GROUP", "test-service")
            .unwrap()
            .unwrap();
        assert_eq!(loaded.hosts.len(), 2);
        assert_eq!(loaded.hosts[1].ip, "10.0.0.2");
        assert!(!loaded.hosts[1].healthy);
        assert_eq!(loaded.hosts[1].weight, 2.0);
    }

    #[test]
    fn test_failover_path_default() {
        let reactor = FailoverReactor::new("test-env".to_string());
        let path_str = reactor.base_path().to_str().unwrap();
        assert!(path_str.contains("naming"));
    }
}
