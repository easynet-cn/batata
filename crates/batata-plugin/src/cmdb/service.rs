//! CMDB Plugin Service Implementation
//!
//! Provides:
//! - CMDB Plugin SPI (PLG-201)
//! - Label sync (PLG-202)
//! - Entity mapping (PLG-203)

use async_trait::async_trait;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use super::model::*;
use crate::Plugin;

/// CMDB Plugin SPI (PLG-201)
#[async_trait]
pub trait CmdbPlugin: Plugin {
    /// Register an entity with CMDB
    async fn register_entity(&self, entity: CmdbEntity) -> anyhow::Result<String>;

    /// Update an entity in CMDB
    async fn update_entity(&self, entity: CmdbEntity) -> anyhow::Result<()>;

    /// Delete an entity from CMDB
    async fn delete_entity(&self, id: &str) -> anyhow::Result<bool>;

    /// Get an entity by ID
    async fn get_entity(&self, id: &str) -> anyhow::Result<Option<CmdbEntity>>;

    /// List entities by type
    async fn list_entities(
        &self,
        entity_type: Option<CmdbEntityType>,
    ) -> anyhow::Result<Vec<CmdbEntity>>;

    /// Search entities by labels
    async fn search_by_labels(
        &self,
        labels: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<CmdbEntity>>;

    /// Sync labels from source to target (PLG-202)
    async fn sync_labels(
        &self,
        entity_id: &str,
        labels: &HashMap<String, String>,
    ) -> anyhow::Result<HashMap<String, String>>;

    /// Map entity to CMDB format (PLG-203)
    async fn map_entity(&self, entity: &CmdbEntity) -> anyhow::Result<serde_json::Value>;

    /// Perform full sync
    async fn full_sync(&self) -> anyhow::Result<CmdbSyncResult>;

    /// Get label mappings
    async fn get_label_mappings(&self) -> anyhow::Result<Vec<LabelMapping>>;

    /// Add a label mapping
    async fn add_label_mapping(&self, mapping: LabelMapping) -> anyhow::Result<String>;

    /// Remove a label mapping
    async fn remove_label_mapping(&self, id: &str) -> anyhow::Result<bool>;

    /// Get CMDB statistics
    async fn get_stats(&self) -> CmdbStats;
}

/// Default CMDB plugin implementation
pub struct DefaultCmdbPlugin {
    config: CmdbConfig,
    /// Entity storage
    entities: DashMap<String, CmdbEntity>,
    /// Label mappings
    label_mappings: RwLock<Vec<LabelMapping>>,
    /// Entity index by type
    type_index: DashMap<String, Vec<String>>,
    /// Label index
    label_index: DashMap<String, Vec<String>>,
    /// Statistics
    total_syncs: AtomicU64,
    successful_syncs: AtomicU64,
    failed_syncs: AtomicU64,
    last_sync_result: RwLock<Option<CmdbSyncResult>>,
}

impl DefaultCmdbPlugin {
    pub fn new(config: CmdbConfig) -> Self {
        Self {
            config,
            entities: DashMap::new(),
            label_mappings: RwLock::new(Vec::new()),
            type_index: DashMap::new(),
            label_index: DashMap::new(),
            total_syncs: AtomicU64::new(0),
            successful_syncs: AtomicU64::new(0),
            failed_syncs: AtomicU64::new(0),
            last_sync_result: RwLock::new(None),
        }
    }

    fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    fn add_to_type_index(&self, entity_type: &CmdbEntityType, entity_id: &str) {
        let type_key = entity_type.to_string();
        self.type_index
            .entry(type_key)
            .or_default()
            .push(entity_id.to_string());
    }

    fn remove_from_type_index(&self, entity_type: &CmdbEntityType, entity_id: &str) {
        let type_key = entity_type.to_string();
        if let Some(mut ids) = self.type_index.get_mut(&type_key) {
            ids.retain(|id| id != entity_id);
        }
    }

    fn add_to_label_index(&self, labels: &HashMap<String, String>, entity_id: &str) {
        for (key, value) in labels {
            let label_key = format!("{}={}", key, value);
            self.label_index
                .entry(label_key)
                .or_default()
                .push(entity_id.to_string());
        }
    }

    fn remove_from_label_index(&self, labels: &HashMap<String, String>, entity_id: &str) {
        for (key, value) in labels {
            let label_key = format!("{}={}", key, value);
            if let Some(mut ids) = self.label_index.get_mut(&label_key) {
                ids.retain(|id| id != entity_id);
            }
        }
    }

    async fn apply_label_mappings(
        &self,
        labels: &HashMap<String, String>,
        entity_type: &CmdbEntityType,
    ) -> HashMap<String, String> {
        let mappings = self.label_mappings.read().await;
        let mut result = HashMap::new();

        for mapping in mappings.iter() {
            if !mapping.enabled {
                continue;
            }

            // Check if mapping applies to this entity type
            if !mapping.entity_types.is_empty() && !mapping.entity_types.contains(entity_type) {
                continue;
            }

            // Get source value
            let source_value = labels
                .get(&mapping.source_key)
                .cloned()
                .or_else(|| mapping.default_value.clone());

            if let Some(value) = source_value {
                let transformed = mapping.transform.apply(&value);
                result.insert(mapping.target_key.clone(), transformed);
            }
        }

        // Include unmapped labels
        for (key, value) in labels {
            if !result.contains_key(key) {
                result.insert(key.clone(), value.clone());
            }
        }

        result
    }
}

impl Default for DefaultCmdbPlugin {
    fn default() -> Self {
        Self::new(CmdbConfig::default())
    }
}

#[async_trait]
impl Plugin for DefaultCmdbPlugin {
    fn name(&self) -> &str {
        "cmdb"
    }

    async fn init(&self) -> anyhow::Result<()> {
        // Load label mappings from config
        let mut mappings = self.label_mappings.write().await;
        *mappings = self.config.label_mappings.clone();

        tracing::info!(
            "CMDB plugin initialized with {} label mappings",
            mappings.len()
        );
        Ok(())
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        tracing::info!("CMDB plugin shutdown");
        Ok(())
    }
}

#[async_trait]
impl CmdbPlugin for DefaultCmdbPlugin {
    async fn register_entity(&self, mut entity: CmdbEntity) -> anyhow::Result<String> {
        if entity.id.is_empty() {
            entity.generate_id();
        }

        entity.last_sync = Self::current_timestamp();
        let id = entity.id.clone();

        self.add_to_type_index(&entity.entity_type, &id);
        self.add_to_label_index(&entity.labels, &id);
        self.entities.insert(id.clone(), entity);

        tracing::debug!("CMDB entity registered: {}", id);
        Ok(id)
    }

    async fn update_entity(&self, mut entity: CmdbEntity) -> anyhow::Result<()> {
        if let Some(old_entity) = self.entities.get(&entity.id) {
            self.remove_from_label_index(&old_entity.labels, &entity.id);
        }

        entity.last_sync = Self::current_timestamp();
        self.add_to_label_index(&entity.labels, &entity.id);
        self.entities.insert(entity.id.clone(), entity);

        Ok(())
    }

    async fn delete_entity(&self, id: &str) -> anyhow::Result<bool> {
        if let Some((_, entity)) = self.entities.remove(id) {
            self.remove_from_type_index(&entity.entity_type, id);
            self.remove_from_label_index(&entity.labels, id);
            return Ok(true);
        }
        Ok(false)
    }

    async fn get_entity(&self, id: &str) -> anyhow::Result<Option<CmdbEntity>> {
        Ok(self.entities.get(id).map(|e| e.clone()))
    }

    async fn list_entities(
        &self,
        entity_type: Option<CmdbEntityType>,
    ) -> anyhow::Result<Vec<CmdbEntity>> {
        match entity_type {
            Some(et) => {
                let type_key = et.to_string();
                let ids = self.type_index.get(&type_key);

                match ids {
                    Some(ids) => {
                        let entities: Vec<_> = ids
                            .iter()
                            .filter_map(|id| self.entities.get(id).map(|e| e.clone()))
                            .collect();
                        Ok(entities)
                    }
                    None => Ok(Vec::new()),
                }
            }
            None => Ok(self.entities.iter().map(|e| e.clone()).collect()),
        }
    }

    async fn search_by_labels(
        &self,
        labels: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<CmdbEntity>> {
        if labels.is_empty() {
            return Ok(Vec::new());
        }

        // Get candidate IDs from first label
        let first_label = labels.iter().next().unwrap();
        let label_key = format!("{}={}", first_label.0, first_label.1);

        let candidate_ids = match self.label_index.get(&label_key) {
            Some(ids) => ids.clone(),
            None => return Ok(Vec::new()),
        };

        // Filter by remaining labels
        let mut results = Vec::new();
        for id in candidate_ids {
            if let Some(entity) = self.entities.get(&id) {
                let matches_all = labels
                    .iter()
                    .all(|(k, v)| entity.labels.get(k).map(|ev| ev == v).unwrap_or(false));

                if matches_all {
                    results.push(entity.clone());
                }
            }
        }

        Ok(results)
    }

    async fn sync_labels(
        &self,
        entity_id: &str,
        labels: &HashMap<String, String>,
    ) -> anyhow::Result<HashMap<String, String>> {
        let entity = self.entities.get(entity_id);
        let entity_type = entity
            .as_ref()
            .map(|e| e.entity_type.clone())
            .unwrap_or_default();

        let mapped_labels = self.apply_label_mappings(labels, &entity_type).await;

        // Update entity labels if it exists
        if let Some(mut entity) = self.entities.get_mut(entity_id) {
            self.remove_from_label_index(&entity.labels, entity_id);
            entity.labels = mapped_labels.clone();
            self.add_to_label_index(&entity.labels, entity_id);
            entity.last_sync = Self::current_timestamp();
        }

        Ok(mapped_labels)
    }

    async fn map_entity(&self, entity: &CmdbEntity) -> anyhow::Result<serde_json::Value> {
        // Apply label mappings
        let mapped_labels = self
            .apply_label_mappings(&entity.labels, &entity.entity_type)
            .await;

        // Create CMDB-formatted entity
        let cmdb_entity = serde_json::json!({
            "id": entity.id,
            "external_id": entity.external_id,
            "type": entity.entity_type.to_string(),
            "name": entity.name,
            "namespace": entity.namespace,
            "group": entity.group,
            "labels": mapped_labels,
            "attributes": entity.attributes,
            "status": entity.status,
            "parent": entity.parent_id,
            "children": entity.children,
            "last_sync": entity.last_sync,
        });

        Ok(cmdb_entity)
    }

    async fn full_sync(&self) -> anyhow::Result<CmdbSyncResult> {
        let start = Instant::now();
        self.total_syncs.fetch_add(1, Ordering::Relaxed);

        let mut result = CmdbSyncResult {
            success: true,
            timestamp: Self::current_timestamp(),
            ..Default::default()
        };

        // Get all entities matching configured types
        let entities: Vec<_> = self
            .entities
            .iter()
            .filter(|e| {
                self.config.entity_types.is_empty()
                    || self.config.entity_types.contains(&e.entity_type)
            })
            .filter(|e| {
                self.config.namespaces.is_empty() || self.config.namespaces.contains(&e.namespace)
            })
            .map(|e| e.clone())
            .collect();

        for mut entity in entities {
            // Apply label mappings
            match self.sync_labels(&entity.id, &entity.labels).await {
                Ok(mapped) => {
                    entity.labels = mapped;
                    entity.last_sync = Self::current_timestamp();
                    self.entities.insert(entity.id.clone(), entity);
                    result.synced += 1;
                    result.updated += 1;
                }
                Err(e) => {
                    result.errors += 1;
                    result
                        .error_messages
                        .push(format!("Failed to sync {}: {}", entity.id, e));
                }
            }
        }

        result.duration_ms = start.elapsed().as_millis() as u64;
        result.success = result.errors == 0;

        if result.success {
            self.successful_syncs.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_syncs.fetch_add(1, Ordering::Relaxed);
        }

        *self.last_sync_result.write().await = Some(result.clone());

        Ok(result)
    }

    async fn get_label_mappings(&self) -> anyhow::Result<Vec<LabelMapping>> {
        Ok(self.label_mappings.read().await.clone())
    }

    async fn add_label_mapping(&self, mut mapping: LabelMapping) -> anyhow::Result<String> {
        if mapping.id.is_empty() {
            mapping.id = uuid::Uuid::new_v4().to_string();
        }

        let id = mapping.id.clone();
        self.label_mappings.write().await.push(mapping);

        Ok(id)
    }

    async fn remove_label_mapping(&self, id: &str) -> anyhow::Result<bool> {
        let mut mappings = self.label_mappings.write().await;
        let len_before = mappings.len();
        mappings.retain(|m| m.id != id);
        Ok(mappings.len() < len_before)
    }

    async fn get_stats(&self) -> CmdbStats {
        let mut entities_by_type = HashMap::new();
        for item in self.type_index.iter() {
            entities_by_type.insert(item.key().clone(), item.value().len() as u32);
        }

        CmdbStats {
            total_entities: self.entities.len() as u32,
            entities_by_type,
            total_syncs: self.total_syncs.load(Ordering::Relaxed),
            successful_syncs: self.successful_syncs.load(Ordering::Relaxed),
            failed_syncs: self.failed_syncs.load(Ordering::Relaxed),
            last_sync: self
                .last_sync_result
                .read()
                .await
                .as_ref()
                .map(|r| r.timestamp)
                .unwrap_or(0),
            last_sync_result: self.last_sync_result.read().await.clone(),
        }
    }
}

/// Start background sync task
pub fn start_sync_task(
    plugin: Arc<dyn CmdbPlugin>,
    interval_seconds: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
        loop {
            interval.tick().await;
            match plugin.full_sync().await {
                Ok(result) => {
                    tracing::info!(
                        "CMDB sync completed: {} synced, {} errors",
                        result.synced,
                        result.errors
                    );
                }
                Err(e) => {
                    tracing::error!("CMDB sync failed: {}", e);
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_entity_registration() {
        let plugin = DefaultCmdbPlugin::default();

        let entity = CmdbEntity::new(CmdbEntityType::Service, "my-service")
            .with_namespace("production")
            .with_group("api")
            .with_label("env", "prod")
            .with_label("team", "platform");

        let id = plugin.register_entity(entity).await.unwrap();
        assert!(!id.is_empty());

        let retrieved = plugin.get_entity(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "my-service");
    }

    #[tokio::test]
    async fn test_label_sync() {
        let plugin = DefaultCmdbPlugin::new(CmdbConfig::default());

        // Add label mapping
        let mapping = LabelMapping {
            id: "map1".to_string(),
            source_key: "env".to_string(),
            target_key: "environment".to_string(),
            transform: LabelTransform::Uppercase,
            ..Default::default()
        };
        plugin.add_label_mapping(mapping).await.unwrap();

        // Create entity
        let entity = CmdbEntity::new(CmdbEntityType::Service, "test-svc").with_label("env", "prod");
        let id = plugin.register_entity(entity).await.unwrap();

        // Sync labels
        let mut labels = HashMap::new();
        labels.insert("env".to_string(), "prod".to_string());

        let mapped = plugin.sync_labels(&id, &labels).await.unwrap();

        assert_eq!(mapped.get("environment"), Some(&"PROD".to_string()));
    }

    #[tokio::test]
    async fn test_search_by_labels() {
        let plugin = DefaultCmdbPlugin::default();

        // Create entities
        let e1 = CmdbEntity::new(CmdbEntityType::Service, "svc1")
            .with_label("env", "prod")
            .with_label("team", "a");
        let e2 = CmdbEntity::new(CmdbEntityType::Service, "svc2")
            .with_label("env", "prod")
            .with_label("team", "b");
        let e3 = CmdbEntity::new(CmdbEntityType::Service, "svc3").with_label("env", "staging");

        plugin.register_entity(e1).await.unwrap();
        plugin.register_entity(e2).await.unwrap();
        plugin.register_entity(e3).await.unwrap();

        // Search by env=prod
        let mut search_labels = HashMap::new();
        search_labels.insert("env".to_string(), "prod".to_string());

        let results = plugin.search_by_labels(&search_labels).await.unwrap();
        assert_eq!(results.len(), 2);

        // Search by env=prod AND team=a
        search_labels.insert("team".to_string(), "a".to_string());
        let results = plugin.search_by_labels(&search_labels).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "svc1");
    }

    #[test]
    fn test_label_transform() {
        assert_eq!(LabelTransform::None.apply("test"), "test");
        assert_eq!(LabelTransform::Lowercase.apply("TEST"), "test");
        assert_eq!(LabelTransform::Uppercase.apply("test"), "TEST");
        assert_eq!(
            LabelTransform::Prefix("pre_".to_string()).apply("test"),
            "pre_test"
        );
        assert_eq!(
            LabelTransform::Suffix("_suf".to_string()).apply("test"),
            "test_suf"
        );
        assert_eq!(
            LabelTransform::Replace {
                from: "-".to_string(),
                to: "_".to_string()
            }
            .apply("a-b-c"),
            "a_b_c"
        );

        let mut map = HashMap::new();
        map.insert("prod".to_string(), "production".to_string());
        assert_eq!(LabelTransform::Map(map).apply("prod"), "production");
    }
}
