use std::sync::Arc;

use crate::persistence::shared::{StoredNamespace, StoredItem};
use crate::persistence::traits::{ApolloPersistenceService, NamespacePersistence, ItemPersistence};
use chrono::Utc;

pub struct ConfigSyncService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ConfigSyncService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

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
        let source_ns = self.persistence.get_by_app_cluster(source_app_id, source_cluster, source_namespace).await?
            .ok_or_else(|| anyhow::anyhow!("Source namespace not found"))?;

        let target_ns = self.persistence.get_by_app_cluster(target_app_id, target_cluster, target_namespace).await?;

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

    pub async fn get_sync_status(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<SyncStatus, anyhow::Error> {
        let target_ns = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?;

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
pub struct SyncResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NamespaceSyncResult {
    pub namespace_name: String,
    pub success: bool,
    pub error: Option<String>,
    pub result: Option<SyncResult>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncStatus {
    pub exists: bool,
    pub item_count: u64,
    pub last_modified_time: Option<String>,
}

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.f+00:00")
        .to_string()
}