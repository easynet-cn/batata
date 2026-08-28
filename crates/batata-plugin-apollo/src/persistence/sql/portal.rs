//! SQL (SeaORM) implementations of the Apollo portal/admin persistence traits.
//!
//! These relocate the raw `sea_orm` queries that previously lived inside the
//! services, returning the `Model` so the services keep their `model_to_dto`
//! conversions. Mirrors the embedded (RocksDB) behavior in `embedded/portal.rs`.

use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entity::{
    apollo_app_namespace, apollo_audit, apollo_consumer, apollo_consumer_token, apollo_favorite,
    apollo_instance_config, apollo_permission, apollo_release_history, apollo_role,
    apollo_role_permission, apollo_server_config, apollo_user_role,
};
use crate::persistence::sql::SqlApolloPersistence;
use crate::persistence::traits::{
    AppNamespacePersistence, AuditPersistence, ConsumerPersistence, ConsumerTokenPersistence,
    FavoritePersistence, InstanceConfigPersistence, PermissionPersistence,
    ReleaseHistoryPersistence, RolePersistence, ServerConfigPersistence,
};
use chrono::Utc;

#[async_trait]
impl AppNamespacePersistence for SqlApolloPersistence {
    async fn create_app_namespace(&self, dto: crate::api::dto::AppNamespaceDTO) -> anyhow::Result<apollo_app_namespace::Model> {
        let db = &self.db;
        let existing = apollo_app_namespace::Entity::find()
            .filter(apollo_app_namespace::Column::AppId.eq(&dto.app_id))
            .filter(apollo_app_namespace::Column::Name.eq(&dto.name))
            .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
            .one(db)
            .await?;
        if existing.is_some() {
            anyhow::bail!("AppNamespace already exists: {}/{}", dto.app_id, dto.name);
        }
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "admin".to_string());
        let active_model = apollo_app_namespace::ActiveModel {
            app_id: Set(dto.app_id),
            name: Set(dto.name),
            format: Set(dto.format),
            is_public: Set(dto.is_public),
            comment: Set(dto.comment),
            is_deleted: Set(false),
            deleted_at: Set(0),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        Ok(active_model.insert(db).await?)
    }

    async fn get_app_namespace(&self, app_id: &str, name: &str) -> anyhow::Result<Option<apollo_app_namespace::Model>> {
        let db = &self.db;
        Ok(apollo_app_namespace::Entity::find()
            .filter(apollo_app_namespace::Column::AppId.eq(app_id))
            .filter(apollo_app_namespace::Column::Name.eq(name))
            .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
            .one(db)
            .await?)
    }

    async fn list_app_namespace_by_app(&self, app_id: &str) -> anyhow::Result<Vec<apollo_app_namespace::Model>> {
        let db = &self.db;
        Ok(apollo_app_namespace::Entity::find()
            .filter(apollo_app_namespace::Column::AppId.eq(app_id))
            .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
            .order_by_desc(apollo_app_namespace::Column::DataChangeCreatedTime)
            .all(db)
            .await?)
    }

    async fn list_public_app_namespace(&self) -> anyhow::Result<Vec<apollo_app_namespace::Model>> {
        let db = &self.db;
        Ok(apollo_app_namespace::Entity::find()
            .filter(apollo_app_namespace::Column::IsPublic.eq(true))
            .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
            .order_by_desc(apollo_app_namespace::Column::DataChangeCreatedTime)
            .all(db)
            .await?)
    }

    async fn delete_app_namespace(&self, app_id: &str, name: &str, operator: &str) -> anyhow::Result<()> {
        let db = &self.db;
        let model = apollo_app_namespace::Entity::find()
            .filter(apollo_app_namespace::Column::AppId.eq(app_id))
            .filter(apollo_app_namespace::Column::Name.eq(name))
            .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("AppNamespace not found: {}/{}", app_id, name))?;
        let now = Utc::now().naive_utc();
        let mut active_model: apollo_app_namespace::ActiveModel = model.into();
        active_model.is_deleted = Set(true);
        active_model.deleted_at = Set(now.and_utc().timestamp_millis());
        active_model.data_change_last_modified_by = Set(Some(operator.to_string()));
        active_model.data_change_last_time = Set(Some(now));
        active_model.update(db).await?;
        Ok(())
    }
}

#[async_trait]
impl AuditPersistence for SqlApolloPersistence {
    async fn create_audit(&self, dto: crate::api::dto::AuditDTO) -> anyhow::Result<apollo_audit::Model> {
        let db = &self.db;
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "system".to_string());
        let active_model = apollo_audit::ActiveModel {
            audit_key: Set(dto.audit_key),
            entity_name: Set(dto.entity_name),
            entity_id: Set(dto.entity_id),
            op_name: Set(dto.op_name),
            op_time: Set(now),
            op_by: Set(dto.op_by),
            op_client_ip: Set(dto.op_client_ip),
            detail: Set(dto.detail),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        Ok(active_model.insert(db).await?)
    }

    async fn list_audit(&self, page: u64, size: u64) -> anyhow::Result<(Vec<apollo_audit::Model>, u64)> {
        let db = &self.db;
        let mut models = apollo_audit::Entity::find()
            .order_by_desc(apollo_audit::Column::OpTime)
            .all(db)
            .await?;
        let total = models.len() as u64;
        let page = page.max(1);
        let size = size.max(1);
        let start = ((page - 1) * size) as usize;
        let page_items = if start >= models.len() { Vec::new() } else { models.drain(start..).take(size as usize).collect() };
        Ok((page_items, total))
    }

    async fn list_audit_by_entity(&self, entity_name: &str, entity_id: &str) -> anyhow::Result<Vec<apollo_audit::Model>> {
        let db = &self.db;
        Ok(apollo_audit::Entity::find()
            .filter(apollo_audit::Column::EntityName.eq(entity_name))
            .filter(apollo_audit::Column::EntityId.eq(entity_id))
            .order_by_desc(apollo_audit::Column::OpTime)
            .all(db)
            .await?)
    }
}

#[async_trait]
impl ConsumerPersistence for SqlApolloPersistence {
    async fn create_consumer(&self, dto: crate::api::dto::ConsumerDTO) -> anyhow::Result<apollo_consumer::Model> {
        let db = &self.db;
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "admin".to_string());
        let active_model = apollo_consumer::ActiveModel {
            app_id: Set(dto.app_id),
            name: Set(dto.name),
            org_id: Set(dto.org_id),
            org_name: Set(dto.org_name),
            owner_name: Set(dto.owner_name),
            owner_email: Set(dto.owner_email),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        Ok(active_model.insert(db).await?)
    }

    async fn get_consumer(&self, id: i32) -> anyhow::Result<Option<apollo_consumer::Model>> {
        Ok(apollo_consumer::Entity::find_by_id(id).one(&self.db).await?)
    }

    async fn get_consumer_by_app(&self, app_id: &str) -> anyhow::Result<Option<apollo_consumer::Model>> {
        Ok(apollo_consumer::Entity::find()
            .filter(apollo_consumer::Column::AppId.eq(app_id))
            .one(&self.db)
            .await?)
    }

    async fn list_consumers(&self) -> anyhow::Result<Vec<apollo_consumer::Model>> {
        Ok(apollo_consumer::Entity::find().all(&self.db).await?)
    }
}

#[async_trait]
impl PermissionPersistence for SqlApolloPersistence {
    async fn create_permission(&self, permission_type: i32, target_id: &str, created_by: &str) -> anyhow::Result<apollo_permission::Model> {
        let db = &self.db;
        let now = Utc::now().naive_utc();
        let active_model = apollo_permission::ActiveModel {
            permission_type: Set(permission_type),
            target_id: Set(target_id.to_string()),
            data_change_created_by: Set(created_by.to_string()),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        Ok(active_model.insert(db).await?)
    }

    async fn list_permission_by_target(&self, target_id: &str) -> anyhow::Result<Vec<apollo_permission::Model>> {
        Ok(apollo_permission::Entity::find()
            .filter(apollo_permission::Column::TargetId.eq(target_id))
            .all(&self.db)
            .await?)
    }

    async fn list_permission_by_type(&self, permission_type: i32) -> anyhow::Result<Vec<apollo_permission::Model>> {
        Ok(apollo_permission::Entity::find()
            .filter(apollo_permission::Column::PermissionType.eq(permission_type))
            .all(&self.db)
            .await?)
    }
}

#[async_trait]
impl RolePersistence for SqlApolloPersistence {
    async fn create_role(&self, dto: crate::api::dto::RoleDTO) -> anyhow::Result<apollo_role::Model> {
        let db = &self.db;
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "admin".to_string());
        let active_model = apollo_role::ActiveModel {
            role_name: Set(dto.role_name),
            role_type: Set(dto.role_type),
            target_id: Set(dto.target_id),
            is_deleted: Set(false),
            deleted_at: Set(0),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        Ok(active_model.insert(db).await?)
    }

    async fn get_role(&self, id: i32) -> anyhow::Result<Option<apollo_role::Model>> {
        Ok(apollo_role::Entity::find_by_id(id)
            .filter(apollo_role::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?)
    }

    async fn list_role_by_target(&self, target_id: &str) -> anyhow::Result<Vec<apollo_role::Model>> {
        Ok(apollo_role::Entity::find()
            .filter(apollo_role::Column::TargetId.eq(target_id))
            .filter(apollo_role::Column::IsDeleted.eq(false))
            .all(&self.db)
            .await?)
    }

    async fn delete_role(&self, id: i32) -> anyhow::Result<()> {
        let db = &self.db;
        let model = apollo_role::Entity::find_by_id(id)
            .filter(apollo_role::Column::IsDeleted.eq(false))
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Role not found: {}", id))?;
        let now = Utc::now().naive_utc();
        let mut active_model: apollo_role::ActiveModel = model.into();
        active_model.is_deleted = Set(true);
        active_model.deleted_at = Set(now.and_utc().timestamp_millis());
        active_model.update(db).await?;
        Ok(())
    }

    async fn assign_role_permission(&self, role_id: i32, permission_id: i32, created_by: &str) -> anyhow::Result<()> {
        let db = &self.db;
        let now = Utc::now().naive_utc();
        let active_model = apollo_role_permission::ActiveModel {
            role_id: Set(role_id),
            permission_id: Set(permission_id),
            data_change_created_by: Set(created_by.to_string()),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        apollo_role_permission::Entity::insert(active_model).exec(db).await?;
        Ok(())
    }

    async fn remove_role_permission(&self, role_id: i32, permission_id: i32) -> anyhow::Result<()> {
        apollo_role_permission::Entity::delete_many()
            .filter(apollo_role_permission::Column::RoleId.eq(role_id))
            .filter(apollo_role_permission::Column::PermissionId.eq(permission_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn list_role_permissions(&self, role_id: i32) -> anyhow::Result<Vec<i32>> {
        let models = apollo_role_permission::Entity::find()
            .filter(apollo_role_permission::Column::RoleId.eq(role_id))
            .all(&self.db)
            .await?;
        Ok(models.into_iter().map(|m| m.permission_id).collect())
    }

    async fn assign_role_to_user(&self, user_id: &str, role_id: i32, created_by: &str) -> anyhow::Result<()> {
        let db = &self.db;
        let now = Utc::now().naive_utc();
        let active_model = apollo_user_role::ActiveModel {
            user_id: Set(user_id.to_string()),
            role_id: Set(role_id),
            data_change_created_by: Set(created_by.to_string()),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        apollo_user_role::Entity::insert(active_model).exec(db).await?;
        Ok(())
    }

    async fn remove_role_from_user(&self, user_id: &str, role_id: i32) -> anyhow::Result<()> {
        apollo_user_role::Entity::delete_many()
            .filter(apollo_user_role::Column::UserId.eq(user_id))
            .filter(apollo_user_role::Column::RoleId.eq(role_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn list_user_roles(&self, user_id: &str) -> anyhow::Result<Vec<apollo_role::Model>> {
        let db = &self.db;
        let user_roles = apollo_user_role::Entity::find()
            .filter(apollo_user_role::Column::UserId.eq(user_id))
            .all(db)
            .await?;
        let role_ids: Vec<i32> = user_roles.iter().map(|ur| ur.role_id).collect();
        if role_ids.is_empty() {
            return Ok(vec![]);
        }
        Ok(apollo_role::Entity::find()
            .filter(apollo_role::Column::IsDeleted.eq(false))
            .filter(apollo_role::Column::Id.is_in(role_ids))
            .all(db)
            .await?)
    }
}

#[async_trait]
impl FavoritePersistence for SqlApolloPersistence {
    async fn create_favorite(&self, dto: crate::api::dto::FavoriteDTO) -> anyhow::Result<apollo_favorite::Model> {
        let db = &self.db;
        let existing = apollo_favorite::Entity::find()
            .filter(apollo_favorite::Column::UserId.eq(&dto.user_id))
            .filter(apollo_favorite::Column::AppId.eq(&dto.app_id))
            .one(db)
            .await?;
        if let Some(model) = existing {
            return Ok(model);
        }
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| dto.user_id.clone());
        let active_model = apollo_favorite::ActiveModel {
            user_id: Set(dto.user_id),
            app_id: Set(dto.app_id),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        Ok(active_model.insert(db).await?)
    }

    async fn list_favorite_by_user(&self, user_id: &str) -> anyhow::Result<Vec<apollo_favorite::Model>> {
        Ok(apollo_favorite::Entity::find()
            .filter(apollo_favorite::Column::UserId.eq(user_id))
            .order_by_desc(apollo_favorite::Column::DataChangeCreatedTime)
            .all(&self.db)
            .await?)
    }

    async fn delete_favorite(&self, id: i32, user_id: &str) -> anyhow::Result<()> {
        let db = &self.db;
        let model = apollo_favorite::Entity::find_by_id(id)
            .filter(apollo_favorite::Column::UserId.eq(user_id))
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Favorite not found: {}", id))?;
        let active_model: apollo_favorite::ActiveModel = model.into();
        active_model.delete(db).await?;
        Ok(())
    }
}

#[async_trait]
impl ServerConfigPersistence for SqlApolloPersistence {
    async fn get_server_config(&self, key: &str) -> anyhow::Result<Option<apollo_server_config::Model>> {
        Ok(apollo_server_config::Entity::find()
            .filter(apollo_server_config::Column::Key.eq(key))
            .one(&self.db)
            .await?)
    }

    async fn list_server_config(&self) -> anyhow::Result<Vec<apollo_server_config::Model>> {
        Ok(apollo_server_config::Entity::find()
            .order_by_asc(apollo_server_config::Column::Key)
            .all(&self.db)
            .await?)
    }

    async fn create_server_config(&self, dto: crate::api::dto::ServerConfigDTO) -> anyhow::Result<apollo_server_config::Model> {
        let db = &self.db;
        let existing = apollo_server_config::Entity::find()
            .filter(apollo_server_config::Column::Key.eq(&dto.key))
            .one(db)
            .await?;
        if existing.is_some() {
            anyhow::bail!("ServerConfig already exists: {}", dto.key);
        }
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_default();
        let active_model = apollo_server_config::ActiveModel {
            key: Set(dto.key),
            value: Set(dto.value),
            comment: Set(dto.comment),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            ..Default::default()
        };
        Ok(active_model.insert(db).await?)
    }

    async fn update_server_config(&self, key: &str, value: &str, operator: &str) -> anyhow::Result<()> {
        let db = &self.db;
        let model = apollo_server_config::Entity::find()
            .filter(apollo_server_config::Column::Key.eq(key))
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("ServerConfig not found: {}", key))?;
        let now = Utc::now().naive_utc();
        let mut active_model: apollo_server_config::ActiveModel = model.into();
        active_model.value = Set(value.to_string());
        active_model.data_change_last_modified_by = Set(Some(operator.to_string()));
        active_model.data_change_last_time = Set(Some(now));
        active_model.update(db).await?;
        Ok(())
    }

    async fn delete_server_config(&self, key: &str, _operator: &str) -> anyhow::Result<()> {
        let db = &self.db;
        let model = apollo_server_config::Entity::find()
            .filter(apollo_server_config::Column::Key.eq(key))
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("ServerConfig not found: {}", key))?;
        let active_model: apollo_server_config::ActiveModel = model.into();
        active_model.delete(db).await?;
        Ok(())
    }
}

#[async_trait]
impl InstanceConfigPersistence for SqlApolloPersistence {
    async fn create_or_update_instance_config(&self, dto: crate::api::dto::InstanceConfigDTO) -> anyhow::Result<apollo_instance_config::Model> {
        let db = &self.db;
        let config_app_id = dto.config_app_id.clone().unwrap_or_default();
        let existing = apollo_instance_config::Entity::find()
            .filter(apollo_instance_config::Column::InstanceId.eq(dto.instance_id))
            .filter(apollo_instance_config::Column::ConfigAppId.eq(&config_app_id))
            .filter(apollo_instance_config::Column::NamespaceName.eq(&dto.namespace_name))
            .filter(apollo_instance_config::Column::ClusterName.eq(&dto.cluster_name))
            .one(db)
            .await?;
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "system".to_string());
        if let Some(model) = existing {
            let mut active_model: apollo_instance_config::ActiveModel = model.into();
            active_model.release_key = Set(dto.release_key);
            active_model.configurations = Set(dto.configurations);
            active_model.data_change_last_modified_by = Set(Some(created_by));
            active_model.data_change_last_time = Set(Some(now));
            return Ok(active_model.update(db).await?);
        }
        let active_model = apollo_instance_config::ActiveModel {
            instance_id: Set(dto.instance_id),
            config_app_id: Set(config_app_id),
            namespace_name: Set(dto.namespace_name),
            cluster_name: Set(dto.cluster_name),
            release_key: Set(dto.release_key),
            configurations: Set(dto.configurations),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        Ok(active_model.insert(db).await?)
    }

    async fn get_instance_config_by_instance(&self, instance_id: i32) -> anyhow::Result<Vec<apollo_instance_config::Model>> {
        Ok(apollo_instance_config::Entity::find()
            .filter(apollo_instance_config::Column::InstanceId.eq(instance_id))
            .order_by_desc(apollo_instance_config::Column::DataChangeLastTime)
            .all(&self.db)
            .await?)
    }

    async fn list_instance_config_by_app_cluster(&self, _app_id: &str, cluster_name: &str, _namespace_name: &str) -> anyhow::Result<Vec<apollo_instance_config::Model>> {
        Ok(apollo_instance_config::Entity::find()
            .filter(apollo_instance_config::Column::ClusterName.eq(cluster_name))
            .order_by_desc(apollo_instance_config::Column::DataChangeLastTime)
            .all(&self.db)
            .await?)
    }

    async fn delete_instance_config_by_instance(&self, instance_id: i32) -> anyhow::Result<()> {
        apollo_instance_config::Entity::delete_many()
            .filter(apollo_instance_config::Column::InstanceId.eq(instance_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl ReleaseHistoryPersistence for SqlApolloPersistence {
    async fn find_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, page: u64, size: u64) -> anyhow::Result<(Vec<apollo_release_history::Model>, u64)> {
        let db = &self.db;
        let mut models = apollo_release_history::Entity::find()
            .filter(apollo_release_history::Column::AppId.eq(app_id))
            .filter(apollo_release_history::Column::ClusterName.eq(cluster_name))
            .filter(apollo_release_history::Column::NamespaceName.eq(namespace_name))
            .filter(apollo_release_history::Column::IsDeleted.eq(false))
            .order_by_desc(apollo_release_history::Column::DataChangeCreatedTime)
            .all(db)
            .await?;
        let total = models.len() as u64;
        let page = page.max(1);
        let size = size.max(1);
        let start = ((page - 1) * size) as usize;
        let page_items = if start >= models.len() { Vec::new() } else { models.drain(start..).take(size as usize).collect() };
        Ok((page_items, total))
    }

    async fn record_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, branch_name: &str, release_id: i32, previous_release_id: i32, operation: i16, operation_context: &str, operator: &str) -> anyhow::Result<apollo_release_history::Model> {
        let db = &self.db;
        let now = Utc::now().naive_utc();
        let active_model = apollo_release_history::ActiveModel {
            app_id: Set(app_id.to_string()),
            cluster_name: Set(cluster_name.to_string()),
            namespace_name: Set(namespace_name.to_string()),
            branch_name: Set(branch_name.to_string()),
            release_id: Set(release_id),
            previous_release_id: Set(previous_release_id),
            operation: Set(operation),
            operation_context: Set(operation_context.to_string()),
            is_deleted: Set(false),
            deleted_at: Set(0),
            data_change_created_by: Set(operator.to_string()),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        Ok(active_model.insert(db).await?)
    }
}

#[async_trait]
impl ConsumerTokenPersistence for SqlApolloPersistence {
    async fn create_consumer_token(&self, consumer_id: i32, created_by: &str) -> anyhow::Result<apollo_consumer_token::Model> {
        let db = &self.db;
        let now = Utc::now().naive_utc();
        let token = format!("{}-{}", consumer_id, now.and_utc().timestamp_millis());
        let active_model = apollo_consumer_token::ActiveModel {
            consumer_id: Set(consumer_id),
            token: Set(token),
            data_change_created_by: Set(created_by.to_string()),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        Ok(active_model.insert(db).await?)
    }

    async fn list_tokens_by_consumer(&self, consumer_id: i32) -> anyhow::Result<Vec<apollo_consumer_token::Model>> {
        let db = &self.db;
        Ok(apollo_consumer_token::Entity::find()
            .filter(apollo_consumer_token::Column::ConsumerId.eq(consumer_id))
            .all(db)
            .await?)
    }

    async fn delete_consumer_token(&self, id: i32) -> anyhow::Result<()> {
        let db = &self.db;
        let model = apollo_consumer_token::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Token not found: {}", id))?;
        let active_model: apollo_consumer_token::ActiveModel = model.into();
        active_model.delete(db).await?;
        Ok(())
    }

    async fn get_consumer_token_by_token(&self, token: &str) -> anyhow::Result<Option<apollo_consumer_token::Model>> {
        let db = &self.db;
        Ok(apollo_consumer_token::Entity::find()
            .filter(apollo_consumer_token::Column::Token.eq(token))
            .one(db)
            .await?)
    }
}
