use std::sync::Arc;

use crate::api::dto::{ConfigChangeContent, ItemDTO};
use crate::persistence::shared::StoredItem;
use crate::persistence::traits::{
    ApolloPersistenceService, CommitPersistence, ItemPersistence, NamespacePersistence,
    ReleasePersistence,
};
use chrono::Utc;

/// Represents the `ItemService` entity.
pub struct ItemService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ItemService {
    /// Creates a new `ItemService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Performs the `create` operation.
    pub async fn create(&self, app_id: &str, cluster_name: &str, namespace_name: &str, dto: ItemDTO) -> Result<ItemDTO, anyhow::Error> {
        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name))?;

        let existing = self.persistence.get_by_key(namespace.id, &dto.key).await?;

        if existing.is_some() {
            return Err(anyhow::anyhow!("Item already exists: {}", dto.key));
        }

        let now = Utc::now().timestamp_millis();
        let created_by = dto.data_change_created_by.clone().unwrap_or_default();

        let stored = StoredItem {
            id: 0,
            namespace_id: namespace.id,
            key: dto.key.clone(),
            r#type: dto.r#type.unwrap_or(0),
            value: dto.value.clone(),
            comment: dto.comment,
            line_num: dto.line_num.unwrap_or(0),
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: dto.data_change_last_modified_by,
            data_change_last_time: Some(now),
        };

        let created = ItemPersistence::create(&self.persistence, stored).await?;
        Ok(created.into())
    }

    /// Returns the requested value.
    pub async fn get_by_key(&self, app_id: &str, cluster_name: &str, namespace_name: &str, key: &str) -> Result<Option<ItemDTO>, anyhow::Error> {
        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?;

        if namespace.is_none() {
            return Ok(None);
        }

        let stored = self.persistence.get_by_key(namespace.unwrap().id, key).await?;
        Ok(stored.map(|s| s.into()))
    }

    /// Returns the requested value.
    pub async fn get_by_id(&self, item_id: i64) -> Result<Option<ItemDTO>, anyhow::Error> {
        let stored = <dyn ItemPersistence>::get_by_id(&self.persistence, item_id).await?;
        Ok(stored.map(|s| s.into()))
    }

    /// Performs the `list` operation.
    pub async fn list(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Vec<ItemDTO>, anyhow::Error> {
        let namespace = <dyn NamespacePersistence>::get_by_app_cluster(&self.persistence, app_id, cluster_name, namespace_name).await?;

        if namespace.is_none() {
            return Ok(vec![]);
        }

        let stored_list = <dyn ItemPersistence>::list_by_namespace(&self.persistence, namespace.unwrap().id).await?;
        Ok(stored_list.into_iter().map(|s| s.into()).collect())
    }

    /// Performs the `update` operation.
    pub async fn update(&self, app_id: &str, cluster_name: &str, namespace_name: &str, item_id: i64, dto: ItemDTO) -> Result<ItemDTO, anyhow::Error> {
        let stored = <dyn ItemPersistence>::get_by_id(&self.persistence, item_id).await?
            .ok_or_else(|| anyhow::anyhow!("Item not found: {}", item_id))?;

        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found"))?;

        if namespace.id != stored.namespace_id {
            return Err(anyhow::anyhow!("Namespace not match"));
        }

        let now = Utc::now().timestamp_millis();

        let updated_stored = StoredItem {
            id: stored.id,
            namespace_id: stored.namespace_id,
            key: stored.key,
            r#type: dto.r#type.unwrap_or(stored.r#type),
            value: if dto.value.is_empty() { stored.value } else { dto.value },
            comment: dto.comment.or(stored.comment),
            line_num: stored.line_num,
            is_deleted: stored.is_deleted,
            deleted_at: stored.deleted_at,
            data_change_created_by: stored.data_change_created_by,
            data_change_created_time: stored.data_change_created_time,
            data_change_last_modified_by: dto.data_change_last_modified_by,
            data_change_last_time: Some(now),
        };

        let updated = ItemPersistence::update(&self.persistence, updated_stored).await?;
        Ok(updated.into())
    }

    /// Updates an existing resource.
    pub async fn update_by_key(&self, app_id: &str, cluster_name: &str, namespace_name: &str, key: &str, dto: ItemDTO) -> Result<ItemDTO, anyhow::Error> {
        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name))?;

        let stored = self.persistence.get_by_key(namespace.id, key).await?
            .ok_or_else(|| anyhow::anyhow!("Item not found: {}", key))?;

        let now = Utc::now().timestamp_millis();

        let updated_stored = StoredItem {
            id: stored.id,
            namespace_id: stored.namespace_id,
            key: stored.key,
            r#type: dto.r#type.unwrap_or(stored.r#type),
            value: if dto.value.is_empty() { stored.value } else { dto.value },
            comment: dto.comment.or(stored.comment),
            line_num: stored.line_num,
            is_deleted: stored.is_deleted,
            deleted_at: stored.deleted_at,
            data_change_created_by: stored.data_change_created_by,
            data_change_created_time: stored.data_change_created_time,
            data_change_last_modified_by: dto.data_change_last_modified_by,
            data_change_last_time: Some(now),
        };

        let updated = ItemPersistence::update(&self.persistence, updated_stored).await?;
        Ok(updated.into())
    }

    /// Performs the `delete` operation.
    pub async fn delete(&self, item_id: i64, _operator: &str) -> Result<(), anyhow::Error> {
        ItemPersistence::delete(&self.persistence, item_id).await?;
        Ok(())
    }

    /// Deletes the specified resource.
    pub async fn delete_by_key(&self, app_id: &str, cluster_name: &str, namespace_name: &str, key: &str, _operator: &str) -> Result<(), anyhow::Error> {
        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name))?;

        let stored = self.persistence.get_by_key(namespace.id, key).await?
            .ok_or_else(|| anyhow::anyhow!("Item not found: {}", key))?;

        ItemPersistence::delete(&self.persistence, stored.id).await?;
        Ok(())
    }

    /// List items that have been soft-deleted for the namespace.
    pub async fn list_deleted(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Vec<ItemDTO>, anyhow::Error> {
        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?;
        let namespace_id = match namespace {
            Some(n) => n.id,
            None => return Ok(vec![]),
        };
        let stored = self.persistence.list_deleted_items(namespace_id).await?;
        Ok(stored.into_iter().map(|s| s.into()).collect())
    }

    /// List the items deleted since the latest active release.
    ///
    /// Upstream semantics (`adminservice` `ItemController.findDeletedItems`,
    /// lines 207-231): resolve the latest active release for the namespace; when
    /// one exists only the commits created at or after that release are
    /// inspected, otherwise every commit is. The deleted items are then read out
    /// of each commit's `changeSets` payload, which upstream persists as a
    /// serialized `ConfigChangeContentBuilder`.
    ///
    /// Note this is NOT the set of currently soft-deleted items: an item that was
    /// deleted (and even re-created) since the last publish is still reported
    /// here, matching upstream behaviour.
    pub async fn list_deleted_since_last_release(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> Result<Vec<ItemDTO>, anyhow::Error> {
        let namespace = self
            .persistence
            .get_by_app_cluster(app_id, cluster_name, namespace_name)
            .await?;

        if namespace.is_none() {
            return Ok(vec![]);
        }

        // Upstream `releaseService.findLatestActiveRelease`: the newest
        // non-abandoned release for the namespace. `list_active` already returns
        // releases newest-id-first, so the first entry is the one we want.
        let latest_release = <dyn ReleasePersistence>::list_active(
            &self.persistence,
            app_id,
            cluster_name,
            namespace_name,
        )
        .await?
        .into_iter()
        .next();

        let commits = <dyn CommitPersistence>::list_by_namespace(
            &self.persistence,
            app_id,
            cluster_name,
            namespace_name,
        )
        .await?;

        let since = latest_release.map(|release| release.data_change_created_time);

        let mut deleted = Vec::new();

        for commit in commits {
            // Upstream narrows the commits to those created at or after the
            // latest release (`commitService.find(..., releaseTime, null)`).
            if let Some(since) = since
                && commit.data_change_created_time < since {
                    continue;
                }

            // A commit persisted in an older (or empty) format contributes no
            // deleted items rather than failing the whole request.
            let content: ConfigChangeContent = match serde_json::from_str(&commit.change_sets) {
                Ok(content) => content,
                Err(_) => continue,
            };

            deleted.extend(content.delete_items);
        }

        Ok(deleted)
    }
}

impl From<StoredItem> for ItemDTO {
    fn from(stored: StoredItem) -> Self {
        Self {
            id: Some(stored.id),
            key: stored.key,
            value: stored.value,
            r#type: Some(stored.r#type),
            comment: stored.comment,
            line_num: Some(stored.line_num),
            data_change_created_by: Some(stored.data_change_created_by),
            data_change_last_modified_by: stored.data_change_last_modified_by,
            data_change_created_time: Some(format_timestamp(stored.data_change_created_time)),
            data_change_last_time: stored.data_change_last_time.map(format_timestamp),
        }
    }
}

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.f+00:00")
        .to_string()
}