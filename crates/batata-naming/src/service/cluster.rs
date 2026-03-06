//! Cluster configuration and statistics operations

use std::{collections::HashMap, sync::Arc};

use crate::model::Instance;

use super::{
    ClusterConfig, ClusterStatistics, NamingService, build_cluster_key, build_service_key,
};

impl NamingService {
    /// Set cluster configuration
    pub fn set_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        config: ClusterConfig,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        let cluster_key = build_cluster_key(&service_key, cluster_name);
        self.cluster_configs.insert(cluster_key, config);
    }

    /// Get cluster configuration
    pub fn get_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Option<ClusterConfig> {
        let service_key = build_service_key(namespace, group_name, service_name);
        let cluster_key = build_cluster_key(&service_key, cluster_name);
        self.cluster_configs.get(&cluster_key).map(|r| r.clone())
    }

    /// Get all cluster configs for a service
    pub fn get_all_cluster_configs(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<ClusterConfig> {
        let service_key = build_service_key(namespace, group_name, service_name);
        let prefix = format!("{}##", service_key);

        self.cluster_configs
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix))
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Update cluster health check configuration
    #[allow(clippy::too_many_arguments)]
    pub fn update_cluster_health_check(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        health_check_type: &str,
        check_port: i32,
        use_instance_port: bool,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        let cluster_key = build_cluster_key(&service_key, cluster_name);

        let mut entry = self
            .cluster_configs
            .entry(cluster_key)
            .or_insert_with(|| ClusterConfig {
                name: cluster_name.to_string(),
                ..Default::default()
            });
        entry.health_check_type = health_check_type.to_string();
        entry.check_port = check_port;
        entry.use_instance_port = use_instance_port;
    }

    /// Update cluster metadata
    pub fn update_cluster_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        metadata: HashMap<String, String>,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        let cluster_key = build_cluster_key(&service_key, cluster_name);

        self.cluster_configs
            .entry(cluster_key)
            .or_insert_with(|| ClusterConfig {
                name: cluster_name.to_string(),
                ..Default::default()
            })
            .metadata = metadata;
    }

    /// Delete cluster configuration
    pub fn delete_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        let cluster_key = build_cluster_key(&service_key, cluster_name);
        self.cluster_configs.remove(&cluster_key);
    }

    /// Create cluster configuration
    #[allow(clippy::too_many_arguments)]
    pub fn create_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        health_check_type: &str,
        check_port: i32,
        use_instance_port: bool,
        metadata: HashMap<String, String>,
    ) -> Result<(), String> {
        let service_key = build_service_key(namespace, group_name, service_name);

        // Check if service exists
        if !self.services.contains_key(&service_key) {
            return Err(format!("Service {} not found", service_name));
        }

        // Check if cluster already exists
        let cluster_key = build_cluster_key(&service_key, cluster_name);
        if self.cluster_configs.contains_key(&cluster_key) {
            return Err(format!("Cluster {} already exists", cluster_name));
        }

        // Create cluster configuration
        let cluster_config = ClusterConfig {
            name: cluster_name.to_string(),
            health_check_type: health_check_type.to_uppercase(),
            check_port,
            use_instance_port,
            metadata,
            ..Default::default()
        };

        self.cluster_configs.insert(cluster_key, cluster_config);
        Ok(())
    }

    /// Get cluster statistics for a service
    pub fn get_cluster_statistics(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<ClusterStatistics> {
        let service_key = build_service_key(namespace, group_name, service_name);

        if let Some(instances_map) = self.services.get(&service_key) {
            // Snapshot Arc pointers to release the DashMap lock quickly
            let snapshot: Vec<Arc<Instance>> = instances_map
                .iter()
                .map(|e| Arc::clone(e.value()))
                .collect();
            // --- DashMap shard lock released ---

            // Group by cluster on owned data
            let mut cluster_map: HashMap<String, Vec<&Instance>> = HashMap::new();
            for inst in &snapshot {
                cluster_map
                    .entry(inst.cluster_name.clone())
                    .or_default()
                    .push(inst);
            }

            cluster_map
                .into_iter()
                .map(|(cluster_name, cluster_instances)| {
                    ClusterStatistics::from_instance_refs(&cluster_name, &cluster_instances)
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get single cluster statistics
    pub fn get_single_cluster_statistics(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Option<ClusterStatistics> {
        let stats = self.get_cluster_statistics(namespace, group_name, service_name);
        stats.into_iter().find(|s| s.cluster_name == cluster_name)
    }
}
