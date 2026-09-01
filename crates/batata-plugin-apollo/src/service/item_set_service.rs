use std::sync::Arc;

use crate::api::dto::{ConfigChangeContent, ItemChangeSets, ItemPair, ItemDTO};
use crate::persistence::shared::{StoredItem, StoredCommit};
use crate::persistence::traits::{ApolloPersistenceService, ItemPersistence, CommitPersistence, NamespacePersistence};
use chrono::Utc;

/// Represents the `ItemSetService` entity.
pub struct ItemSetService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ItemSetService {
    /// Creates a new `ItemSetService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Updates an existing resource.
    pub async fn update_set(&self, app_id: &str, cluster_name: &str, namespace_name: &str, change_sets: ItemChangeSets) -> Result<(), anyhow::Error> {
        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name))?;

        let now = Utc::now().timestamp_millis();
        let operator = change_sets.create_items.first()
            .or(change_sets.update_items.first())
            .or(change_sets.delete_items.first())
            .and_then(|item| item.data_change_created_by.clone())
            .unwrap_or_else(|| "admin".to_string());

        // Upstream `ConfigChangeContentBuilder`: the commit row stores full item
        // snapshots (and old/new pairs for updates), not a flat change log.
        let mut content = ConfigChangeContent::default();

        for item in change_sets.create_items {
            let existing = self.persistence.get_by_key(namespace.id, &item.key).await?;

            if existing.is_some() {
                continue;
            }

            let stored = StoredItem {
                id: 0,
                namespace_id: namespace.id,
                key: item.key.clone(),
                r#type: item.r#type.unwrap_or(0),
                value: item.value.clone(),
                comment: item.comment,
                line_num: item.line_num.unwrap_or(0),
                is_deleted: false,
                deleted_at: 0,
                data_change_created_by: operator.clone(),
                data_change_created_time: now,
                data_change_last_modified_by: Some(operator.clone()),
                data_change_last_time: Some(now),
            };

            let created = <dyn ItemPersistence>::create(&self.persistence, stored).await?;

            content.create_items.push(created.into());
        }

        for item in change_sets.update_items {
            let stored = self.persistence.get_by_key(namespace.id, &item.key).await?;

            if let Some(mut stored_item) = stored {
                // Snapshot the item before applying the mutation so the commit
                // keeps a full before/after record.
                let old_item: ItemDTO = stored_item.clone().into();

                stored_item.r#type = item.r#type.unwrap_or(stored_item.r#type);
                stored_item.value = item.value.clone();
                stored_item.comment = item.comment.or(stored_item.comment);
                stored_item.line_num = item.line_num.unwrap_or(stored_item.line_num);
                stored_item.data_change_last_modified_by = Some(operator.clone());
                stored_item.data_change_last_time = Some(now);

                let updated = <dyn ItemPersistence>::update(&self.persistence, stored_item).await?;

                // Upstream `ConfigChangeContentBuilder.updateItem` only records a
                // pair when the value actually changed.
                if old_item.value != updated.value {
                    content.update_items.push(ItemPair {
                        old_item,
                        new_item: updated.into(),
                    });
                }
            }
        }

        for item in change_sets.delete_items {
            let stored = self.persistence.get_by_key(namespace.id, &item.key).await?;

            if let Some(stored_item) = stored {
                // Snapshot the item before it is removed so the commit keeps a
                // full record of what was deleted.
                let deleted: ItemDTO = stored_item.clone().into();

                <dyn ItemPersistence>::delete(&self.persistence, stored_item.id).await?;

                content.delete_items.push(deleted);
            }
        }

        if content.has_content() {
            let change_sets_json = serde_json::to_string(&content)?;

            let commit_stored = StoredCommit {
                id: 0,
                change_sets: change_sets_json,
                app_id: app_id.to_string(),
                cluster_name: cluster_name.to_string(),
                namespace_name: namespace_name.to_string(),
                comment: None,
                is_deleted: false,
                deleted_at: 0,
                data_change_created_by: operator,
                data_change_created_time: now,
                data_change_last_modified_by: None,
                data_change_last_time: None,
            };

            <dyn CommitPersistence>::create(&self.persistence, commit_stored).await?;
        }

        Ok(())
    }
}