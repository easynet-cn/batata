//! Instance CRUD: register, deregister, query, heartbeat, health

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::model::{NacosInstance, NacosService, ProtectionInfo};

use super::{
    NacosNamingServiceImpl, build_instance_key, build_instance_key_parts, build_service_key,
    parse_service_key,
};

#[allow(clippy::too_many_arguments)]
impl NacosNamingServiceImpl {
    /// Register a service instance
    pub fn register_instance(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        mut instance: NacosInstance,
    ) -> bool {
        let service_key = build_service_key(namespace, group, service);

        // Normalize defaults
        instance.weight = clamp_weight(instance.weight);
        if instance.cluster_name.is_empty() {
            instance.cluster_name = "DEFAULT".to_string();
        }
        instance.service_name = service.to_string();

        let instance_key = build_instance_key(&instance);

        // Get or create service entry
        let instances = self.services.entry(service_key.clone()).or_default();
        instances.insert(instance_key, Arc::new(instance));

        // Auto-create ServiceMetadata if needed
        self.service_metadata
            .entry(service_key.clone())
            .or_default();

        self.increment_revision(&service_key);
        true
    }

    /// Deregister a service instance
    pub fn deregister_instance(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        ip: &str,
        port: i32,
        cluster: &str,
        _ephemeral: bool,
    ) -> bool {
        let service_key = build_service_key(namespace, group, service);

        let cluster = if cluster.is_empty() {
            "DEFAULT"
        } else {
            cluster
        };
        let instance_key = build_instance_key_parts(ip, port, cluster);

        if let Some(instances) = self.services.get(&service_key)
            && instances.remove(&instance_key).is_some()
        {
            self.increment_revision(&service_key);
        }
        // Return true for idempotency (Nacos SDK expects this)
        true
    }

    /// Get instances for a service
    pub fn get_instances(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        clusters: &str,
        healthy_only: bool,
    ) -> Vec<Arc<NacosInstance>> {
        let service_key = build_service_key(namespace, group, service);

        // Snapshot Arc pointers (brief shard lock)
        let snapshot: Vec<Arc<NacosInstance>> = match self.services.get(&service_key) {
            Some(instances) => instances.iter().map(|e| Arc::clone(e.value())).collect(),
            None => return Vec::new(),
        };

        // Filter on snapshot (no locks held)
        snapshot
            .into_iter()
            .filter(|inst| {
                let cluster_match = clusters.is_empty()
                    || clusters == "*"
                    || clusters.split(',').any(|c| c.trim() == inst.cluster_name);
                let health_match = !healthy_only || inst.healthy;
                cluster_match && health_match
            })
            .collect()
    }

    /// Get service with protection threshold logic
    pub fn get_service(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        clusters: &str,
        healthy_only: bool,
    ) -> NacosService {
        let service_key = build_service_key(namespace, group, service);

        let protect_threshold = self
            .service_metadata
            .get(&service_key)
            .map(|m| m.protect_threshold)
            .unwrap_or(0.0);

        // Snapshot Arc pointers (brief shard lock)
        let (snapshot, has_any) = match self.services.get(&service_key) {
            Some(all) => {
                let has = !all.is_empty();
                let snap: Vec<Arc<NacosInstance>> =
                    all.iter().map(|e| Arc::clone(e.value())).collect();
                (snap, has)
            }
            None => (Vec::new(), false),
        };

        // Filter by cluster
        let cluster_filtered: Vec<&Arc<NacosInstance>> = snapshot
            .iter()
            .filter(|inst| {
                clusters.is_empty()
                    || clusters == "*"
                    || clusters.split(',').any(|c| c.trim() == inst.cluster_name)
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

        let hosts: Vec<NacosInstance> = if protection_triggered {
            // Return ALL instances to prevent cascading failures
            cluster_filtered
                .iter()
                .map(|i| NacosInstance::clone(i))
                .collect()
        } else if healthy_only {
            cluster_filtered
                .iter()
                .filter(|i| i.healthy)
                .map(|i| NacosInstance::clone(i))
                .collect()
        } else {
            cluster_filtered
                .iter()
                .map(|i| NacosInstance::clone(i))
                .collect()
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        NacosService {
            name: service.to_string(),
            group_name: group.to_string(),
            clusters: clusters.to_string(),
            cache_millis: 10000,
            hosts,
            last_ref_time: now,
            checksum: String::new(),
            all_ips: has_any,
            reach_protection_threshold: protection_triggered,
        }
    }

    /// Get service with protection info
    pub fn get_service_with_protection_info(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        clusters: &str,
        healthy_only: bool,
    ) -> (NacosService, ProtectionInfo) {
        let svc = self.get_service(namespace, group, service, clusters, healthy_only);

        let service_key = build_service_key(namespace, group, service);
        let protect_threshold = self
            .service_metadata
            .get(&service_key)
            .map(|m| m.protect_threshold)
            .unwrap_or(0.0);

        // Recount from hosts (already filtered)
        let total = svc.hosts.len();
        let healthy = svc.hosts.iter().filter(|h| h.healthy).count();
        let ratio = if total > 0 {
            healthy as f32 / total as f32
        } else {
            1.0
        };

        let info = ProtectionInfo {
            threshold: protect_threshold,
            total_instances: total,
            healthy_instances: healthy,
            healthy_ratio: ratio,
            triggered: svc.reach_protection_threshold,
        };

        (svc, info)
    }

    /// Get all service keys (for Distro sync)
    pub fn get_all_service_keys(&self) -> Vec<String> {
        self.services.iter().map(|e| e.key().clone()).collect()
    }

    /// List services with pagination
    pub fn list_services(
        &self,
        namespace: &str,
        group: &str,
        page_no: u32,
        page_size: u32,
    ) -> (u32, Vec<String>) {
        let prefix = if group.is_empty() {
            format!("{namespace}@@")
        } else {
            format!("{namespace}@@{group}@@")
        };

        let mut name_set = std::collections::HashSet::new();

        for entry in self.services.iter() {
            if entry.key().starts_with(&prefix)
                && let Some((_, _, svc)) = parse_service_key(entry.key())
            {
                name_set.insert(svc.to_string());
            }
        }

        for entry in self.service_metadata.iter() {
            if entry.key().starts_with(&prefix)
                && let Some((_, _, svc)) = parse_service_key(entry.key())
            {
                name_set.insert(svc.to_string());
            }
        }

        let mut names: Vec<String> = name_set.into_iter().collect();
        names.sort();

        let total = names.len() as u32;
        let start = ((page_no.saturating_sub(1)) * page_size) as usize;
        let paginated = names
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();

        (total, paginated)
    }

    /// Batch register instances (reconciliation model — replaces all)
    pub fn batch_register_instances(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        instances: Vec<NacosInstance>,
    ) -> bool {
        let service_key = build_service_key(namespace, group, service);

        let entry = self.services.entry(service_key.clone()).or_default();
        entry.clear();

        for mut instance in instances {
            instance.weight = clamp_weight(instance.weight);
            if instance.cluster_name.is_empty() {
                instance.cluster_name = "DEFAULT".to_string();
            }
            instance.service_name = service.to_string();
            let key = build_instance_key(&instance);
            entry.insert(key, Arc::new(instance));
        }

        self.increment_revision(&service_key);
        true
    }

    /// Batch deregister instances
    pub fn batch_deregister_instances(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        instances: Vec<NacosInstance>,
    ) -> bool {
        let service_key = build_service_key(namespace, group, service);

        if let Some(svc_instances) = self.services.get(&service_key) {
            for instance in &instances {
                let cluster = if instance.cluster_name.is_empty() {
                    "DEFAULT"
                } else {
                    &instance.cluster_name
                };
                let key = build_instance_key_parts(&instance.ip, instance.port, cluster);
                svc_instances.remove(&key);
            }
            self.increment_revision(&service_key);
        }
        true
    }

    /// Replace all ephemeral instances (for Distro sync)
    pub fn replace_ephemeral_instances(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        instances: Vec<NacosInstance>,
    ) -> bool {
        let service_key = build_service_key(namespace, group, service);

        let entry = self.services.entry(service_key.clone()).or_default();

        // Remove existing ephemeral instances
        let keys_to_remove: Vec<String> = entry
            .iter()
            .filter(|e| e.value().ephemeral)
            .map(|e| e.key().clone())
            .collect();
        for key in keys_to_remove {
            entry.remove(&key);
        }

        // Insert new ephemeral instances
        for mut instance in instances {
            instance.weight = clamp_weight(instance.weight);
            if instance.cluster_name.is_empty() {
                instance.cluster_name = "DEFAULT".to_string();
            }
            instance.service_name = service.to_string();
            let key = build_instance_key(&instance);
            entry.insert(key, Arc::new(instance));
        }

        self.increment_revision(&service_key);
        true
    }

    /// Update instance health status
    pub fn update_instance_health(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        ip: &str,
        port: i32,
        cluster: &str,
        healthy: bool,
    ) -> bool {
        let service_key = build_service_key(namespace, group, service);
        let cluster = if cluster.is_empty() {
            "DEFAULT"
        } else {
            cluster
        };
        let instance_key = build_instance_key_parts(ip, port, cluster);

        if let Some(instances) = self.services.get(&service_key)
            && let Some(old) = instances.get(&instance_key)
        {
            if old.healthy != healthy {
                let mut updated = (**old).clone();
                updated.healthy = healthy;
                drop(old);
                instances.insert(instance_key, Arc::new(updated));
                self.increment_revision(&service_key);
            }
            return true;
        }
        false
    }

    /// Process heartbeat — returns true if instance exists
    pub fn heartbeat(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        ip: &str,
        port: i32,
        cluster: &str,
    ) -> bool {
        let service_key = build_service_key(namespace, group, service);
        let cluster = if cluster.is_empty() {
            "DEFAULT"
        } else {
            cluster
        };
        let instance_key = build_instance_key_parts(ip, port, cluster);

        if let Some(instances) = self.services.get(&service_key)
            && let Some(old) = instances.get(&instance_key)
        {
            if !old.healthy {
                // Restore health on heartbeat
                let mut updated = (**old).clone();
                updated.healthy = true;
                drop(old);
                instances.insert(instance_key, Arc::new(updated));
                self.increment_revision(&service_key);
            }
            return true;
        }
        false
    }

    /// Check if a service exists
    pub fn service_exists(&self, namespace: &str, group: &str, service: &str) -> bool {
        let service_key = build_service_key(namespace, group, service);
        self.services.contains_key(&service_key) || self.service_metadata.contains_key(&service_key)
    }

    /// Get instance count for a service
    pub fn get_instance_count(&self, namespace: &str, group: &str, service: &str) -> usize {
        let service_key = build_service_key(namespace, group, service);
        self.services
            .get(&service_key)
            .map(|instances| instances.len())
            .unwrap_or(0)
    }
}

/// Clamp weight to valid range [0.0, 10000.0]
fn clamp_weight(weight: f64) -> f64 {
    weight.clamp(0.0, 10000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_instance(ip: &str, port: i32) -> NacosInstance {
        NacosInstance {
            instance_id: NacosInstance::generate_id(ip, port, "DEFAULT", "test-svc"),
            ip: ip.to_string(),
            port,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: "test-svc".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_register_and_get() {
        let svc = NacosNamingServiceImpl::new();
        let inst = test_instance("10.0.0.1", 8080);

        assert!(svc.register_instance("public", "DEFAULT_GROUP", "test-svc", inst));

        let instances = svc.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].ip, "10.0.0.1");
        assert_eq!(instances[0].port, 8080);
        assert!(instances[0].healthy);
    }

    #[test]
    fn test_deregister() {
        let svc = NacosNamingServiceImpl::new();
        let inst = test_instance("10.0.0.1", 8080);

        svc.register_instance("public", "DEFAULT_GROUP", "test-svc", inst);
        assert_eq!(
            svc.get_instance_count("public", "DEFAULT_GROUP", "test-svc"),
            1
        );

        svc.deregister_instance(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            "10.0.0.1",
            8080,
            "DEFAULT",
            true,
        );
        assert_eq!(
            svc.get_instance_count("public", "DEFAULT_GROUP", "test-svc"),
            0
        );
    }

    #[test]
    fn test_protection_threshold() {
        let svc = NacosNamingServiceImpl::new();

        // Register 2 instances, one unhealthy
        let healthy = test_instance("10.0.0.1", 8080);
        let mut unhealthy = test_instance("10.0.0.2", 8080);
        unhealthy.healthy = false;

        svc.register_instance("public", "DEFAULT_GROUP", "test-svc", healthy);
        svc.register_instance("public", "DEFAULT_GROUP", "test-svc", unhealthy);

        // Set protection threshold to 0.8 (80%)
        svc.set_service_metadata(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            crate::model::ServiceMetadata {
                protect_threshold: 0.8,
                ..Default::default()
            },
        );

        // healthy_only=true but protection triggered (ratio 0.5 < 0.8) → returns all
        let result = svc.get_service("public", "DEFAULT_GROUP", "test-svc", "", true);
        assert!(result.reach_protection_threshold);
        assert_eq!(result.hosts.len(), 2);
    }

    #[test]
    fn test_list_services() {
        let svc = NacosNamingServiceImpl::new();
        svc.register_instance(
            "public",
            "DEFAULT_GROUP",
            "svc-a",
            test_instance("10.0.0.1", 8080),
        );
        svc.register_instance(
            "public",
            "DEFAULT_GROUP",
            "svc-b",
            test_instance("10.0.0.2", 8080),
        );
        svc.register_instance(
            "other-ns",
            "DEFAULT_GROUP",
            "svc-c",
            test_instance("10.0.0.3", 8080),
        );

        let (total, names) = svc.list_services("public", "DEFAULT_GROUP", 1, 10);
        assert_eq!(total, 2);
        assert!(names.contains(&"svc-a".to_string()));
        assert!(names.contains(&"svc-b".to_string()));
    }

    #[test]
    fn test_heartbeat_restores_health() {
        let svc = NacosNamingServiceImpl::new();
        let inst = test_instance("10.0.0.1", 8080);
        svc.register_instance("public", "DEFAULT_GROUP", "test-svc", inst);

        // Mark unhealthy
        svc.update_instance_health(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            "10.0.0.1",
            8080,
            "DEFAULT",
            false,
        );
        let instances = svc.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert!(!instances[0].healthy);

        // Heartbeat restores health
        assert!(svc.heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            "10.0.0.1",
            8080,
            "DEFAULT"
        ));
        let instances = svc.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert!(instances[0].healthy);
    }

    #[test]
    fn test_cluster_filter() {
        let svc = NacosNamingServiceImpl::new();
        let mut inst_a = test_instance("10.0.0.1", 8080);
        inst_a.cluster_name = "cluster-a".to_string();
        let mut inst_b = test_instance("10.0.0.2", 8080);
        inst_b.cluster_name = "cluster-b".to_string();

        svc.register_instance("public", "DEFAULT_GROUP", "test-svc", inst_a);
        svc.register_instance("public", "DEFAULT_GROUP", "test-svc", inst_b);

        // Filter by cluster-a
        let result = svc.get_instances("public", "DEFAULT_GROUP", "test-svc", "cluster-a", false);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].cluster_name, "cluster-a");

        // Multi-cluster filter
        let result = svc.get_instances(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            "cluster-a,cluster-b",
            false,
        );
        assert_eq!(result.len(), 2);
    }
}
