//! Embedded (RocksDB) implementations of the Apollo portal/admin persistence traits.
//!
//! Each entity is stored as its `sea_orm` `Model` serialized to JSON inside a
//! dedicated column family. See `store.rs` for the key/value helpers.

use async_trait::async_trait;
use chrono::Utc;

use crate::persistence::traits::{
    AppNamespacePersistence, AuditPersistence, ConsumerPersistence, ConsumerTokenPersistence,
    FavoritePersistence, InstanceConfigPersistence, PermissionPersistence, ReleaseHistoryPersistence,
    RolePersistence, ServerConfigPersistence,
};
use crate::persistence::embedded::JsonStore;
use super::EmbeddedApolloPersistence;

use batata_consistency::raft::state_machine::{
    CF_APOLLO_APP_NAMESPACE, CF_APOLLO_AUDIT, CF_APOLLO_CONSUMER, CF_APOLLO_CONSUMER_TOKEN,
    CF_APOLLO_FAVORITE, CF_APOLLO_INSTANCE_CONFIG, CF_APOLLO_PERMISSION, CF_APOLLO_RELEASE_HISTORY,
    CF_APOLLO_ROLE, CF_APOLLO_ROLE_PERMISSION, CF_APOLLO_SERVER_CONFIG, CF_APOLLO_USER_ROLE,
};

use crate::api::dto::{
    AppNamespaceDTO, AuditDTO, ConsumerDTO, FavoriteDTO, InstanceConfigDTO, RoleDTO, ServerConfigDTO,
};
use crate::entity::{
    apollo_app_namespace, apollo_audit, apollo_consumer, apollo_consumer_token, apollo_favorite,
    apollo_instance_config, apollo_permission, apollo_release_history, apollo_role,
    apollo_role_permission, apollo_server_config, apollo_user_role,
};

impl EmbeddedApolloPersistence {
    fn store(&self, cf: &'static str) -> JsonStore {
        JsonStore::new(self.db.clone(), cf)
    }

    fn next_id(&self) -> i32 {
        self.portal_id_gen.next_id()
    }
}

#[async_trait]
impl AppNamespacePersistence for EmbeddedApolloPersistence {
    async fn create_app_namespace(&self, dto: AppNamespaceDTO) -> anyhow::Result<apollo_app_namespace::Model> {
        let store = self.store(CF_APOLLO_APP_NAMESPACE);
        let key = format!("{}:{}", dto.app_id, dto.name);
        if store.get::<apollo_app_namespace::Model>(key.as_bytes())?.is_some() {
            anyhow::bail!("AppNamespace already exists: {}/{}", dto.app_id, dto.name);
        }
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "admin".to_string());
        let model = apollo_app_namespace::Model {
            id: self.next_id(),
            name: dto.name,
            app_id: dto.app_id,
            format: dto.format,
            is_public: dto.is_public,
            comment: dto.comment,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        store.put(key.as_bytes(), &model)?;
        Ok(model)
    }

    async fn get_app_namespace(&self, app_id: &str, name: &str) -> anyhow::Result<Option<apollo_app_namespace::Model>> {
        let store = self.store(CF_APOLLO_APP_NAMESPACE);
        let key = format!("{}:{}", app_id, name);
        Ok(store
            .get::<apollo_app_namespace::Model>(key.as_bytes())?
            .filter(|m| !m.is_deleted))
    }

    async fn list_app_namespace_by_app(&self, app_id: &str) -> anyhow::Result<Vec<apollo_app_namespace::Model>> {
        let store = self.store(CF_APOLLO_APP_NAMESPACE);
        let mut all: Vec<apollo_app_namespace::Model> = store.scan_all()?;
        all.retain(|m| m.app_id == app_id && !m.is_deleted);
        all.sort_by(|a, b| b.data_change_created_time.cmp(&a.data_change_created_time));
        Ok(all)
    }

    async fn list_public_app_namespace(&self) -> anyhow::Result<Vec<apollo_app_namespace::Model>> {
        let store = self.store(CF_APOLLO_APP_NAMESPACE);
        let mut all: Vec<apollo_app_namespace::Model> = store.scan_all()?;
        all.retain(|m| m.is_public && !m.is_deleted);
        all.sort_by(|a, b| b.data_change_created_time.cmp(&a.data_change_created_time));
        Ok(all)
    }

    async fn delete_app_namespace(&self, app_id: &str, name: &str, operator: &str) -> anyhow::Result<()> {
        let store = self.store(CF_APOLLO_APP_NAMESPACE);
        let key = format!("{}:{}", app_id, name);
        let mut model = store
            .get::<apollo_app_namespace::Model>(key.as_bytes())?
            .ok_or_else(|| anyhow::anyhow!("AppNamespace not found: {}/{}", app_id, name))?;
        let now = Utc::now().naive_utc();
        model.is_deleted = true;
        model.deleted_at = now.and_utc().timestamp_millis();
        model.data_change_last_modified_by = Some(operator.to_string());
        model.data_change_last_time = Some(now);
        store.put(key.as_bytes(), &model)?;
        Ok(())
    }
}

#[async_trait]
impl AuditPersistence for EmbeddedApolloPersistence {
    async fn create_audit(&self, dto: AuditDTO) -> anyhow::Result<apollo_audit::Model> {
        let store = self.store(CF_APOLLO_AUDIT);
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "system".to_string());
        let model = apollo_audit::Model {
            id: self.next_id(),
            audit_key: dto.audit_key,
            entity_name: dto.entity_name,
            entity_id: dto.entity_id,
            op_name: dto.op_name,
            op_time: now,
            op_by: dto.op_by,
            op_client_ip: dto.op_client_ip,
            detail: dto.detail,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        let key = format!("id:{}", model.id);
        store.put(key.as_bytes(), &model)?;
        Ok(model)
    }

    async fn list_audit(&self, page: u64, size: u64) -> anyhow::Result<(Vec<apollo_audit::Model>, u64)> {
        let store = self.store(CF_APOLLO_AUDIT);
        let mut all: Vec<apollo_audit::Model> = store.scan_all()?;
        all.sort_by(|a, b| b.op_time.cmp(&a.op_time));
        let total = all.len() as u64;
        let page = page.max(1);
        let size = size.max(1);
        let start = ((page - 1) * size) as usize;
        let page_items = if start >= all.len() { Vec::new() } else { all.into_iter().skip(start).take(size as usize).collect() };
        Ok((page_items, total))
    }

    async fn list_audit_by_entity(&self, entity_name: &str, entity_id: &str) -> anyhow::Result<Vec<apollo_audit::Model>> {
        let store = self.store(CF_APOLLO_AUDIT);
        let mut all: Vec<apollo_audit::Model> = store.scan_all()?;
        all.retain(|m| m.entity_name == entity_name && m.entity_id == entity_id);
        all.sort_by(|a, b| b.op_time.cmp(&a.op_time));
        Ok(all)
    }
}

#[async_trait]
impl ConsumerPersistence for EmbeddedApolloPersistence {
    async fn create_consumer(&self, dto: ConsumerDTO) -> anyhow::Result<apollo_consumer::Model> {
        let store = self.store(CF_APOLLO_CONSUMER);
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "admin".to_string());
        let model = apollo_consumer::Model {
            id: self.next_id(),
            app_id: dto.app_id,
            name: dto.name,
            org_id: dto.org_id,
            org_name: dto.org_name,
            owner_name: dto.owner_name,
            owner_email: dto.owner_email,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        let key = format!("id:{}", model.id);
        store.put(key.as_bytes(), &model)?;
        Ok(model)
    }

    async fn get_consumer(&self, id: i32) -> anyhow::Result<Option<apollo_consumer::Model>> {
        let store = self.store(CF_APOLLO_CONSUMER);
        Ok(store.get::<apollo_consumer::Model>(format!("id:{}", id).as_bytes())?)
    }

    async fn get_consumer_by_app(&self, app_id: &str) -> anyhow::Result<Option<apollo_consumer::Model>> {
        let store = self.store(CF_APOLLO_CONSUMER);
        let all: Vec<apollo_consumer::Model> = store.scan_all()?;
        Ok(all.into_iter().find(|m| m.app_id == app_id))
    }

    async fn list_consumers(&self) -> anyhow::Result<Vec<apollo_consumer::Model>> {
        let store = self.store(CF_APOLLO_CONSUMER);
        Ok(store.scan_all()?)
    }
}

#[async_trait]
impl PermissionPersistence for EmbeddedApolloPersistence {
    async fn create_permission(&self, permission_type: i32, target_id: &str, created_by: &str) -> anyhow::Result<apollo_permission::Model> {
        let store = self.store(CF_APOLLO_PERMISSION);
        let now = Utc::now().naive_utc();
        let model = apollo_permission::Model {
            id: self.next_id(),
            permission_type,
            target_id: target_id.to_string(),
            data_change_created_by: created_by.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        let key = format!("id:{}", model.id);
        store.put(key.as_bytes(), &model)?;
        Ok(model)
    }

    async fn list_permission_by_target(&self, target_id: &str) -> anyhow::Result<Vec<apollo_permission::Model>> {
        let store = self.store(CF_APOLLO_PERMISSION);
        let mut all: Vec<apollo_permission::Model> = store.scan_all()?;
        all.retain(|m| m.target_id == target_id);
        Ok(all)
    }

    async fn list_permission_by_type(&self, permission_type: i32) -> anyhow::Result<Vec<apollo_permission::Model>> {
        let store = self.store(CF_APOLLO_PERMISSION);
        let mut all: Vec<apollo_permission::Model> = store.scan_all()?;
        all.retain(|m| m.permission_type == permission_type);
        Ok(all)
    }
}

#[async_trait]
impl RolePersistence for EmbeddedApolloPersistence {
    async fn create_role(&self, dto: RoleDTO) -> anyhow::Result<apollo_role::Model> {
        let store = self.store(CF_APOLLO_ROLE);
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "admin".to_string());
        let model = apollo_role::Model {
            id: self.next_id(),
            role_name: dto.role_name,
            role_type: dto.role_type,
            target_id: dto.target_id,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        let key = format!("id:{}", model.id);
        store.put(key.as_bytes(), &model)?;
        Ok(model)
    }

    async fn get_role(&self, id: i32) -> anyhow::Result<Option<apollo_role::Model>> {
        let store = self.store(CF_APOLLO_ROLE);
        Ok(store
            .get::<apollo_role::Model>(format!("id:{}", id).as_bytes())?
            .filter(|m| !m.is_deleted))
    }

    async fn list_role_by_target(&self, target_id: &str) -> anyhow::Result<Vec<apollo_role::Model>> {
        let store = self.store(CF_APOLLO_ROLE);
        let mut all: Vec<apollo_role::Model> = store.scan_all()?;
        all.retain(|m| m.target_id == target_id && !m.is_deleted);
        Ok(all)
    }

    async fn delete_role(&self, id: i32) -> anyhow::Result<()> {
        let store = self.store(CF_APOLLO_ROLE);
        let mut model = store
            .get::<apollo_role::Model>(format!("id:{}", id).as_bytes())?
            .ok_or_else(|| anyhow::anyhow!("Role not found: {}", id))?;
        let now = Utc::now().naive_utc();
        model.is_deleted = true;
        model.deleted_at = now.and_utc().timestamp_millis();
        store.put(format!("id:{}", id).as_bytes(), &model)?;
        Ok(())
    }

    async fn assign_role_permission(&self, role_id: i32, permission_id: i32, created_by: &str) -> anyhow::Result<()> {
        let store = self.store(CF_APOLLO_ROLE_PERMISSION);
        let now = Utc::now().naive_utc();
        let model = apollo_role_permission::Model {
            id: self.next_id(),
            role_id,
            permission_id,
            data_change_created_by: created_by.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        let key = format!("id:{}", model.id);
        store.put(key.as_bytes(), &model)?;
        Ok(())
    }

    async fn remove_role_permission(&self, role_id: i32, permission_id: i32) -> anyhow::Result<()> {
        let store = self.store(CF_APOLLO_ROLE_PERMISSION);
        let all: Vec<apollo_role_permission::Model> = store.scan_all()?;
        for m in all {
            if m.role_id == role_id && m.permission_id == permission_id {
                store.delete(format!("id:{}", m.id).as_bytes())?;
            }
        }
        Ok(())
    }

    async fn list_role_permissions(&self, role_id: i32) -> anyhow::Result<Vec<i32>> {
        let store = self.store(CF_APOLLO_ROLE_PERMISSION);
        let all: Vec<apollo_role_permission::Model> = store.scan_all()?;
        Ok(all.into_iter().filter(|m| m.role_id == role_id).map(|m| m.permission_id).collect())
    }

    async fn assign_role_to_user(&self, user_id: &str, role_id: i32, created_by: &str) -> anyhow::Result<()> {
        let store = self.store(CF_APOLLO_USER_ROLE);
        let now = Utc::now().naive_utc();
        let model = apollo_user_role::Model {
            id: self.next_id(),
            user_id: user_id.to_string(),
            role_id,
            data_change_created_by: created_by.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        let key = format!("id:{}", model.id);
        store.put(key.as_bytes(), &model)?;
        Ok(())
    }

    async fn remove_role_from_user(&self, user_id: &str, role_id: i32) -> anyhow::Result<()> {
        let store = self.store(CF_APOLLO_USER_ROLE);
        let all: Vec<apollo_user_role::Model> = store.scan_all()?;
        for m in all {
            if m.user_id == user_id && m.role_id == role_id {
                store.delete(format!("id:{}", m.id).as_bytes())?;
            }
        }
        Ok(())
    }

    async fn list_user_roles(&self, user_id: &str) -> anyhow::Result<Vec<apollo_role::Model>> {
        let ur_store = self.store(CF_APOLLO_USER_ROLE);
        let all_ur: Vec<apollo_user_role::Model> = ur_store.scan_all()?;
        let role_ids: Vec<i32> = all_ur.into_iter().filter(|m| m.user_id == user_id).map(|m| m.role_id).collect();
        if role_ids.is_empty() {
            return Ok(vec![]);
        }
        let role_store = self.store(CF_APOLLO_ROLE);
        let all_roles: Vec<apollo_role::Model> = role_store.scan_all()?;
        Ok(all_roles
            .into_iter()
            .filter(|m| !m.is_deleted && role_ids.contains(&m.id))
            .collect())
    }
}

#[async_trait]
impl FavoritePersistence for EmbeddedApolloPersistence {
    async fn create_favorite(&self, dto: FavoriteDTO) -> anyhow::Result<apollo_favorite::Model> {
        let store = self.store(CF_APOLLO_FAVORITE);
        let all: Vec<apollo_favorite::Model> = store.scan_all()?;
        if let Some(existing) = all.into_iter().find(|m| m.user_id == dto.user_id && m.app_id == dto.app_id) {
            return Ok(existing);
        }
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| dto.user_id.clone());
        let model = apollo_favorite::Model {
            id: self.next_id(),
            user_id: dto.user_id,
            app_id: dto.app_id,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        let key = format!("id:{}", model.id);
        store.put(key.as_bytes(), &model)?;
        Ok(model)
    }

    async fn list_favorite_by_user(&self, user_id: &str) -> anyhow::Result<Vec<apollo_favorite::Model>> {
        let store = self.store(CF_APOLLO_FAVORITE);
        let mut all: Vec<apollo_favorite::Model> = store.scan_all()?;
        all.retain(|m| m.user_id == user_id);
        all.sort_by(|a, b| b.data_change_created_time.cmp(&a.data_change_created_time));
        Ok(all)
    }

    async fn delete_favorite(&self, id: i32, user_id: &str) -> anyhow::Result<()> {
        let store = self.store(CF_APOLLO_FAVORITE);
        let model = store
            .get::<apollo_favorite::Model>(format!("id:{}", id).as_bytes())?
            .ok_or_else(|| anyhow::anyhow!("Favorite not found: {}", id))?;
        if model.user_id != user_id {
            anyhow::bail!("Favorite not found: {}", id);
        }
        store.delete(format!("id:{}", id).as_bytes())?;
        Ok(())
    }
}

#[async_trait]
impl ServerConfigPersistence for EmbeddedApolloPersistence {
    async fn get_server_config(&self, key: &str) -> anyhow::Result<Option<apollo_server_config::Model>> {
        let store = self.store(CF_APOLLO_SERVER_CONFIG);
        Ok(store.get::<apollo_server_config::Model>(key.as_bytes())?)
    }

    async fn list_server_config(&self) -> anyhow::Result<Vec<apollo_server_config::Model>> {
        let store = self.store(CF_APOLLO_SERVER_CONFIG);
        let mut all: Vec<apollo_server_config::Model> = store.scan_all()?;
        all.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(all)
    }

    async fn create_server_config(&self, dto: ServerConfigDTO) -> anyhow::Result<apollo_server_config::Model> {
        let store = self.store(CF_APOLLO_SERVER_CONFIG);
        if store.get::<apollo_server_config::Model>(dto.key.as_bytes())?.is_some() {
            anyhow::bail!("ServerConfig already exists: {}", dto.key);
        }
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_default();
        let model = apollo_server_config::Model {
            id: self.next_id(),
            key: dto.key.clone(),
            value: dto.value,
            comment: dto.comment,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        store.put(dto.key.as_bytes(), &model)?;
        Ok(model)
    }

    async fn update_server_config(&self, key: &str, value: &str, operator: &str) -> anyhow::Result<()> {
        let store = self.store(CF_APOLLO_SERVER_CONFIG);
        let mut model = store
            .get::<apollo_server_config::Model>(key.as_bytes())?
            .ok_or_else(|| anyhow::anyhow!("ServerConfig not found: {}", key))?;
        let now = Utc::now().naive_utc();
        model.value = value.to_string();
        model.data_change_last_modified_by = Some(operator.to_string());
        model.data_change_last_time = Some(now);
        store.put(key.as_bytes(), &model)?;
        Ok(())
    }

    async fn delete_server_config(&self, key: &str, _operator: &str) -> anyhow::Result<()> {
        let store = self.store(CF_APOLLO_SERVER_CONFIG);
        if store.get::<apollo_server_config::Model>(key.as_bytes())?.is_none() {
            anyhow::bail!("ServerConfig not found: {}", key);
        }
        store.delete(key.as_bytes())?;
        Ok(())
    }
}

#[async_trait]
impl InstanceConfigPersistence for EmbeddedApolloPersistence {
    async fn create_or_update_instance_config(&self, dto: InstanceConfigDTO) -> anyhow::Result<apollo_instance_config::Model> {
        let store = self.store(CF_APOLLO_INSTANCE_CONFIG);
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "system".to_string());
        let all: Vec<apollo_instance_config::Model> = store.scan_all()?;
        let config_app_id = dto.config_app_id.clone().unwrap_or_default();
        if let Some(mut model) = all.into_iter().find(|m| m.instance_id == dto.instance_id && m.namespace_name == dto.namespace_name && m.cluster_name == dto.cluster_name && m.config_app_id == config_app_id) {
            model.release_key = dto.release_key;
            model.configurations = dto.configurations;
            model.data_change_last_modified_by = Some(created_by);
            model.data_change_last_time = Some(now);
            let key = format!("id:{}", model.id);
            store.put(key.as_bytes(), &model)?;
            return Ok(model);
        }
        let model = apollo_instance_config::Model {
            id: self.next_id(),
            instance_id: dto.instance_id,
            config_app_id,
            namespace_name: dto.namespace_name,
            cluster_name: dto.cluster_name,
            release_key: dto.release_key,
            configurations: dto.configurations,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        let key = format!("id:{}", model.id);
        store.put(key.as_bytes(), &model)?;
        Ok(model)
    }

    async fn get_instance_config_by_instance(&self, instance_id: i32) -> anyhow::Result<Vec<apollo_instance_config::Model>> {
        let store = self.store(CF_APOLLO_INSTANCE_CONFIG);
        let mut all: Vec<apollo_instance_config::Model> = store.scan_all()?;
        all.retain(|m| m.instance_id == instance_id);
        all.sort_by(|a, b| b.data_change_last_time.cmp(&a.data_change_last_time));
        Ok(all)
    }

    async fn list_instance_config_by_app_cluster(&self, _app_id: &str, cluster_name: &str, _namespace_name: &str) -> anyhow::Result<Vec<apollo_instance_config::Model>> {
        let store = self.store(CF_APOLLO_INSTANCE_CONFIG);
        let mut all: Vec<apollo_instance_config::Model> = store.scan_all()?;
        all.retain(|m| m.cluster_name == cluster_name);
        all.sort_by(|a, b| b.data_change_last_time.cmp(&a.data_change_last_time));
        Ok(all)
    }

    async fn delete_instance_config_by_instance(&self, instance_id: i32) -> anyhow::Result<()> {
        let store = self.store(CF_APOLLO_INSTANCE_CONFIG);
        let all: Vec<apollo_instance_config::Model> = store.scan_all()?;
        for m in all {
            if m.instance_id == instance_id {
                store.delete(format!("id:{}", m.id).as_bytes())?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ReleaseHistoryPersistence for EmbeddedApolloPersistence {
    async fn find_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, page: u64, size: u64) -> anyhow::Result<(Vec<apollo_release_history::Model>, u64)> {
        let store = self.store(CF_APOLLO_RELEASE_HISTORY);
        let mut all: Vec<apollo_release_history::Model> = store.scan_all()?;
        all.retain(|m| m.app_id == app_id && m.cluster_name == cluster_name && m.namespace_name == namespace_name && !m.is_deleted);
        all.sort_by(|a, b| b.data_change_created_time.cmp(&a.data_change_created_time));
        let total = all.len() as u64;
        let page = page.max(1);
        let size = size.max(1);
        let start = ((page - 1) * size) as usize;
        let page_items = if start >= all.len() { Vec::new() } else { all.into_iter().skip(start).take(size as usize).collect() };
        Ok((page_items, total))
    }

    async fn record_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, branch_name: &str, release_id: i32, previous_release_id: i32, operation: i16, operation_context: &str, operator: &str) -> anyhow::Result<apollo_release_history::Model> {
        let store = self.store(CF_APOLLO_RELEASE_HISTORY);
        let now = Utc::now().naive_utc();
        let model = apollo_release_history::Model {
            id: self.next_id(),
            app_id: app_id.to_string(),
            cluster_name: cluster_name.to_string(),
            namespace_name: namespace_name.to_string(),
            branch_name: branch_name.to_string(),
            release_id,
            previous_release_id,
            operation,
            operation_context: operation_context.to_string(),
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: operator.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        let key = format!("id:{}", model.id);
        store.put(key.as_bytes(), &model)?;
        Ok(model)
    }
}

#[async_trait]
impl ConsumerTokenPersistence for EmbeddedApolloPersistence {
    async fn create_consumer_token(&self, consumer_id: i32, created_by: &str) -> anyhow::Result<apollo_consumer_token::Model> {
        let store = self.store(CF_APOLLO_CONSUMER_TOKEN);
        let now = Utc::now().naive_utc();
        let token = format!("{}-{}", consumer_id, now.and_utc().timestamp_millis());
        let model = apollo_consumer_token::Model {
            id: self.next_id(),
            consumer_id,
            token,
            data_change_created_by: created_by.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        store.put(format!("id:{}", model.id).as_bytes(), &model)?;
        Ok(model)
    }

    async fn list_tokens_by_consumer(&self, consumer_id: i32) -> anyhow::Result<Vec<apollo_consumer_token::Model>> {
        let store = self.store(CF_APOLLO_CONSUMER_TOKEN);
        let all: Vec<apollo_consumer_token::Model> = store.scan_all()?;
        Ok(all.into_iter().filter(|m| m.consumer_id == consumer_id).collect())
    }

    async fn delete_consumer_token(&self, id: i32) -> anyhow::Result<()> {
        let store = self.store(CF_APOLLO_CONSUMER_TOKEN);
        store.delete(format!("id:{}", id).as_bytes())?;
        Ok(())
    }

    async fn get_consumer_token_by_token(&self, token: &str) -> anyhow::Result<Option<apollo_consumer_token::Model>> {
        let store = self.store(CF_APOLLO_CONSUMER_TOKEN);
        let all: Vec<apollo_consumer_token::Model> = store.scan_all()?;
        Ok(all.into_iter().find(|m| m.token == token))
    }
}
