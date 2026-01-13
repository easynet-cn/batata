//! Persistent naming service for database-backed service instances
//!
//! This module provides persistence support for service instances that survive
//! server restarts. Persistent instances are stored in the database and loaded
//! on startup.

use std::collections::HashMap;

use anyhow::Result;
use sea_orm::{
    prelude::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use tracing::{info, warn};

use batata_persistence::entity::{cluster_info, instance_info, service_info};

use crate::model::Instance;

/// Persistent naming service that stores instances in the database
pub struct PersistentNamingService {
    db: DatabaseConnection,
}

impl PersistentNamingService {
    /// Create a new persistent naming service
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Load all persistent instances from the database
    pub async fn load_persistent_instances(
        &self,
    ) -> Result<HashMap<String, HashMap<String, Instance>>> {
        let mut result: HashMap<String, HashMap<String, Instance>> = HashMap::new();

        // Load all services with their instances
        let services = service_info::Entity::find()
            .order_by_asc(service_info::Column::Id)
            .all(&self.db)
            .await?;

        for service in services {
            let service_key = format!(
                "{}@@{}@@{}",
                service.namespace_id, service.group_name, service.service_name
            );

            // Load instances for this service (only persistent ones)
            let instances = instance_info::Entity::find()
                .filter(instance_info::Column::ServiceId.eq(service.id))
                .filter(instance_info::Column::Ephemeral.eq(false))
                .all(&self.db)
                .await?;

            let instance_map: HashMap<String, Instance> = instances
                .into_iter()
                .map(|inst| {
                    let instance_key = format!("{}#{}#{}", inst.ip, inst.port, inst.cluster_name);
                    let instance = Instance {
                        instance_id: inst.instance_id,
                        ip: inst.ip,
                        port: inst.port,
                        weight: inst.weight,
                        healthy: inst.healthy,
                        enabled: inst.enabled,
                        ephemeral: inst.ephemeral,
                        cluster_name: inst.cluster_name,
                        service_name: service.service_name.clone(),
                        metadata: inst
                            .metadata
                            .and_then(|m| serde_json::from_str(&m).ok())
                            .unwrap_or_default(),
                        instance_heart_beat_interval: inst.heartbeat_interval.unwrap_or(5000)
                            as i64,
                        instance_heart_beat_time_out: inst.heartbeat_timeout.unwrap_or(15000)
                            as i64,
                        ip_delete_timeout: inst.ip_delete_timeout.unwrap_or(30000) as i64,
                        instance_id_generator: String::new(),
                    };
                    (instance_key, instance)
                })
                .collect();

            if !instance_map.is_empty() {
                result.insert(service_key, instance_map);
            }
        }

        info!(
            "Loaded {} services with persistent instances from database",
            result.len()
        );
        Ok(result)
    }

    /// Get or create a service, returning the service ID
    async fn get_or_create_service(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> Result<i64> {
        // Try to find existing service
        if let Some(service) = service_info::Entity::find()
            .filter(service_info::Column::NamespaceId.eq(namespace_id))
            .filter(service_info::Column::GroupName.eq(group_name))
            .filter(service_info::Column::ServiceName.eq(service_name))
            .one(&self.db)
            .await?
        {
            return Ok(service.id);
        }

        // Create new service
        let now = chrono::Utc::now().naive_utc();
        let service = service_info::ActiveModel {
            namespace_id: Set(namespace_id.to_string()),
            group_name: Set(group_name.to_string()),
            service_name: Set(service_name.to_string()),
            protect_threshold: Set(0.0),
            metadata: Set(None),
            selector: Set(None),
            enabled: Set(true),
            gmt_create: Set(now),
            gmt_modified: Set(now),
            ..Default::default()
        };

        let result = service.insert(&self.db).await?;
        info!(
            "Created service {}//{}//{} with id {}",
            namespace_id, group_name, service_name, result.id
        );
        Ok(result.id)
    }

    /// Register a persistent instance
    pub async fn register_persistent_instance(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> Result<()> {
        let service_id = self
            .get_or_create_service(namespace_id, group_name, service_name)
            .await?;

        let now = chrono::Utc::now().naive_utc();
        let metadata = if instance.metadata.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&instance.metadata)?)
        };

        // Try to update existing instance
        let existing = instance_info::Entity::find()
            .filter(instance_info::Column::InstanceId.eq(&instance.instance_id))
            .one(&self.db)
            .await?;

        if let Some(existing) = existing {
            // Update existing instance
            let mut active: instance_info::ActiveModel = existing.into();
            active.ip = Set(instance.ip.clone());
            active.port = Set(instance.port);
            active.weight = Set(instance.weight);
            active.healthy = Set(instance.healthy);
            active.enabled = Set(instance.enabled);
            active.ephemeral = Set(instance.ephemeral);
            active.metadata = Set(metadata);
            active.heartbeat_interval = Set(Some(instance.instance_heart_beat_interval as i32));
            active.heartbeat_timeout = Set(Some(instance.instance_heart_beat_time_out as i32));
            active.ip_delete_timeout = Set(Some(instance.ip_delete_timeout as i32));
            active.last_heartbeat = Set(Some(now));
            active.gmt_modified = Set(now);
            active.update(&self.db).await?;
        } else {
            // Insert new instance
            let new_instance = instance_info::ActiveModel {
                instance_id: Set(instance.instance_id.clone()),
                service_id: Set(service_id),
                cluster_name: Set(instance.cluster_name.clone()),
                ip: Set(instance.ip.clone()),
                port: Set(instance.port),
                weight: Set(instance.weight),
                healthy: Set(instance.healthy),
                enabled: Set(instance.enabled),
                ephemeral: Set(instance.ephemeral),
                metadata: Set(metadata),
                heartbeat_interval: Set(Some(instance.instance_heart_beat_interval as i32)),
                heartbeat_timeout: Set(Some(instance.instance_heart_beat_time_out as i32)),
                ip_delete_timeout: Set(Some(instance.ip_delete_timeout as i32)),
                last_heartbeat: Set(Some(now)),
                gmt_create: Set(now),
                gmt_modified: Set(now),
                ..Default::default()
            };
            new_instance.insert(&self.db).await?;
        }

        info!(
            "Registered persistent instance {} for service {}//{}/{}",
            instance.instance_id, namespace_id, group_name, service_name
        );
        Ok(())
    }

    /// Deregister a persistent instance
    pub async fn deregister_persistent_instance(&self, instance_id: &str) -> Result<bool> {
        let result = instance_info::Entity::delete_many()
            .filter(instance_info::Column::InstanceId.eq(instance_id))
            .exec(&self.db)
            .await?;

        if result.rows_affected > 0 {
            info!("Deregistered persistent instance {}", instance_id);
            Ok(true)
        } else {
            warn!("Persistent instance {} not found", instance_id);
            Ok(false)
        }
    }

    /// Update instance health status
    pub async fn update_instance_health(
        &self,
        instance_id: &str,
        healthy: bool,
    ) -> Result<bool> {
        let now = chrono::Utc::now().naive_utc();

        let result = instance_info::Entity::update_many()
            .filter(instance_info::Column::InstanceId.eq(instance_id))
            .col_expr(instance_info::Column::Healthy, Expr::value(healthy))
            .col_expr(instance_info::Column::LastHeartbeat, Expr::value(now))
            .col_expr(instance_info::Column::GmtModified, Expr::value(now))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Update instance heartbeat timestamp
    pub async fn update_instance_heartbeat(&self, instance_id: &str) -> Result<bool> {
        let now = chrono::Utc::now().naive_utc();

        let result = instance_info::Entity::update_many()
            .filter(instance_info::Column::InstanceId.eq(instance_id))
            .col_expr(instance_info::Column::Healthy, Expr::value(true))
            .col_expr(instance_info::Column::LastHeartbeat, Expr::value(now))
            .col_expr(instance_info::Column::GmtModified, Expr::value(now))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Get all persistent instances for a service
    pub async fn get_persistent_instances(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> Result<Vec<Instance>> {
        // Find service
        let service = service_info::Entity::find()
            .filter(service_info::Column::NamespaceId.eq(namespace_id))
            .filter(service_info::Column::GroupName.eq(group_name))
            .filter(service_info::Column::ServiceName.eq(service_name))
            .one(&self.db)
            .await?;

        let Some(service) = service else {
            return Ok(Vec::new());
        };

        // Find instances
        let instances = instance_info::Entity::find()
            .filter(instance_info::Column::ServiceId.eq(service.id))
            .filter(instance_info::Column::Ephemeral.eq(false))
            .all(&self.db)
            .await?;

        Ok(instances
            .into_iter()
            .map(|inst| Instance {
                instance_id: inst.instance_id,
                ip: inst.ip,
                port: inst.port,
                weight: inst.weight,
                healthy: inst.healthy,
                enabled: inst.enabled,
                ephemeral: inst.ephemeral,
                cluster_name: inst.cluster_name,
                service_name: service.service_name.clone(),
                metadata: inst
                    .metadata
                    .and_then(|m| serde_json::from_str(&m).ok())
                    .unwrap_or_default(),
                instance_heart_beat_interval: inst.heartbeat_interval.unwrap_or(5000) as i64,
                instance_heart_beat_time_out: inst.heartbeat_timeout.unwrap_or(15000) as i64,
                ip_delete_timeout: inst.ip_delete_timeout.unwrap_or(30000) as i64,
                instance_id_generator: String::new(),
            })
            .collect())
    }

    /// Clean up stale ephemeral instances (optional - for instances that have timeout)
    pub async fn cleanup_stale_ephemeral_instances(&self, timeout_ms: i64) -> Result<u64> {
        let cutoff = chrono::Utc::now().naive_utc()
            - chrono::Duration::milliseconds(timeout_ms);

        let result = instance_info::Entity::delete_many()
            .filter(instance_info::Column::Ephemeral.eq(true))
            .filter(instance_info::Column::LastHeartbeat.lt(cutoff))
            .exec(&self.db)
            .await?;

        if result.rows_affected > 0 {
            info!(
                "Cleaned up {} stale ephemeral instances",
                result.rows_affected
            );
        }

        Ok(result.rows_affected)
    }

    /// Get or create a cluster
    #[allow(dead_code)]
    pub async fn get_or_create_cluster(
        &self,
        service_id: i64,
        cluster_name: &str,
    ) -> Result<i64> {
        // Try to find existing cluster
        if let Some(cluster) = cluster_info::Entity::find()
            .filter(cluster_info::Column::ServiceId.eq(service_id))
            .filter(cluster_info::Column::ClusterName.eq(cluster_name))
            .one(&self.db)
            .await?
        {
            return Ok(cluster.id);
        }

        // Create new cluster
        let now = chrono::Utc::now().naive_utc();
        let cluster = cluster_info::ActiveModel {
            service_id: Set(service_id),
            cluster_name: Set(cluster_name.to_string()),
            health_check_type: Set(Some("TCP".to_string())),
            health_check_port: Set(Some(0)),
            health_check_path: Set(Some(String::new())),
            use_instance_port: Set(Some(true)),
            metadata: Set(None),
            gmt_create: Set(now),
            gmt_modified: Set(now),
            ..Default::default()
        };

        let result = cluster.insert(&self.db).await?;
        Ok(result.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_instance(ip: &str, port: i32, ephemeral: bool) -> Instance {
        Instance {
            instance_id: format!("{}#{}#DEFAULT", ip, port),
            ip: ip.to_string(),
            port,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral,
            cluster_name: "DEFAULT".to_string(),
            service_name: "test-service".to_string(),
            metadata: HashMap::new(),
            instance_heart_beat_interval: 5000,
            instance_heart_beat_time_out: 15000,
            ip_delete_timeout: 30000,
            instance_id_generator: String::new(),
        }
    }

    #[test]
    fn test_create_persistent_instance() {
        let instance = create_test_instance("127.0.0.1", 8080, false);
        assert!(!instance.ephemeral);
    }

    #[test]
    fn test_create_ephemeral_instance() {
        let instance = create_test_instance("127.0.0.1", 8080, true);
        assert!(instance.ephemeral);
    }
}
