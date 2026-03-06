//! Instance CRUD operations: register, deregister, query, heartbeat, health

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::model::{Instance, Service};

use super::{NamingService, build_instance_key, build_service_key};

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

        // Increment service revision for change detection
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
            instances.remove(&instance_key);
            return true;
        }
        false
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

        // Get protection threshold from service metadata (brief lock)
        let protect_threshold = self
            .service_metadata
            .get(&service_key)
            .map(|m| m.protect_threshold)
            .unwrap_or(0.0);

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

        // Filter on Arc references (no clones needed for reads)
        let cluster_filtered: Vec<&Arc<Instance>> = snapshot
            .iter()
            .filter(|inst| {
                cluster.is_empty()
                    || cluster == "*"
                    || cluster.split(',').any(|c| c.trim() == inst.cluster_name)
            })
            .collect();

        let total = cluster_filtered.len();
        let healthy_count = cluster_filtered.iter().filter(|i| i.healthy).count();

        let healthy_ratio = if total > 0 {
            healthy_count as f32 / total as f32
        } else {
            1.0
        };

        let protection_triggered = protect_threshold > 0.0 && healthy_ratio < protect_threshold;

        // Clone only the final set of instances to return
        let instances: Vec<Instance> = if protection_triggered {
            cluster_filtered.iter().map(|i| (***i).clone()).collect()
        } else if healthy_only {
            cluster_filtered
                .iter()
                .filter(|i| i.healthy)
                .map(|i| (***i).clone())
                .collect()
        } else {
            cluster_filtered.iter().map(|i| (***i).clone()).collect()
        };

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

        // Get protection threshold from service metadata (brief lock)
        let protect_threshold = self
            .service_metadata
            .get(&service_key)
            .map(|m| m.protect_threshold)
            .unwrap_or(0.0);

        // Snapshot Arc pointers (brief outer shard lock)
        let snapshot: Vec<Arc<Instance>> = self
            .services
            .get(&service_key)
            .map(|inner| inner.iter().map(|e| Arc::clone(e.value())).collect())
            .unwrap_or_default();
        // --- outer shard lock released ---

        // Filter on Arc references (no clones needed for reads)
        let cluster_filtered: Vec<&Arc<Instance>> = snapshot
            .iter()
            .filter(|inst| {
                cluster.is_empty()
                    || cluster == "*"
                    || cluster.split(',').any(|c| c.trim() == inst.cluster_name)
            })
            .collect();

        let total = cluster_filtered.len();
        let healthy_count = cluster_filtered.iter().filter(|i| i.healthy).count();
        let has_any = !cluster_filtered.is_empty();

        let healthy_ratio = if total > 0 {
            healthy_count as f32 / total as f32
        } else {
            1.0
        };

        let protection_triggered = protect_threshold > 0.0 && healthy_ratio < protect_threshold;

        // Clone only the final set of instances to return
        let instances: Vec<Instance> = if protection_triggered {
            cluster_filtered.iter().map(|i| (***i).clone()).collect()
        } else if healthy_only {
            cluster_filtered
                .iter()
                .filter(|i| i.healthy)
                .map(|i| (***i).clone())
                .collect()
        } else {
            cluster_filtered.iter().map(|i| (***i).clone()).collect()
        };

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

    /// List all services in a namespace
    /// Single-pass iteration to minimize DashMap lock hold time
    pub fn list_services(
        &self,
        namespace: &str,
        group_name: &str,
        page_no: i32,
        page_size: i32,
    ) -> (i32, Vec<String>) {
        let prefix = if group_name.is_empty() {
            format!("{}@@", namespace)
        } else {
            format!("{}@@{}@@", namespace, group_name)
        };

        // Single pass: collect all matching service names, then paginate on owned data.
        // This avoids iterating the DashMap twice (which holds shard read locks each time).
        let all_names: Vec<String> = self
            .services
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix))
            .map(|entry| {
                super::parse_service_key(entry.key())
                    .map(|(_, _, svc)| svc.to_string())
                    .unwrap_or_else(|| entry.key().clone())
            })
            .collect();
        // --- all shard locks released ---

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

        // Increment service revision for change detection
        self.increment_service_revision(&service_key);
        true
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
        let instance_key = format!("{}#{}#{}", ip, port, cluster_name);

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
        let instance_key = format!("{}#{}#{}", ip, port, cluster_name);

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

    /// Check if a service exists (has metadata or instances)
    pub fn service_exists(&self, namespace: &str, group_name: &str, service_name: &str) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.service_metadata.contains_key(&service_key)
            || self
                .services
                .get(&service_key)
                .is_some_and(|s| !s.is_empty())
    }
}
