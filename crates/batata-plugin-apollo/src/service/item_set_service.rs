use std::sync::Arc;

use crate::api::dto::ItemChangeSets;
use crate::persistence::shared::{StoredItem, StoredCommit};
use crate::persistence::traits::{ApolloPersistenceService, ItemPersistence, CommitPersistence, NamespacePersistence};
use chrono::Utc;
use serde_json::json;

pub struct ItemSetService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ItemSetService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn update_set(&self, app_id: &str, cluster_name: &str, namespace_name: &str, change_sets: ItemChangeSets) -> Result<(), anyhow::Error> {
        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name))?;

        let now = Utc::now().timestamp_millis();
        let operator = change_sets.create_items.first()
            .or(change_sets.update_items.first())
            .or(change_sets.delete_items.first())
            .and_then(|item| item.data_change_created_by.clone())
            .unwrap_or_else(|| "admin".to_string());

        let mut change_records = Vec::new();

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

            <dyn ItemPersistence>::create(&self.persistence, stored).await?;

            change_records.push(json!({
                "field": "item",
                "oldValue": "",
                "newValue": format!("{}={}", item.key, item.value),
                "changeType": "ADD"
            }));
        }

        for item in change_sets.update_items {
            let stored = self.persistence.get_by_key(namespace.id, &item.key).await?;

            if let Some(mut stored_item) = stored {
                let old_value = stored_item.value.clone();

                stored_item.r#type = item.r#type.unwrap_or(stored_item.r#type);
                stored_item.value = item.value.clone();
                stored_item.comment = item.comment.or(stored_item.comment);
                stored_item.line_num = item.line_num.unwrap_or(stored_item.line_num);
                stored_item.data_change_last_modified_by = Some(operator.clone());
                stored_item.data_change_last_time = Some(now);

                <dyn ItemPersistence>::update(&self.persistence, stored_item).await?;

                change_records.push(json!({
                    "field": "item",
                    "oldValue": format!("{}={}", item.key, old_value),
                    "newValue": format!("{}={}", item.key, item.value),
                    "changeType": "MODIFY"
                }));
            }
        }

        for item in change_sets.delete_items {
            let stored = self.persistence.get_by_key(namespace.id, &item.key).await?;

            if let Some(stored_item) = stored {
                let old_value = stored_item.value.clone();

                <dyn ItemPersistence>::delete(&self.persistence, stored_item.id).await?;

                change_records.push(json!({
                    "field": "item",
                    "oldValue": format!("{}={}", item.key, old_value),
                    "newValue": "",
                    "changeType": "DELETE"
                }));
            }
        }

        if !change_records.is_empty() {
            let change_sets_json = serde_json::to_string(&change_records)?;

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