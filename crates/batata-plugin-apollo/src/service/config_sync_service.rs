use std::sync::Arc;

use crate::api::dto::{ItemChangeSets, ItemDiffs, ItemDTO};
use crate::persistence::shared::{StoredNamespace, StoredItem};
use crate::persistence::traits::{ApolloPersistenceService, NamespacePersistence, ItemPersistence};
use chrono::Utc;

/// Represents the `ConfigSyncService` entity.
pub struct ConfigSyncService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ConfigSyncService {
    /// Creates a new `ConfigSyncService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Performs the `sync_configs` operation.
    pub async fn sync_configs(
        &self,
        source_app_id: &str,
        source_cluster: &str,
        source_namespace: &str,
        target_app_id: &str,
        target_cluster: &str,
        target_namespace: &str,
        operator: &str,
        overwrite: bool,
    ) -> Result<SyncResult, anyhow::Error> {
        let source_ns = <dyn NamespacePersistence>::get_by_app_cluster(&self.persistence,source_app_id, source_cluster, source_namespace).await?
            .ok_or_else(|| anyhow::anyhow!("Source namespace not found"))?;

        let target_ns = <dyn NamespacePersistence>::get_by_app_cluster(&self.persistence,target_app_id, target_cluster, target_namespace).await?;

        let target_ns_id = if let Some(ns) = target_ns {
            ns.id
        } else {
            let now = Utc::now().timestamp_millis();
            let stored = StoredNamespace {
                id: 0,
                app_id: target_app_id.to_string(),
                cluster_name: target_cluster.to_string(),
                namespace_name: target_namespace.to_string(),
                format: source_ns.format,
                is_public: source_ns.is_public,
                comment: source_ns.comment,
                is_deleted: false,
                deleted_at: 0,
                data_change_created_by: operator.to_string(),
                data_change_created_time: now,
                data_change_last_modified_by: None,
                data_change_last_time: None,
            };
            let created = <dyn NamespacePersistence>::create(&self.persistence, stored).await?;
            created.id
        };

        let source_items = <dyn ItemPersistence>::list_by_namespace(&self.persistence, source_ns.id).await?;

        let total = source_items.len();
        let mut created = 0;
        let mut updated = 0;
        let mut skipped = 0;

        let now = Utc::now().timestamp_millis();

        for item in source_items {
            let existing = <dyn ItemPersistence>::get_by_key(&self.persistence, target_ns_id, &item.key).await?;

            if let Some(mut existing_item) = existing {
                if !overwrite {
                    skipped += 1;
                    continue;
                }

                existing_item.value = item.value.clone();
                existing_item.r#type = item.r#type;
                existing_item.comment = item.comment.clone();
                existing_item.line_num = item.line_num;
                existing_item.data_change_last_modified_by = Some(operator.to_string());
                existing_item.data_change_last_time = Some(now);

                <dyn ItemPersistence>::update(&self.persistence, existing_item).await?;
                updated += 1;
            } else {
                let stored = StoredItem {
                    id: 0,
                    namespace_id: target_ns_id,
                    key: item.key.clone(),
                    r#type: item.r#type,
                    value: item.value.clone(),
                    comment: item.comment.clone(),
                    line_num: item.line_num,
                    is_deleted: false,
                    deleted_at: 0,
                    data_change_created_by: operator.to_string(),
                    data_change_created_time: now,
                    data_change_last_modified_by: Some(operator.to_string()),
                    data_change_last_time: Some(now),
                };
                <dyn ItemPersistence>::create(&self.persistence, stored).await?;
                created += 1;
            }
        }

        Ok(SyncResult {
            created,
            updated,
            skipped,
            total,
            message: format!("Sync completed: {} created, {} updated, {} skipped", created, updated, skipped),
        })
    }

    /// Performs the `sync_app_all_namespaces` operation.
    pub async fn sync_app_all_namespaces(
        &self,
        source_app_id: &str,
        source_cluster: &str,
        target_app_id: &str,
        target_cluster: &str,
        operator: &str,
        overwrite: bool,
    ) -> Result<Vec<NamespaceSyncResult>, anyhow::Error> {
        let source_ns_list = self.persistence.list_by_app(source_app_id).await?;
        let source_ns_filtered: Vec<_> = source_ns_list.into_iter()
            .filter(|s| s.cluster_name == source_cluster && !s.is_deleted)
            .collect();

        let mut results = Vec::new();

        for ns in source_ns_filtered {
            let result = self.sync_configs(
                source_app_id,
                source_cluster,
                &ns.namespace_name,
                target_app_id,
                target_cluster,
                &ns.namespace_name,
                operator,
                overwrite,
            ).await;

            results.push(NamespaceSyncResult {
                namespace_name: ns.namespace_name,
                success: result.is_ok(),
                error: result.as_ref().err().map(|e| e.to_string()),
                result: result.ok(),
            });
        }

        Ok(results)
    }

    /// `items/diff` — compares the source change sets against each target
    /// namespace's current items, returning one `ItemDiffs` (create / update /
    /// delete) per target namespace.
    ///
    /// Upstream semantics (`portal/controller/ItemController.java:180-203`):
    /// `configService.compare(syncToNamespaces, syncItems)`. For each target
    /// namespace we diff `syncItems` (the proposed create/update/delete item
    /// lists) against the items currently stored in that namespace.
    pub async fn compare(
        &self,
        source_app_id: &str,
        source_cluster: &str,
        source_namespace: &str,
        sync_to_namespaces: &[String],
        sync_items: &ItemChangeSets,
    ) -> Result<Vec<ItemDiffs>, anyhow::Error> {
        let _source_ns = <dyn NamespacePersistence>::get_by_app_cluster(
            &self.persistence,
            source_app_id,
            source_cluster,
            source_namespace,
        )
        .await?
        .ok_or_else(|| anyhow::anyhow!("Source namespace not found"))?;

        // Index the target item keys for fast lookup.
        let mut result = Vec::with_capacity(sync_to_namespaces.len());
        for target_ns_name in sync_to_namespaces {
            let target_ns = self
                .persistence
                .get_by_app_cluster(source_app_id, source_cluster, target_ns_name)
                .await?;
            let target_items = match &target_ns {
                Some(ns) => <dyn ItemPersistence>::list_by_namespace(&self.persistence, ns.id).await?,
                None => Vec::new(),
            };
            let target_map: std::collections::HashMap<String, StoredItem> = target_items
                .into_iter()
                .map(|i| (i.key.clone(), i))
                .collect();

            let mut create_items = Vec::new();
            let mut update_items = Vec::new();
            let mut delete_items = Vec::new();

            for proposed in &sync_items.create_items {
                if target_map.contains_key(&proposed.key) {
                    update_items.push(proposed.clone());
                } else {
                    create_items.push(proposed.clone());
                }
            }
            for proposed in &sync_items.update_items {
                if target_map.contains_key(&proposed.key) {
                    update_items.push(proposed.clone());
                } else {
                    create_items.push(proposed.clone());
                }
            }
            for proposed in &sync_items.delete_items {
                if target_map.contains_key(&proposed.key) {
                    delete_items.push(proposed.clone());
                }
            }

            result.push(ItemDiffs {
                namespace_name: target_ns_name.clone(),
                create_items,
                update_items,
                delete_items,
            });
        }
        Ok(result)
    }

    /// `PUT /apps/{appId}/namespaces/{namespaceName}/items` — synchronizes the
    /// source change sets into every target namespace.
    ///
    /// Upstream semantics (`portal/controller/ItemController.java:205-230`):
    /// `configService.synchronizeConfigToCluster(...)` applies `syncItems`
    /// (create/update/delete) to each target namespace, creating items that do
    /// not exist and updating/deleting those that do. Returns the resulting
    /// item lists per target namespace.
    pub async fn synchronize(
        &self,
        source_app_id: &str,
        source_cluster: &str,
        source_namespace: &str,
        sync_to_namespaces: &[String],
        sync_items: &ItemChangeSets,
        operator: &str,
    ) -> Result<Vec<Vec<ItemDTO>>, anyhow::Error> {
        let _ = source_namespace;
        let now = Utc::now().timestamp_millis();
        let mut results = Vec::with_capacity(sync_to_namespaces.len());

        // Flatten the proposed change set into a single keyed map of desired items.
        let mut desired: std::collections::BTreeMap<String, &ItemDTO> = std::collections::BTreeMap::new();
        for it in &sync_items.create_items {
            desired.insert(it.key.clone(), it);
        }
        for it in &sync_items.update_items {
            desired.insert(it.key.clone(), it);
        }
        // Collect keys that are to be deleted (removed from desired).
        let delete_keys: Vec<String> = sync_items.delete_items.iter().map(|d| d.key.clone()).collect();
        for k in &delete_keys {
            desired.remove(k);
        }

        for target_ns_name in sync_to_namespaces {
            let target_ns = <dyn NamespacePersistence>::get_by_app_cluster(
                &self.persistence,
                source_app_id,
                source_cluster,
                target_ns_name,
            )
            .await?;
            let target_ns_id = if let Some(ns) = target_ns {
                ns.id
            } else {
                let stored = StoredNamespace {
                    id: 0,
                    app_id: source_app_id.to_string(),
                    cluster_name: source_cluster.to_string(),
                    namespace_name: target_ns_name.clone(),
                    format: "properties".to_string(),
                    is_public: false,
                    comment: Some(String::new()),
                    is_deleted: false,
                    deleted_at: 0,
                    data_change_created_by: operator.to_string(),
                    data_change_created_time: now,
                    data_change_last_modified_by: None,
                    data_change_last_time: None,
                };
                let created = <dyn NamespacePersistence>::create(&self.persistence, stored).await?;
                created.id
            };

            let existing_items = <dyn ItemPersistence>::list_by_namespace(&self.persistence, target_ns_id).await?;
            let existing_map: std::collections::HashMap<String, StoredItem> =
                existing_items.into_iter().map(|i| (i.key.clone(), i)).collect();

            // Create / update desired items.
            for it in desired.values() {
                let stored = StoredItem {
                    id: 0,
                    namespace_id: target_ns_id,
                    key: it.key.clone(),
                    r#type: it.r#type.unwrap_or(0),
                    value: it.value.clone(),
                    comment: it.comment.clone(),
                    line_num: it.line_num.unwrap_or(0),
                    is_deleted: false,
                    deleted_at: 0,
                    data_change_created_by: operator.to_string(),
                    data_change_created_time: now,
                    data_change_last_modified_by: Some(operator.to_string()),
                    data_change_last_time: Some(now),
                };
                if existing_map.contains_key(&it.key) {
                    let mut existing = existing_map.get(&it.key).unwrap().clone();
                    existing.value = stored.value;
                    existing.r#type = stored.r#type;
                    existing.comment = stored.comment;
                    existing.line_num = stored.line_num;
                    existing.data_change_last_modified_by = Some(operator.to_string());
                    existing.data_change_last_time = Some(now);
                    <dyn ItemPersistence>::update(&self.persistence, existing).await?;
                } else {
                    <dyn ItemPersistence>::create(&self.persistence, stored).await?;
                }
            }

            // Delete targeted keys.
            for k in &delete_keys {
                if let Some(mut existing) = existing_map.get(k).cloned() {
                    existing.is_deleted = true;
                    existing.deleted_at = now;
                    existing.data_change_last_modified_by = Some(operator.to_string());
                    existing.data_change_last_time = Some(now);
                    <dyn ItemPersistence>::update(&self.persistence, existing).await?;
                }
            }

            // Return the resulting item list (excluding soft-deleted).
            let final_items = <dyn ItemPersistence>::list_by_namespace(&self.persistence, target_ns_id).await?;
            let dtos: Vec<ItemDTO> = final_items
                .into_iter()
                .filter(|i| !i.is_deleted)
                .map(|i| ItemDTO {
                    id: Some(i.id),
                    key: i.key,
                    value: i.value,
                    r#type: Some(i.r#type),
                    comment: i.comment,
                    line_num: Some(i.line_num),
                    data_change_created_by: Some(i.data_change_created_by),
                    data_change_last_modified_by: i.data_change_last_modified_by,
                    data_change_created_time: Some(format_timestamp(i.data_change_created_time)),
                    data_change_last_time: i.data_change_last_time.map(format_timestamp),
                })
                .collect();
            results.push(dtos);
        }
        Ok(results)
    }

    /// Returns the requested value.
    pub async fn get_sync_status(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<SyncStatus, anyhow::Error> {
        let target_ns = <dyn NamespacePersistence>::get_by_app_cluster(&self.persistence,app_id, cluster_name, namespace_name).await?;

        if target_ns.is_none() {
            return Ok(SyncStatus {
                exists: false,
                item_count: 0,
                last_modified_time: None,
            });
        }

        let ns = target_ns.unwrap();
        let items = <dyn ItemPersistence>::list_by_namespace(&self.persistence, ns.id).await?;
        let item_count = items.len() as u64;

        Ok(SyncStatus {
            exists: true,
            item_count,
            last_modified_time: ns.data_change_last_time.map(format_timestamp),
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
/// Represents the `SyncResult` entity.
pub struct SyncResult {
    /// The `created` field.
    pub created: usize,
    /// The `updated` field.
    pub updated: usize,
    /// The `skipped` field.
    pub skipped: usize,
    /// The `total` field.
    pub total: usize,
    /// The `message` field.
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
/// Represents the `NamespaceSyncResult` entity.
pub struct NamespaceSyncResult {
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `success` field.
    pub success: bool,
    /// The `error` field.
    pub error: Option<String>,
    /// The `result` field.
    pub result: Option<SyncResult>,
}

#[derive(Debug, Clone, serde::Serialize)]
/// Represents the `SyncStatus` entity.
pub struct SyncStatus {
    /// The `exists` field.
    pub exists: bool,
    /// The `item_count` field.
    pub item_count: u64,
    /// The `last_modified_time` field.
    pub last_modified_time: Option<String>,
}

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.f+00:00")
        .to_string()
}