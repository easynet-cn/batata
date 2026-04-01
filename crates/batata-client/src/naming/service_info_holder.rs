//! Service info holder for caching service instance lists locally.
//!
//! Matches Nacos Java `ServiceInfoHolder` with:
//! - In-memory cache via DashMap
//! - Disk cache persistence via FailoverReactor
//! - Diff computation when service info changes
//! - Failover data retrieval when server is unavailable

use std::sync::Arc;

use batata_api::naming::model::Service;
use dashmap::DashMap;
use tracing::{debug, warn};

use super::failover::FailoverReactor;
use super::instances_diff::InstancesDiff;

/// Local cache of service instance information.
///
/// Updated when server pushes `NotifySubscriberRequest` or
/// when we receive `SubscribeServiceResponse`.
pub struct ServiceInfoHolder {
    /// key = "groupName@@serviceName"
    service_info_map: DashMap<String, Service>,
    /// Optional failover reactor for disk persistence
    failover_reactor: Option<Arc<FailoverReactor>>,
}

impl ServiceInfoHolder {
    pub fn new() -> Self {
        Self {
            service_info_map: DashMap::new(),
            failover_reactor: None,
        }
    }

    /// Create with a failover reactor for disk persistence.
    pub fn with_failover(failover_reactor: Arc<FailoverReactor>) -> Self {
        Self {
            service_info_map: DashMap::new(),
            failover_reactor: Some(failover_reactor),
        }
    }

    /// Process incoming service info: compute diff, update cache, persist to disk.
    ///
    /// Matches Nacos Java `ServiceInfoHolder.processServiceInfo()`.
    /// Returns the computed diff if there was a previous entry.
    pub fn process_service_info(&self, key: &str, new_service: Service) -> Option<InstancesDiff> {
        // Compute diff against existing cached data
        let diff = self
            .service_info_map
            .get(key)
            .map(|old_service| InstancesDiff::diff(&old_service.hosts, &new_service.hosts));

        // Update in-memory cache
        self.service_info_map
            .insert(key.to_string(), new_service.clone());

        // Persist to disk (non-blocking best-effort)
        if let Some(reactor) = &self.failover_reactor {
            if let Err(e) =
                reactor.save_failover(&new_service.group_name, &new_service.name, &new_service)
            {
                warn!("Failed to save failover for service {}: {}", key, e);
            }
        }

        debug!(
            "Processed service info: key={}, hosts={}, diff_changes={}",
            key,
            new_service.hosts.len(),
            diff.as_ref().map_or(0, |d| d.change_count())
        );

        diff
    }

    /// Update or insert service info in the cache (simple update without diff).
    pub fn update(&self, key: &str, service: Service) {
        self.service_info_map.insert(key.to_string(), service);
    }

    /// Get cached service info.
    pub fn get(&self, key: &str) -> Option<Service> {
        self.service_info_map.get(key).map(|e| e.clone())
    }

    /// Get service info from cache, falling back to failover data if not in cache.
    ///
    /// Matches Nacos Java `ServiceInfoHolder` failover behavior.
    pub fn get_or_failover(
        &self,
        key: &str,
        group_name: &str,
        service_name: &str,
    ) -> Option<Service> {
        // Try in-memory cache first
        if let Some(service) = self.get(key) {
            return Some(service);
        }

        // Try disk failover
        if let Some(reactor) = &self.failover_reactor {
            match reactor.load_failover(group_name, service_name) {
                Ok(Some(service)) => {
                    debug!("Loaded failover data for service: {}", key);
                    // Put it in memory cache too
                    self.service_info_map
                        .insert(key.to_string(), service.clone());
                    return Some(service);
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("Failed to load failover for service {}: {}", key, e);
                }
            }
        }

        None
    }

    /// Load all service info from disk cache on startup.
    ///
    /// Matches Nacos Java `ServiceInfoHolder` initialization with
    /// `NAMING_LOAD_CACHE_AT_START` property.
    pub fn load_from_disk_cache(&self) {
        // This is a best-effort operation
        // The failover reactor handles the actual file reading
        // In the future, we could scan the failover directory and pre-populate
        debug!("Disk cache loading is handled on-demand via get_or_failover");
    }

    /// Remove service info from the cache.
    pub fn remove(&self, key: &str) -> Option<Service> {
        self.service_info_map.remove(key).map(|(_, v)| v)
    }

    /// Get all cached service keys.
    pub fn keys(&self) -> Vec<String> {
        self.service_info_map
            .iter()
            .map(|e| e.key().clone())
            .collect()
    }

    /// Get the number of cached services.
    pub fn len(&self) -> usize {
        self.service_info_map.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.service_info_map.is_empty()
    }
}

impl Default for ServiceInfoHolder {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a service info cache key from group and service name.
pub fn build_service_key(group_name: &str, service_name: &str) -> String {
    format!("{}@@{}", group_name, service_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_api::naming::model::Instance;

    #[test]
    fn test_build_service_key() {
        assert_eq!(
            build_service_key("DEFAULT_GROUP", "my-service"),
            "DEFAULT_GROUP@@my-service"
        );
    }

    #[test]
    fn test_service_info_holder() {
        let holder = ServiceInfoHolder::new();
        let key = "DEFAULT_GROUP@@test-service";

        // Initially empty
        assert!(holder.get(key).is_none());

        // Insert
        let mut service = Service::new("test-service".to_string(), "DEFAULT_GROUP".to_string());
        service
            .hosts
            .push(Instance::new("127.0.0.1".to_string(), 8080));
        holder.update(key, service);

        // Get
        let cached = holder.get(key).unwrap();
        assert_eq!(cached.name, "test-service");
        assert_eq!(cached.hosts.len(), 1);

        // Keys
        assert_eq!(holder.keys(), vec![key.to_string()]);

        // Remove
        let removed = holder.remove(key).unwrap();
        assert_eq!(removed.name, "test-service");
        assert!(holder.get(key).is_none());
    }

    #[test]
    fn test_service_info_holder_default() {
        let holder = ServiceInfoHolder::default();
        assert!(holder.keys().is_empty());
    }

    #[test]
    fn test_service_info_holder_update_overwrites() {
        let holder = ServiceInfoHolder::new();
        let key = "DEFAULT_GROUP@@test-service";

        let mut service1 = Service::new("test-service".to_string(), "DEFAULT_GROUP".to_string());
        service1
            .hosts
            .push(Instance::new("127.0.0.1".to_string(), 8080));
        holder.update(key, service1);

        let mut service2 = Service::new("test-service".to_string(), "DEFAULT_GROUP".to_string());
        service2
            .hosts
            .push(Instance::new("10.0.0.1".to_string(), 9090));
        service2
            .hosts
            .push(Instance::new("10.0.0.2".to_string(), 9091));
        holder.update(key, service2);

        let cached = holder.get(key).unwrap();
        assert_eq!(cached.hosts.len(), 2);
        assert_eq!(cached.hosts[0].ip, "10.0.0.1");
    }

    #[test]
    fn test_service_info_holder_multiple_services() {
        let holder = ServiceInfoHolder::new();

        holder.update(
            "group1@@svc1",
            Service::new("svc1".to_string(), "group1".to_string()),
        );
        holder.update(
            "group1@@svc2",
            Service::new("svc2".to_string(), "group1".to_string()),
        );
        holder.update(
            "group2@@svc1",
            Service::new("svc1".to_string(), "group2".to_string()),
        );

        assert_eq!(holder.keys().len(), 3);
        assert!(holder.get("group1@@svc1").is_some());
        assert!(holder.get("group1@@svc2").is_some());
        assert!(holder.get("group2@@svc1").is_some());
    }

    #[test]
    fn test_service_info_holder_remove_nonexistent() {
        let holder = ServiceInfoHolder::new();
        assert!(holder.remove("nonexistent").is_none());
    }

    #[test]
    fn test_build_service_key_special_chars() {
        assert_eq!(
            build_service_key("my-group", "my.service.v2"),
            "my-group@@my.service.v2"
        );
    }

    #[test]
    fn test_process_service_info_first_time() {
        let holder = ServiceInfoHolder::new();
        let key = "DEFAULT_GROUP@@test-service";

        let mut service = Service::new("test-service".to_string(), "DEFAULT_GROUP".to_string());
        service
            .hosts
            .push(Instance::new("10.0.0.1".to_string(), 8080));

        // First time — no diff (no previous entry)
        let diff = holder.process_service_info(key, service);
        assert!(diff.is_none());

        // Verify cached
        let cached = holder.get(key).unwrap();
        assert_eq!(cached.hosts.len(), 1);
    }

    #[test]
    fn test_process_service_info_with_diff() {
        let holder = ServiceInfoHolder::new();
        let key = "DEFAULT_GROUP@@test-service";

        // Initial
        let mut service1 = Service::new("test-service".to_string(), "DEFAULT_GROUP".to_string());
        service1
            .hosts
            .push(Instance::new("10.0.0.1".to_string(), 8080));
        holder.update(key, service1);

        // Update with additional instance
        let mut service2 = Service::new("test-service".to_string(), "DEFAULT_GROUP".to_string());
        service2
            .hosts
            .push(Instance::new("10.0.0.1".to_string(), 8080));
        service2
            .hosts
            .push(Instance::new("10.0.0.2".to_string(), 8081));

        let diff = holder.process_service_info(key, service2);
        assert!(diff.is_some());
        let diff = diff.unwrap();
        assert_eq!(diff.added_instances.len(), 1);
        assert_eq!(diff.added_instances[0].ip, "10.0.0.2");
    }

    #[test]
    fn test_get_or_failover_cache_hit() {
        let holder = ServiceInfoHolder::new();
        let key = "DEFAULT_GROUP@@test-service";

        let service = Service::new("test-service".to_string(), "DEFAULT_GROUP".to_string());
        holder.update(key, service);

        let result = holder.get_or_failover(key, "DEFAULT_GROUP", "test-service");
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "test-service");
    }

    #[test]
    fn test_get_or_failover_no_data() {
        let holder = ServiceInfoHolder::new();
        let result = holder.get_or_failover("DEFAULT_GROUP@@none", "DEFAULT_GROUP", "none");
        assert!(result.is_none());
    }

    #[test]
    fn test_len_and_is_empty() {
        let holder = ServiceInfoHolder::new();
        assert!(holder.is_empty());
        assert_eq!(holder.len(), 0);

        holder.update("k1", Service::new("s1".into(), "g1".into()));
        assert!(!holder.is_empty());
        assert_eq!(holder.len(), 1);
    }
}
