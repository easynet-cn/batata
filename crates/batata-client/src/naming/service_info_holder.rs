//! Service info holder for caching service instance lists locally

use batata_api::naming::model::Service;
use dashmap::DashMap;

/// Local cache of service instance information.
///
/// Updated when server pushes `NotifySubscriberRequest` or
/// when we receive `SubscribeServiceResponse`.
pub struct ServiceInfoHolder {
    /// key = "groupName@@serviceName"
    service_info_map: DashMap<String, Service>,
}

impl ServiceInfoHolder {
    pub fn new() -> Self {
        Self {
            service_info_map: DashMap::new(),
        }
    }

    /// Update or insert service info in the cache.
    pub fn update(&self, key: &str, service: Service) {
        self.service_info_map.insert(key.to_string(), service);
    }

    /// Get cached service info.
    pub fn get(&self, key: &str) -> Option<Service> {
        self.service_info_map.get(key).map(|e| e.clone())
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
}
