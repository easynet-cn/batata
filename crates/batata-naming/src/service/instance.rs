//! Instance CRUD operations: register, deregister, query, heartbeat, health

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::model::{Instance, Service};

use super::{NamingService, build_instance_key, build_instance_key_parts, build_service_key};

impl NamingService {
    /// Register a service instance
    pub fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        mut instance: Instance,
    ) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);

        // Normalize defaults BEFORE building instance key for consistent storage
        instance.weight = batata_api::naming::model::clamp_weight(instance.weight);
        if instance.cluster_name.is_empty() {
            instance.cluster_name = "DEFAULT".to_string();
        }
        instance.service_name = service_name.to_string();

        let instance_key = build_instance_key(&instance);

        // Get or create service entry
        let instances = self.services.entry(service_key.clone()).or_default();
        instances.insert(instance_key, Arc::new(instance));

        // Auto-create ServiceMetadata if it doesn't exist yet
        self.service_metadata
            .entry(service_key.clone())
            .or_default();

        // Update service name index and revision
        self.index_service_name(&service_key);
        self.increment_service_revision(&service_key);
        true
    }

    /// Deregister a service instance
    pub fn deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);

        // If cluster_name is empty, try "DEFAULT" (the default cluster name)
        let instance_key = if instance.cluster_name.is_empty() {
            format!("{}#{}#DEFAULT", instance.ip, instance.port)
        } else {
            build_instance_key(instance)
        };

        if let Some(instances) = self.services.get(&service_key) {
            if instances.remove(&instance_key).is_some() {
                self.increment_service_revision(&service_key);
            }
            // Return true for idempotency — deregistering a non-existent instance
            // is OK (Nacos SDK expects this during cleanup)
            return true;
        }
        // Service not found — still return true for idempotency
        true
    }

    /// Get all instances for a service
    pub fn get_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Vec<Instance> {
        let service_key = build_service_key(namespace, group_name, service_name);

        // Snapshot Arc pointers first to minimize outer DashMap shard lock hold time.
        // Arc::clone is a pointer copy, so the shard read lock is held very briefly.
        let snapshot: Vec<Arc<Instance>> = match self.services.get(&service_key) {
            Some(instances) => instances.iter().map(|e| Arc::clone(e.value())).collect(),
            None => return Vec::new(),
        };

        // Filter on the snapshot (no locks held), then clone only matching instances
        snapshot
            .iter()
            .filter(|inst| {
                let cluster_match = cluster.is_empty()
                    || cluster == "*"
                    || cluster.split(',').any(|c| c.trim() == inst.cluster_name);
                let health_match = !healthy_only || inst.healthy;
                cluster_match && health_match
            })
            .map(|inst| (**inst).clone())
            .collect()
    }

    /// Get service info with instances
    ///
    /// This method implements protection threshold logic:
    /// - If healthy_ratio < protect_threshold, return ALL instances (including unhealthy)
    /// - This prevents cascading failures when too many instances become unhealthy
    pub fn get_service(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Service {
        let service_key = build_service_key(namespace, group_name, service_name);

        // Get protection threshold and metadata from service metadata (brief lock)
        let (protect_threshold, service_meta_map) = self
            .service_metadata
            .get(&service_key)
            .map(|m| (m.protect_threshold, m.metadata.clone()))
            .unwrap_or_default();

        // Snapshot Arc pointers to minimize outer DashMap shard lock hold time.
        let (snapshot, has_any_instances) = match self.services.get(&service_key) {
            Some(all_instances) => {
                let has_any = !all_instances.is_empty();
                let snap: Vec<Arc<Instance>> = all_instances
                    .iter()
                    .map(|e| Arc::clone(e.value()))
                    .collect();
                (snap, has_any)
            }
            None => (Vec::new(), false),
        };
        // --- outer shard lock released here ---

        // Single-pass: count total and healthy instances matching cluster filter.
        // Avoids intermediate Vec allocation for cluster_filtered.
        let mut total = 0usize;
        let mut healthy_count = 0usize;
        for inst in &snapshot {
            let cluster_match = cluster.is_empty()
                || cluster == "*"
                || cluster.split(',').any(|c| c.trim() == inst.cluster_name);
            if cluster_match {
                total += 1;
                if inst.healthy {
                    healthy_count += 1;
                }
            }
        }

        let healthy_ratio = if total > 0 {
            healthy_count as f32 / total as f32
        } else {
            1.0
        };

        let protection_triggered = protect_threshold > 0.0 && healthy_ratio < protect_threshold;

        // Second pass: clone only the final set of instances matching all filters.
        // We must iterate again because protection_triggered depends on full counts.
        let instances: Vec<Instance> = snapshot
            .iter()
            .filter(|inst| {
                let cluster_match = cluster.is_empty()
                    || cluster == "*"
                    || cluster.split(',').any(|c| c.trim() == inst.cluster_name);
                let health_match = protection_triggered || !healthy_only || inst.healthy;
                cluster_match && health_match
            })
            .map(|inst| Instance::clone(inst))
            .collect();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Service {
            name: service_name.to_string(),
            group_name: group_name.to_string(),
            clusters: cluster.to_string(),
            cache_millis: 10000,
            hosts: instances,
            last_ref_time: now,
            checksum: String::new(),
            all_ips: has_any_instances,
            reach_protection_threshold: protection_triggered,
            metadata: service_meta_map,
            protect_threshold,
        }
    }

    /// Get service info with protection threshold check
    ///
    /// Returns additional information about protection threshold status
    pub fn get_service_with_protection_info(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> (Service, super::ProtectionInfo) {
        let service_key = build_service_key(namespace, group_name, service_name);

        // Get protection threshold and metadata from service metadata (brief lock)
        let (protect_threshold, service_meta_map) = self
            .service_metadata
            .get(&service_key)
            .map(|m| (m.protect_threshold, m.metadata.clone()))
            .unwrap_or_default();

        // Snapshot Arc pointers (brief outer shard lock)
        let snapshot: Vec<Arc<Instance>> = self
            .services
            .get(&service_key)
            .map(|inner| inner.iter().map(|e| Arc::clone(e.value())).collect())
            .unwrap_or_default();
        // --- outer shard lock released ---

        // Single-pass: count total and healthy instances matching cluster filter
        let mut total = 0usize;
        let mut healthy_count = 0usize;
        for inst in &snapshot {
            let cluster_match = cluster.is_empty()
                || cluster == "*"
                || cluster.split(',').any(|c| c.trim() == inst.cluster_name);
            if cluster_match {
                total += 1;
                if inst.healthy {
                    healthy_count += 1;
                }
            }
        }

        let has_any = total > 0;

        let healthy_ratio = if total > 0 {
            healthy_count as f32 / total as f32
        } else {
            1.0
        };

        let protection_triggered = protect_threshold > 0.0 && healthy_ratio < protect_threshold;

        // Second pass: clone only the final set of instances matching all filters
        let instances: Vec<Instance> = snapshot
            .iter()
            .filter(|inst| {
                let cluster_match = cluster.is_empty()
                    || cluster == "*"
                    || cluster.split(',').any(|c| c.trim() == inst.cluster_name);
                let health_match = protection_triggered || !healthy_only || inst.healthy;
                cluster_match && health_match
            })
            .map(|inst| Instance::clone(inst))
            .collect();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let service = Service {
            name: service_name.to_string(),
            group_name: group_name.to_string(),
            clusters: cluster.to_string(),
            cache_millis: 10000,
            hosts: instances,
            last_ref_time: now,
            checksum: String::new(),
            all_ips: has_any,
            reach_protection_threshold: protection_triggered,
            metadata: service_meta_map,
            protect_threshold,
        };

        let protection_info = super::ProtectionInfo {
            threshold: protect_threshold,
            total_instances: total,
            healthy_instances: healthy_count,
            healthy_ratio,
            triggered: protection_triggered,
        };

        (service, protection_info)
    }

    /// Get all service keys (for distro protocol sync)
    /// Returns keys in format: namespace@@group@@serviceName
    pub fn get_all_service_keys(&self) -> Vec<String> {
        self.services.iter().map(|e| e.key().clone()).collect()
    }

    /// List all services in a namespace.
    /// Uses the service_name_index for O(1) prefix lookup instead of scanning all services.
    pub fn list_services(
        &self,
        namespace: &str,
        group_name: &str,
        page_no: i32,
        page_size: i32,
    ) -> (i32, Vec<String>) {
        let mut all_names: Vec<String> = if group_name.is_empty() {
            // No group filter: collect from all groups matching the namespace
            let ns_prefix = format!("{}@@", namespace);
            let mut name_set = std::collections::HashSet::new();
            for entry in self.service_name_index.iter() {
                if entry.key().starts_with(&ns_prefix) {
                    for name in entry.value().iter() {
                        name_set.insert(name.clone());
                    }
                }
            }
            name_set.into_iter().collect()
        } else {
            // Specific group: direct O(1) lookup
            let prefix = format!("{}@@{}", namespace, group_name);
            self.service_name_index
                .get(&prefix)
                .map(|names| names.iter().cloned().collect())
                .unwrap_or_default()
        };

        all_names.sort(); // deterministic ordering for pagination

        let total = all_names.len() as i32;
        let start = ((page_no - 1) * page_size) as usize;
        let paginated = all_names
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();

        (total, paginated)
    }

    /// Batch register instances (reconciliation model)
    ///
    /// Replaces ALL instances for the given service with the provided list.
    /// This follows the Nacos reconciliation model where batch register
    /// represents the full desired state, not an incremental addition.
    pub fn batch_register_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);

        // Clear existing instances for this service, then register the new ones
        let entry = self.services.entry(service_key.clone()).or_default();
        entry.clear();

        for instance in instances {
            let mut instance = instance;
            instance.weight = batata_api::naming::model::clamp_weight(instance.weight);
            if instance.cluster_name.is_empty() {
                instance.cluster_name = "DEFAULT".to_string();
            }
            instance.service_name = service_name.to_string();
            let instance_key = build_instance_key(&instance);
            entry.insert(instance_key, Arc::new(instance));
        }

        // Update service name index and revision
        self.index_service_name(&service_key);
        self.increment_service_revision(&service_key);
        true
    }

    /// Replace all ephemeral instances for a service with the provided list.
    ///
    /// This is used by the Distro protocol to reconcile ephemeral instance state
    /// from the responsible node. Persistent (non-ephemeral) instances are preserved.
    pub fn replace_ephemeral_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);

        let entry = self.services.entry(service_key.clone()).or_default();

        // Remove all existing ephemeral instances
        let keys_to_remove: Vec<String> = entry
            .iter()
            .filter(|e| e.value().ephemeral)
            .map(|e| e.key().clone())
            .collect();
        for key in keys_to_remove {
            entry.remove(&key);
        }

        // Insert the new ephemeral instances
        for instance in instances {
            let mut instance = instance;
            instance.weight = batata_api::naming::model::clamp_weight(instance.weight);
            if instance.cluster_name.is_empty() {
                instance.cluster_name = "DEFAULT".to_string();
            }
            instance.service_name = service_name.to_string();
            let instance_key = build_instance_key(&instance);
            entry.insert(instance_key, Arc::new(instance));
        }

        // Increment service revision for change detection
        self.increment_service_revision(&service_key);
        true
    }

    /// Merge remote instances from a Distro sync.
    ///
    /// Adds or updates instances from the sync data. Also removes ephemeral
    /// instances that were previously synced from remote (marked with
    /// `_distro_remote=true` metadata) but are no longer in the incoming data
    /// — this handles deregistration propagation. Locally registered instances
    /// (without the remote marker) are never removed by sync.
    pub fn merge_remote_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        let entry = self.services.entry(service_key.clone()).or_default();

        // Build set of incoming instance keys
        let mut incoming_keys = std::collections::HashSet::new();

        for instance in instances {
            let mut instance = instance;
            instance.weight = batata_api::naming::model::clamp_weight(instance.weight);
            if instance.cluster_name.is_empty() {
                instance.cluster_name = "DEFAULT".to_string();
            }
            instance.service_name = service_name.to_string();
            // Mark as remotely synced
            instance
                .metadata
                .insert("_distro_remote".to_string(), "true".to_string());
            let instance_key = build_instance_key(&instance);
            incoming_keys.insert(instance_key.clone());
            entry.insert(instance_key, Arc::new(instance));
        }

        // Remove ephemeral instances previously synced from remote that are
        // no longer in the incoming data (deregistered on the source node).
        let keys_to_remove: Vec<String> = entry
            .iter()
            .filter(|e| {
                e.value().ephemeral
                    && e.value()
                        .metadata
                        .get("_distro_remote")
                        .map(|v| v == "true")
                        .unwrap_or(false)
                    && !incoming_keys.contains(e.key())
            })
            .map(|e| e.key().clone())
            .collect();
        for key in keys_to_remove {
            entry.remove(&key);
        }

        self.increment_service_revision(&service_key);
    }

    /// Batch deregister instances
    pub fn batch_deregister_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: &[Instance],
    ) -> bool {
        for instance in instances {
            self.deregister_instance(namespace, group_name, service_name, instance);
        }
        true
    }

    /// Update instance heartbeat
    pub fn heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);
        let instance_key = build_instance_key_parts(ip, port, cluster_name);

        if let Some(instances) = self.services.get(&service_key)
            && let Some(entry) = instances.get(&instance_key)
        {
            if !entry.healthy {
                let mut updated = (**entry).clone();
                updated.healthy = true;
                drop(entry);
                instances.insert(instance_key, Arc::new(updated));
            }
            return true;
        }
        false
    }

    /// Update instance health status (for V2 Health API)
    #[allow(clippy::too_many_arguments)]
    pub fn update_instance_health(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
        healthy: bool,
    ) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);
        let instance_key = build_instance_key_parts(ip, port, cluster_name);

        if let Some(instances) = self.services.get(&service_key)
            && let Some(entry) = instances.get(&instance_key)
        {
            if entry.healthy != healthy {
                let mut updated = (**entry).clone();
                updated.healthy = healthy;
                drop(entry);
                instances.insert(instance_key, Arc::new(updated));
            }
            return true;
        }
        false
    }

    /// Get instance count for a service
    pub fn get_instance_count(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> usize {
        let service_key = build_service_key(namespace, group_name, service_name);

        self.services
            .get(&service_key)
            .map(|instances| instances.len())
            .unwrap_or(0)
    }

    /// Get healthy instance count for a service
    pub fn get_healthy_instance_count(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> usize {
        let service_key = build_service_key(namespace, group_name, service_name);

        // Brief lock: count directly during iteration (no nested access)
        self.services
            .get(&service_key)
            .map(|instances| instances.iter().filter(|e| e.value().healthy).count())
            .unwrap_or(0)
    }

    /// Check if a service exists (has metadata or is registered, even with 0 instances)
    pub fn service_exists(&self, namespace: &str, group_name: &str, service_name: &str) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.service_metadata.contains_key(&service_key) || self.services.contains_key(&service_key)
    }
}
