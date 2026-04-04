//! Subscriber, publisher, and connection instance tracking

use std::collections::HashSet;

use super::{NacosNamingServiceImpl, build_service_key};

impl NacosNamingServiceImpl {
    // ========================================================================
    // Subscription
    // ========================================================================

    /// Subscribe to a service
    pub fn subscribe(
        &self,
        connection_id: &str,
        namespace: &str,
        group: &str,
        service: &str,
        _clusters: &str,
    ) {
        let service_key = build_service_key(namespace, group, service);

        self.subscribers
            .entry(connection_id.to_string())
            .or_default()
            .insert(service_key.clone());

        self.subscriber_index
            .entry(service_key)
            .or_default()
            .insert(connection_id.to_string());
    }

    /// Unsubscribe from a service
    pub fn unsubscribe(
        &self,
        connection_id: &str,
        namespace: &str,
        group: &str,
        service: &str,
        _clusters: &str,
    ) {
        let service_key = build_service_key(namespace, group, service);

        if let Some(mut subs) = self.subscribers.get_mut(connection_id) {
            subs.remove(&service_key);
        }

        if let Some(mut connections) = self.subscriber_index.get_mut(&service_key) {
            connections.remove(connection_id);
        }
    }

    /// Get subscribers for a service (O(1) via reverse index)
    pub fn get_subscribers(&self, namespace: &str, group: &str, service: &str) -> Vec<String> {
        let service_key = build_service_key(namespace, group, service);

        self.subscriber_index
            .get(&service_key)
            .map(|entry| entry.value().iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clean up all subscriptions for a connection
    pub fn remove_subscriber(&self, connection_id: &str) {
        if let Some((_, service_keys)) = self.subscribers.remove(connection_id) {
            for service_key in &service_keys {
                if let Some(mut connections) = self.subscriber_index.get_mut(service_key) {
                    connections.remove(connection_id);
                }
            }
        }
        self.publishers.remove(connection_id);
    }

    // ========================================================================
    // Connection Instance Tracking
    // ========================================================================

    /// Track that a connection registered an instance
    pub fn add_connection_instance(
        &self,
        connection_id: &str,
        service_key: &str,
        instance_key: &str,
    ) {
        if self.closing_connections.contains_key(connection_id) {
            return;
        }
        self.connection_instances
            .entry(connection_id.to_string())
            .or_default()
            .insert((service_key.to_string(), instance_key.to_string()));
    }

    /// Untrack a connection's instance
    pub fn remove_connection_instance(
        &self,
        connection_id: &str,
        service_key: &str,
        instance_key: &str,
    ) {
        if let Some(mut instances) = self.connection_instances.get_mut(connection_id) {
            instances.remove(&(service_key.to_string(), instance_key.to_string()));
        }
    }

    /// Deregister all instances for a connection (on disconnect)
    ///
    /// Returns affected service keys for subscriber notification.
    pub fn deregister_all_by_connection(&self, connection_id: &str) -> Vec<String> {
        // Mark as closing
        self.closing_connections
            .insert(connection_id.to_string(), ());

        let instances_to_remove: Vec<(String, String)> =
            match self.connection_instances.remove(connection_id) {
                Some((_, instances)) => instances.into_iter().collect(),
                None => Vec::new(),
            };

        let mut affected = HashSet::new();
        for (service_key, instance_key) in instances_to_remove {
            if let Some(svc_instances) = self.services.get(&service_key)
                && svc_instances.remove(&instance_key).is_some()
            {
                affected.insert(service_key);
            }
        }

        self.publishers.remove(connection_id);
        self.closing_connections.remove(connection_id);

        affected.into_iter().collect()
    }

    // ========================================================================
    // Publisher Tracking
    // ========================================================================

    /// Add a publisher
    pub fn add_publisher(&self, connection_id: &str, namespace: &str, group: &str, service: &str) {
        let service_key = build_service_key(namespace, group, service);
        self.publishers
            .entry(connection_id.to_string())
            .or_default()
            .insert(service_key);
    }

    /// Remove a publisher
    pub fn remove_publisher(
        &self,
        connection_id: &str,
        namespace: &str,
        group: &str,
        service: &str,
    ) {
        let service_key = build_service_key(namespace, group, service);
        if let Some(mut services) = self.publishers.get_mut(connection_id) {
            services.remove(&service_key);
        }
    }

    /// Get all services published by a connection
    pub fn get_published_services(&self, connection_id: &str) -> Vec<String> {
        self.publishers
            .get(connection_id)
            .map(|entry| entry.value().iter().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NacosInstance;

    fn test_instance(ip: &str, port: i32) -> NacosInstance {
        NacosInstance {
            ip: ip.to_string(),
            port,
            cluster_name: "DEFAULT".to_string(),
            service_name: "test-svc".to_string(),
            ephemeral: true,
            healthy: true,
            enabled: true,
            weight: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn test_subscribe_unsubscribe() {
        let svc = NacosNamingServiceImpl::new();

        svc.subscribe("conn-1", "public", "DEFAULT_GROUP", "test-svc", "");
        let subs = svc.get_subscribers("public", "DEFAULT_GROUP", "test-svc");
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0], "conn-1");

        svc.unsubscribe("conn-1", "public", "DEFAULT_GROUP", "test-svc", "");
        let subs = svc.get_subscribers("public", "DEFAULT_GROUP", "test-svc");
        assert!(subs.is_empty());
    }

    #[test]
    fn test_deregister_all_by_connection() {
        let svc = NacosNamingServiceImpl::new();
        let inst = test_instance("10.0.0.1", 8080);

        svc.register_instance("public", "DEFAULT_GROUP", "test-svc", inst);
        let service_key = build_service_key("public", "DEFAULT_GROUP", "test-svc");
        svc.add_connection_instance("conn-1", &service_key, "10.0.0.1#8080#DEFAULT");

        assert_eq!(
            svc.get_instance_count("public", "DEFAULT_GROUP", "test-svc"),
            1
        );

        let affected = svc.deregister_all_by_connection("conn-1");
        assert_eq!(affected.len(), 1);
        assert_eq!(
            svc.get_instance_count("public", "DEFAULT_GROUP", "test-svc"),
            0
        );
    }
}
