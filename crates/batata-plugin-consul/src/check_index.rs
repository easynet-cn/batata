// Consul-specific health check index
//
// Maintains the mapping between Consul service IDs and the internal
// instance/service keys used by InstanceCheckRegistry.

use dashmap::DashMap;

/// Consul service ID to instance key mapping for O(1) check lookup
pub struct ConsulCheckIndex {
    /// consul_service_id → (service_key, instance_key)
    service_index: DashMap<String, (String, String)>,
}

impl ConsulCheckIndex {
    pub fn new() -> Self {
        Self {
            service_index: DashMap::new(),
        }
    }

    /// Register a Consul service ID → (service_key, instance_key) mapping
    pub fn register(&self, consul_svc_id: &str, svc_key: &str, inst_key: &str) {
        self.service_index.insert(
            consul_svc_id.to_string(),
            (svc_key.to_string(), inst_key.to_string()),
        );
    }

    /// Look up a Consul service ID to find the (service_key, instance_key)
    pub fn lookup(&self, consul_svc_id: &str) -> Option<(String, String)> {
        self.service_index
            .get(consul_svc_id)
            .map(|entry| entry.value().clone())
    }

    /// Remove a Consul service ID mapping
    pub fn remove(&self, consul_svc_id: &str) {
        self.service_index.remove(consul_svc_id);
    }
}

impl Default for ConsulCheckIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_lookup_remove() {
        let index = ConsulCheckIndex::new();

        index.register(
            "my-svc-1",
            "public#DEFAULT_GROUP#svc",
            "public#DEFAULT_GROUP#svc#1.2.3.4#80#DEFAULT",
        );

        let result = index.lookup("my-svc-1");
        assert!(result.is_some());
        let (svc_key, inst_key) = result.unwrap();
        assert_eq!(svc_key, "public#DEFAULT_GROUP#svc");
        assert_eq!(inst_key, "public#DEFAULT_GROUP#svc#1.2.3.4#80#DEFAULT");

        index.remove("my-svc-1");
        assert!(index.lookup("my-svc-1").is_none());
    }

    #[test]
    fn test_lookup_nonexistent() {
        let index = ConsulCheckIndex::new();
        assert!(index.lookup("nonexistent").is_none());
    }
}
