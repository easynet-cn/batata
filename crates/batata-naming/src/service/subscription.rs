//! Subscriber, publisher, and connection instance tracking

use std::collections::HashSet;

use super::{NamingService, build_service_key};

impl NamingService {
    /// Subscribe to a service
    pub fn subscribe(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);

        self.subscribers
            .entry(connection_id.to_string())
            .or_default()
            .insert(service_key.clone());

        // Maintain reverse index: service_key -> connection_ids
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
        group_name: &str,
        service_name: &str,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);

        if let Some(mut subs) = self.subscribers.get_mut(connection_id) {
            subs.remove(&service_key);
        }

        // Maintain reverse index
        if let Some(mut connections) = self.subscriber_index.get_mut(&service_key) {
            connections.remove(connection_id);
        }
    }

    /// Get subscribers for a service (O(1) lookup via reverse index)
    pub fn get_subscribers(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<String> {
        let service_key = build_service_key(namespace, group_name, service_name);

        self.subscriber_index
            .get(&service_key)
            .map(|entry| entry.value().iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clean up subscriber when connection is closed
    pub fn remove_subscriber(&self, connection_id: &str) {
        // Remove from reverse index for all subscribed services
        if let Some((_, service_keys)) = self.subscribers.remove(connection_id) {
            for service_key in &service_keys {
                if let Some(mut connections) = self.subscriber_index.get_mut(service_key) {
                    connections.remove(connection_id);
                }
            }
        }
        self.fuzzy_watchers.remove(connection_id);
        self.publishers.remove(connection_id);
    }

    // ============== Connection Instance Tracking ==============

    /// Track that a connection registered a specific instance.
    /// Called when an ephemeral instance is registered via gRPC.
    pub fn add_connection_instance(
        &self,
        connection_id: &str,
        service_key: &str,
        instance_key: &str,
    ) {
        self.connection_instances
            .entry(connection_id.to_string())
            .or_default()
            .insert((service_key.to_string(), instance_key.to_string()));
    }

    /// Untrack a connection's instance.
    /// Called when an ephemeral instance is deregistered via gRPC.
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

    /// Deregister all instances associated with a connection.
    /// Called when a gRPC connection disconnects to clean up orphan ephemeral instances.
    /// Returns the set of affected service keys (for subscriber notification).
    pub fn deregister_all_by_connection(&self, connection_id: &str) -> Vec<String> {
        // Remove connection tracking first (releases connection_instances lock)
        let instances_to_remove: Vec<(String, String)> =
            match self.connection_instances.remove(connection_id) {
                Some((_, instances)) => instances.into_iter().collect(),
                None => Vec::new(),
            };
        // --- connection_instances lock released ---

        // Now process removals one by one (each get() is a brief shard lock)
        let mut affected_service_keys = HashSet::new();
        for (service_key, instance_key) in instances_to_remove {
            if let Some(service_instances) = self.services.get(&service_key)
                && service_instances.remove(&instance_key).is_some()
            {
                affected_service_keys.insert(service_key);
            }
        }

        // Also clean up publisher tracking
        self.publishers.remove(connection_id);

        affected_service_keys.into_iter().collect()
    }

    // ============== Publisher Tracking (for V2 Client API) ==============

    /// Track that a connection published a service
    pub fn add_publisher(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.publishers
            .entry(connection_id.to_string())
            .or_default()
            .insert(service_key);
    }

    /// Remove publisher tracking for a service
    pub fn remove_publisher(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        if let Some(mut pubs) = self.publishers.get_mut(connection_id) {
            pubs.remove(&service_key);
        }
    }

    /// Get all services published by a connection
    pub fn get_published_services(&self, connection_id: &str) -> Vec<String> {
        self.publishers
            .get(connection_id)
            .map(|pubs| pubs.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all connections that published a specific service
    pub fn get_publishers(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<String> {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.publishers
            .iter()
            .filter(|entry| entry.value().contains(&service_key))
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get all services subscribed by a connection
    pub fn get_subscribed_services(&self, connection_id: &str) -> Vec<String> {
        self.subscribers
            .get(connection_id)
            .map(|subs| subs.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all connection IDs that are publishers
    pub fn get_all_publisher_ids(&self) -> Vec<String> {
        self.publishers.iter().map(|e| e.key().clone()).collect()
    }

    /// Get all connection IDs that are subscribers
    pub fn get_all_subscriber_ids(&self) -> Vec<String> {
        self.subscribers.iter().map(|e| e.key().clone()).collect()
    }
}
