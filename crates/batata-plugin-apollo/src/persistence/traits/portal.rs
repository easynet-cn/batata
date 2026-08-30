//! Portal/admin persistence traits for Apollo (RocksDB + SQL backends).
//!
//! These cover the entities that the original implementation accessed through
//! raw `sea_orm` queries inside the services. By routing them through traits
//! with both embedded (RocksDB) and SQL implementations, the Apollo plugin
//! works consistently on all three backends (RocksDB / MySQL / PostgreSQL).
//!
//! Trait methods return the raw `sea_orm` `Model`; the services keep their
//! `model_to_dto` conversions.

use async_trait::async_trait;
use std::sync::Arc;

use crate::persistence::traits::ApolloPersistenceService;

use crate::api::dto::{
    AppNamespaceDTO, AuditDTO, ConsumerDTO, FavoriteDTO, RoleDTO, ServerConfigDTO, InstanceConfigDTO,
};
use crate::entity::{
    apollo_app_namespace, apollo_audit, apollo_consumer, apollo_consumer_token, apollo_favorite,
    apollo_instance_config, apollo_permission, apollo_release_history, apollo_role,
    apollo_server_config,
};

#[async_trait]
/// Defines the `AppNamespacePersistence` trait.
pub trait AppNamespacePersistence {
    /// Creates a new resource.
    async fn create_app_namespace(&self, dto: AppNamespaceDTO) -> anyhow::Result<apollo_app_namespace::Model>;
    /// Returns the requested value.
    async fn get_app_namespace(&self, app_id: &str, name: &str) -> anyhow::Result<Option<apollo_app_namespace::Model>>;
    /// Returns the requested value.
    async fn list_app_namespace_by_app(&self, app_id: &str) -> anyhow::Result<Vec<apollo_app_namespace::Model>>;
    /// Returns the requested value.
    async fn list_public_app_namespace(&self) -> anyhow::Result<Vec<apollo_app_namespace::Model>>;
    /// Deletes the specified resource.
    async fn delete_app_namespace(&self, app_id: &str, name: &str, operator: &str) -> anyhow::Result<()>;
}

#[async_trait]
/// Defines the `AuditPersistence` trait.
pub trait AuditPersistence {
    /// Creates a new resource.
    async fn create_audit(&self, dto: AuditDTO) -> anyhow::Result<apollo_audit::Model>;
    /// Returns the requested value.
    async fn list_audit(&self, page: u64, size: u64) -> anyhow::Result<(Vec<apollo_audit::Model>, u64)>;
    /// Returns the requested value.
    async fn list_audit_by_entity(&self, entity_name: &str, entity_id: &str) -> anyhow::Result<Vec<apollo_audit::Model>>;
}

#[async_trait]
/// Defines the `ConsumerPersistence` trait.
pub trait ConsumerPersistence {
    /// Creates a new resource.
    async fn create_consumer(&self, dto: ConsumerDTO) -> anyhow::Result<apollo_consumer::Model>;
    /// Returns the requested value.
    async fn get_consumer(&self, id: i32) -> anyhow::Result<Option<apollo_consumer::Model>>;
    /// Returns the requested value.
    async fn get_consumer_by_app(&self, app_id: &str) -> anyhow::Result<Option<apollo_consumer::Model>>;
    /// Returns the requested value.
    async fn list_consumers(&self) -> anyhow::Result<Vec<apollo_consumer::Model>>;
}

#[async_trait]
/// Defines the `ConsumerTokenPersistence` trait.
pub trait ConsumerTokenPersistence: Send + Sync {
    /// Creates a new resource.
    async fn create_consumer_token(&self, consumer_id: i32, created_by: &str) -> anyhow::Result<apollo_consumer_token::Model>;
    /// Returns the requested value.
    async fn list_tokens_by_consumer(&self, consumer_id: i32) -> anyhow::Result<Vec<apollo_consumer_token::Model>>;
    /// Deletes the specified resource.
    async fn delete_consumer_token(&self, id: i32) -> anyhow::Result<()>;
    /// Returns the requested value.
    async fn get_consumer_token_by_token(&self, token: &str) -> anyhow::Result<Option<apollo_consumer_token::Model>>;
}

#[async_trait]
/// Defines the `PermissionPersistence` trait.
pub trait PermissionPersistence {
    /// Creates a new resource.
    async fn create_permission(&self, permission_type: i32, target_id: &str, created_by: &str) -> anyhow::Result<apollo_permission::Model>;
    /// Returns the requested value.
    async fn list_permission_by_target(&self, target_id: &str) -> anyhow::Result<Vec<apollo_permission::Model>>;
    /// Returns the requested value.
    async fn list_permission_by_type(&self, permission_type: i32) -> anyhow::Result<Vec<apollo_permission::Model>>;
}

#[async_trait]
/// Defines the `RolePersistence` trait.
pub trait RolePersistence {
    /// Creates a new resource.
    async fn create_role(&self, dto: RoleDTO) -> anyhow::Result<apollo_role::Model>;
    /// Returns the requested value.
    async fn get_role(&self, id: i32) -> anyhow::Result<Option<apollo_role::Model>>;
    /// Returns the requested value.
    async fn list_role_by_target(&self, target_id: &str) -> anyhow::Result<Vec<apollo_role::Model>>;
    /// Deletes the specified resource.
    async fn delete_role(&self, id: i32) -> anyhow::Result<()>;
    /// Performs the `assign_role_permission` operation.
    async fn assign_role_permission(&self, role_id: i32, permission_id: i32, created_by: &str) -> anyhow::Result<()>;
    /// Deletes the specified resource.
    async fn remove_role_permission(&self, role_id: i32, permission_id: i32) -> anyhow::Result<()>;
    /// Returns the requested value.
    async fn list_role_permissions(&self, role_id: i32) -> anyhow::Result<Vec<i32>>;
    /// Performs the `assign_role_to_user` operation.
    async fn assign_role_to_user(&self, user_id: &str, role_id: i32, created_by: &str) -> anyhow::Result<()>;
    /// Deletes the specified resource.
    async fn remove_role_from_user(&self, user_id: &str, role_id: i32) -> anyhow::Result<()>;
    /// Returns the requested value.
    async fn list_user_roles(&self, user_id: &str) -> anyhow::Result<Vec<apollo_role::Model>>;
}

#[async_trait]
/// Defines the `FavoritePersistence` trait.
pub trait FavoritePersistence {
    /// Creates a new resource.
    async fn create_favorite(&self, dto: FavoriteDTO) -> anyhow::Result<apollo_favorite::Model>;
    /// Returns the requested value.
    async fn list_favorite_by_user(&self, user_id: &str) -> anyhow::Result<Vec<apollo_favorite::Model>>;
    /// Deletes the specified resource.
    async fn delete_favorite(&self, id: i32, user_id: &str) -> anyhow::Result<()>;
}

#[async_trait]
/// Defines the `ServerConfigPersistence` trait.
pub trait ServerConfigPersistence {
    /// Returns the requested value.
    async fn get_server_config(&self, key: &str) -> anyhow::Result<Option<apollo_server_config::Model>>;
    /// Returns the requested value.
    async fn list_server_config(&self) -> anyhow::Result<Vec<apollo_server_config::Model>>;
    /// Creates a new resource.
    async fn create_server_config(&self, dto: ServerConfigDTO) -> anyhow::Result<apollo_server_config::Model>;
    /// Updates an existing resource.
    async fn update_server_config(&self, key: &str, value: &str, operator: &str) -> anyhow::Result<()>;
    /// Deletes the specified resource.
    async fn delete_server_config(&self, key: &str, operator: &str) -> anyhow::Result<()>;
}

#[async_trait]
/// Defines the `InstanceConfigPersistence` trait.
pub trait InstanceConfigPersistence {
    /// Creates a new resource.
    async fn create_or_update_instance_config(&self, dto: InstanceConfigDTO) -> anyhow::Result<apollo_instance_config::Model>;
    /// Returns the requested value.
    async fn get_instance_config_by_instance(&self, instance_id: i32) -> anyhow::Result<Vec<apollo_instance_config::Model>>;
    /// Returns the requested value.
    async fn list_instance_config_by_app_cluster(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Vec<apollo_instance_config::Model>>;
    /// Deletes the specified resource.
    async fn delete_instance_config_by_instance(&self, instance_id: i32) -> anyhow::Result<()>;
}

#[async_trait]
/// Defines the `ReleaseHistoryPersistence` trait.
pub trait ReleaseHistoryPersistence {
    /// Returns the requested value.
    async fn find_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, page: u64, size: u64) -> anyhow::Result<(Vec<apollo_release_history::Model>, u64)>;
    /// Performs the `record_release_history` operation.
    async fn record_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, branch_name: &str, release_id: i32, previous_release_id: i32, operation: i16, operation_context: &str, operator: &str) -> anyhow::Result<apollo_release_history::Model>;
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> AppNamespacePersistence for Arc<T> {
    async fn create_app_namespace(&self, dto: AppNamespaceDTO) -> anyhow::Result<apollo_app_namespace::Model> {
        AppNamespacePersistence::create_app_namespace(&**self, dto).await
    }
    async fn get_app_namespace(&self, app_id: &str, name: &str) -> anyhow::Result<Option<apollo_app_namespace::Model>> {
        AppNamespacePersistence::get_app_namespace(&**self, app_id, name).await
    }
    async fn list_app_namespace_by_app(&self, app_id: &str) -> anyhow::Result<Vec<apollo_app_namespace::Model>> {
        AppNamespacePersistence::list_app_namespace_by_app(&**self, app_id).await
    }
    async fn list_public_app_namespace(&self) -> anyhow::Result<Vec<apollo_app_namespace::Model>> {
        AppNamespacePersistence::list_public_app_namespace(&**self).await
    }
    async fn delete_app_namespace(&self, app_id: &str, name: &str, operator: &str) -> anyhow::Result<()> {
        AppNamespacePersistence::delete_app_namespace(&**self, app_id, name, operator).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> AuditPersistence for Arc<T> {
    async fn create_audit(&self, dto: AuditDTO) -> anyhow::Result<apollo_audit::Model> {
        AuditPersistence::create_audit(&**self, dto).await
    }
    async fn list_audit(&self, page: u64, size: u64) -> anyhow::Result<(Vec<apollo_audit::Model>, u64)> {
        AuditPersistence::list_audit(&**self, page, size).await
    }
    async fn list_audit_by_entity(&self, entity_name: &str, entity_id: &str) -> anyhow::Result<Vec<apollo_audit::Model>> {
        AuditPersistence::list_audit_by_entity(&**self, entity_name, entity_id).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> ConsumerPersistence for Arc<T> {
    async fn create_consumer(&self, dto: ConsumerDTO) -> anyhow::Result<apollo_consumer::Model> {
        ConsumerPersistence::create_consumer(&**self, dto).await
    }
    async fn get_consumer(&self, id: i32) -> anyhow::Result<Option<apollo_consumer::Model>> {
        ConsumerPersistence::get_consumer(&**self, id).await
    }
    async fn get_consumer_by_app(&self, app_id: &str) -> anyhow::Result<Option<apollo_consumer::Model>> {
        ConsumerPersistence::get_consumer_by_app(&**self, app_id).await
    }
    async fn list_consumers(&self) -> anyhow::Result<Vec<apollo_consumer::Model>> {
        ConsumerPersistence::list_consumers(&**self).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> ConsumerTokenPersistence for Arc<T> {
    async fn create_consumer_token(&self, consumer_id: i32, created_by: &str) -> anyhow::Result<apollo_consumer_token::Model> {
        ConsumerTokenPersistence::create_consumer_token(&**self, consumer_id, created_by).await
    }
    async fn list_tokens_by_consumer(&self, consumer_id: i32) -> anyhow::Result<Vec<apollo_consumer_token::Model>> {
        ConsumerTokenPersistence::list_tokens_by_consumer(&**self, consumer_id).await
    }
    async fn delete_consumer_token(&self, id: i32) -> anyhow::Result<()> {
        ConsumerTokenPersistence::delete_consumer_token(&**self, id).await
    }
    async fn get_consumer_token_by_token(&self, token: &str) -> anyhow::Result<Option<apollo_consumer_token::Model>> {
        ConsumerTokenPersistence::get_consumer_token_by_token(&**self, token).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> PermissionPersistence for Arc<T> {
    async fn create_permission(&self, permission_type: i32, target_id: &str, created_by: &str) -> anyhow::Result<apollo_permission::Model> {
        PermissionPersistence::create_permission(&**self, permission_type, target_id, created_by).await
    }
    async fn list_permission_by_target(&self, target_id: &str) -> anyhow::Result<Vec<apollo_permission::Model>> {
        PermissionPersistence::list_permission_by_target(&**self, target_id).await
    }
    async fn list_permission_by_type(&self, permission_type: i32) -> anyhow::Result<Vec<apollo_permission::Model>> {
        PermissionPersistence::list_permission_by_type(&**self, permission_type).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> RolePersistence for Arc<T> {
    async fn create_role(&self, dto: RoleDTO) -> anyhow::Result<apollo_role::Model> {
        RolePersistence::create_role(&**self, dto).await
    }
    async fn get_role(&self, id: i32) -> anyhow::Result<Option<apollo_role::Model>> {
        RolePersistence::get_role(&**self, id).await
    }
    async fn list_role_by_target(&self, target_id: &str) -> anyhow::Result<Vec<apollo_role::Model>> {
        RolePersistence::list_role_by_target(&**self, target_id).await
    }
    async fn delete_role(&self, id: i32) -> anyhow::Result<()> {
        RolePersistence::delete_role(&**self, id).await
    }
    async fn assign_role_permission(&self, role_id: i32, permission_id: i32, created_by: &str) -> anyhow::Result<()> {
        RolePersistence::assign_role_permission(&**self, role_id, permission_id, created_by).await
    }
    async fn remove_role_permission(&self, role_id: i32, permission_id: i32) -> anyhow::Result<()> {
        RolePersistence::remove_role_permission(&**self, role_id, permission_id).await
    }
    async fn list_role_permissions(&self, role_id: i32) -> anyhow::Result<Vec<i32>> {
        RolePersistence::list_role_permissions(&**self, role_id).await
    }
    async fn assign_role_to_user(&self, user_id: &str, role_id: i32, created_by: &str) -> anyhow::Result<()> {
        RolePersistence::assign_role_to_user(&**self, user_id, role_id, created_by).await
    }
    async fn remove_role_from_user(&self, user_id: &str, role_id: i32) -> anyhow::Result<()> {
        RolePersistence::remove_role_from_user(&**self, user_id, role_id).await
    }
    async fn list_user_roles(&self, user_id: &str) -> anyhow::Result<Vec<apollo_role::Model>> {
        RolePersistence::list_user_roles(&**self, user_id).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> FavoritePersistence for Arc<T> {
    async fn create_favorite(&self, dto: FavoriteDTO) -> anyhow::Result<apollo_favorite::Model> {
        FavoritePersistence::create_favorite(&**self, dto).await
    }
    async fn list_favorite_by_user(&self, user_id: &str) -> anyhow::Result<Vec<apollo_favorite::Model>> {
        FavoritePersistence::list_favorite_by_user(&**self, user_id).await
    }
    async fn delete_favorite(&self, id: i32, user_id: &str) -> anyhow::Result<()> {
        FavoritePersistence::delete_favorite(&**self, id, user_id).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> ServerConfigPersistence for Arc<T> {
    async fn get_server_config(&self, key: &str) -> anyhow::Result<Option<apollo_server_config::Model>> {
        ServerConfigPersistence::get_server_config(&**self, key).await
    }
    async fn list_server_config(&self) -> anyhow::Result<Vec<apollo_server_config::Model>> {
        ServerConfigPersistence::list_server_config(&**self).await
    }
    async fn create_server_config(&self, dto: ServerConfigDTO) -> anyhow::Result<apollo_server_config::Model> {
        ServerConfigPersistence::create_server_config(&**self, dto).await
    }
    async fn update_server_config(&self, key: &str, value: &str, operator: &str) -> anyhow::Result<()> {
        ServerConfigPersistence::update_server_config(&**self, key, value, operator).await
    }
    async fn delete_server_config(&self, key: &str, operator: &str) -> anyhow::Result<()> {
        ServerConfigPersistence::delete_server_config(&**self, key, operator).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> InstanceConfigPersistence for Arc<T> {
    async fn create_or_update_instance_config(&self, dto: InstanceConfigDTO) -> anyhow::Result<apollo_instance_config::Model> {
        InstanceConfigPersistence::create_or_update_instance_config(&**self, dto).await
    }
    async fn get_instance_config_by_instance(&self, instance_id: i32) -> anyhow::Result<Vec<apollo_instance_config::Model>> {
        InstanceConfigPersistence::get_instance_config_by_instance(&**self, instance_id).await
    }
    async fn list_instance_config_by_app_cluster(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Vec<apollo_instance_config::Model>> {
        InstanceConfigPersistence::list_instance_config_by_app_cluster(&**self, app_id, cluster_name, namespace_name).await
    }
    async fn delete_instance_config_by_instance(&self, instance_id: i32) -> anyhow::Result<()> {
        InstanceConfigPersistence::delete_instance_config_by_instance(&**self, instance_id).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> ReleaseHistoryPersistence for Arc<T> {
    async fn find_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, page: u64, size: u64) -> anyhow::Result<(Vec<apollo_release_history::Model>, u64)> {
        ReleaseHistoryPersistence::find_release_history(&**self, app_id, cluster_name, namespace_name, page, size).await
    }
    async fn record_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, branch_name: &str, release_id: i32, previous_release_id: i32, operation: i16, operation_context: &str, operator: &str) -> anyhow::Result<apollo_release_history::Model> {
        ReleaseHistoryPersistence::record_release_history(&**self, app_id, cluster_name, namespace_name, branch_name, release_id, previous_release_id, operation, operation_context, operator).await
    }
}
