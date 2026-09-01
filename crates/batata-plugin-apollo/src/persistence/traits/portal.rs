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

use crate::entity::apollo_user_token;
use crate::persistence::traits::ApolloPersistenceService;

use crate::api::dto::{
    AppNamespaceDTO, AuditDTO, ConsumerDTO, ConsumerRoleDTO, FavoriteDTO, RoleDTO, ServerConfigDTO,
    InstanceConfigDTO, UserDTO,
};
use crate::entity::{
    apollo_app_namespace, apollo_audit, apollo_consumer, apollo_consumer_role,
    apollo_consumer_token, apollo_favorite, apollo_instance_config, apollo_permission,
    apollo_release_history, apollo_role, apollo_server_config, apollo_users,
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
    async fn get_consumer(&self, id: i64) -> anyhow::Result<Option<apollo_consumer::Model>>;
    /// Returns the requested value.
    async fn get_consumer_by_app(&self, app_id: &str) -> anyhow::Result<Option<apollo_consumer::Model>>;
    /// Returns the requested value.
    async fn list_consumers(&self) -> anyhow::Result<Vec<apollo_consumer::Model>>;
}

#[async_trait]
/// Defines the `ConsumerTokenPersistence` trait.
pub trait ConsumerTokenPersistence: Send + Sync {
    /// Creates a new resource.
    async fn create_consumer_token(&self, consumer_id: i64, created_by: &str) -> anyhow::Result<apollo_consumer_token::Model>;
    /// Returns the requested value.
    async fn list_tokens_by_consumer(&self, consumer_id: i64) -> anyhow::Result<Vec<apollo_consumer_token::Model>>;
    /// Deletes the specified resource.
    async fn delete_consumer_token(&self, id: i64) -> anyhow::Result<()>;
    /// Returns the requested value.
    async fn get_consumer_token_by_token(&self, token: &str) -> anyhow::Result<Option<apollo_consumer_token::Model>>;
}

#[async_trait]
/// Defines the `UserPersistence` trait.
///
/// Upstream stores portal users in `apollo_users` and looks them up by their
/// account name, which doubles as the primary key of the account.
pub trait UserPersistence {
    /// Creates a new user and returns the stored row.
    async fn create_user(&self, dto: UserDTO) -> anyhow::Result<apollo_users::Model>;
    /// Looks a user up by account name.
    async fn get_user(&self, username: &str) -> anyhow::Result<Option<apollo_users::Model>>;
    /// Lists every user.
    async fn list_users(&self) -> anyhow::Result<Vec<apollo_users::Model>>;
    /// Updates an existing user, returning the refreshed row.
    async fn update_user(&self, username: &str, dto: UserDTO) -> anyhow::Result<apollo_users::Model>;
    /// Deletes the specified user.
    async fn delete_user(&self, username: &str) -> anyhow::Result<()>;
}

#[async_trait]
/// Defines the `UserTokenPersistence` trait.
///
/// Upstream portal persists session / openapi user tokens in `apollo_user_token`.
/// The plaintext token is never stored: only its `token_prefix` (the first 32
/// chars, indexed for fast lookup) and `token_hash` (`sha256` of the full token)
/// are kept, mirroring `UserTokenService` in the Apollo Java portal.
pub trait UserTokenPersistence {
    /// Creates a user token, returning the stored row (without the plaintext).
    async fn create_user_token(
        &self,
        user_id: &str,
        name: &str,
        token_prefix: &str,
        token_hash: &str,
        scopes: Option<&str>,
        expires: chrono::DateTime<chrono::Utc>,
        created_by: &str,
    ) -> anyhow::Result<apollo_user_token::Model>;
    /// Looks up a token by its prefix (the first 32 chars of the plaintext).
    async fn get_user_token_by_prefix(
        &self,
        token_prefix: &str,
    ) -> anyhow::Result<Option<apollo_user_token::Model>>;
    /// Lists every (non-deleted) token of a user.
    async fn list_user_tokens(&self, user_id: &str) -> anyhow::Result<Vec<apollo_user_token::Model>>;
    /// Soft-deletes a token by id.
    async fn delete_user_token(&self, id: i64) -> anyhow::Result<()>;
}

#[async_trait]
/// Defines the `ConsumerRolePersistence` trait.
///
/// Upstream `ConsumerRole` is what grants an OpenAPI consumer its permissions:
/// a consumer without any linked role can authenticate but is not authorized to
/// do anything.
pub trait ConsumerRolePersistence {
    /// Grants a role to a consumer.
    async fn assign_role_to_consumer(
        &self,
        consumer_id: i64,
        role_id: i64,
        created_by: &str,
    ) -> anyhow::Result<()>;
    /// Revokes a role from a consumer.
    async fn remove_role_from_consumer(&self, consumer_id: i64, role_id: i64)
        -> anyhow::Result<()>;
    /// Lists every role granted to a consumer.
    async fn list_consumer_roles(&self, consumer_id: i64) -> anyhow::Result<Vec<apollo_role::Model>>;
    /// Lists every stored consumer-role link.
    async fn list_consumer_role_links(&self) -> anyhow::Result<Vec<apollo_consumer_role::Model>>;
    /// Creates a consumer-role link from a DTO, returning the stored row.
    async fn create_consumer_role(
        &self,
        dto: ConsumerRoleDTO,
    ) -> anyhow::Result<apollo_consumer_role::Model>;
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
    /// Looks up a single permission by its primary key.
    async fn get_permission_by_id(&self, id: i64) -> anyhow::Result<Option<apollo_permission::Model>>;
}

#[async_trait]
/// Defines the `RolePersistence` trait.
pub trait RolePersistence {
    /// Creates a new resource.
    async fn create_role(&self, dto: RoleDTO) -> anyhow::Result<apollo_role::Model>;
    /// Returns the requested value.
    async fn get_role(&self, id: i64) -> anyhow::Result<Option<apollo_role::Model>>;
    /// Returns the requested value.
    async fn list_role_by_target(&self, target_id: &str) -> anyhow::Result<Vec<apollo_role::Model>>;
    /// Looks a role up by its name.
    ///
    /// Upstream `roleService.findRoleByRoleName`; role names are the join point
    /// between `RoleUtils` naming rules and the stored rows.
    async fn get_role_by_name(&self, role_name: &str) -> anyhow::Result<Option<apollo_role::Model>>;
    /// Deletes the specified resource.
    async fn delete_role(&self, id: i64) -> anyhow::Result<()>;
    /// Performs the `assign_role_permission` operation.
    async fn assign_role_permission(&self, role_id: i64, permission_id: i64, created_by: &str) -> anyhow::Result<()>;
    /// Deletes the specified resource.
    async fn remove_role_permission(&self, role_id: i64, permission_id: i64) -> anyhow::Result<()>;
    /// Returns the requested value.
    async fn list_role_permissions(&self, role_id: i64) -> anyhow::Result<Vec<i64>>;
    /// Performs the `assign_role_to_user` operation.
    async fn assign_role_to_user(&self, user_id: &str, role_id: i64, created_by: &str) -> anyhow::Result<()>;
    /// Deletes the specified resource.
    async fn remove_role_from_user(&self, user_id: &str, role_id: i64) -> anyhow::Result<()>;
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
    async fn delete_favorite(&self, id: i64, user_id: &str) -> anyhow::Result<()>;
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
    async fn get_instance_config_by_instance(&self, instance_id: i64) -> anyhow::Result<Vec<apollo_instance_config::Model>>;
    /// Returns the requested value.
    async fn list_instance_config_by_app_cluster(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Vec<apollo_instance_config::Model>>;
    /// Deletes the specified resource.
    async fn delete_instance_config_by_instance(&self, instance_id: i64) -> anyhow::Result<()>;
    /// Lists instance configs of a namespace, paged and optionally filtered by
    /// the owning `config_app_id` (upstream `instanceAppId`).
    ///
    /// Returns the page content together with the total number of matching rows
    /// (used to build the `OpenInstancePageDTO`). `page`/`size` are 1-based.
    async fn list_instance_config_paged(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
        config_app_id: Option<&str>,
        page: u64,
        size: u64,
    ) -> anyhow::Result<(Vec<apollo_instance_config::Model>, u64)>;
    /// Counts the distinct instance configs of a namespace.
    async fn count_instance_config(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<u64>;
    /// Lists instance configs whose `release_key` is one of the provided keys
    /// (upstream `findByReleaseKeys`). Used by the `by-release` endpoint.
    async fn list_instance_config_by_release_keys(&self, release_keys: &[String]) -> anyhow::Result<Vec<apollo_instance_config::Model>>;
    /// Lists instance configs of a namespace whose `release_key` is NOT one of
    /// the provided keys (upstream `findByReleaseKeysNotIn`). Used by the
    /// `by-release-not-in` endpoint.
    async fn list_instance_config_by_release_keys_not_in(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
        release_keys: &[String],
    ) -> anyhow::Result<Vec<apollo_instance_config::Model>>;
}

#[async_trait]
/// Defines the `ReleaseHistoryPersistence` trait.
pub trait ReleaseHistoryPersistence {
    /// Returns the requested value.
    async fn find_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, page: u64, size: u64) -> anyhow::Result<(Vec<apollo_release_history::Model>, u64)>;
    /// Performs the `record_release_history` operation.
    async fn record_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, branch_name: &str, release_id: i64, previous_release_id: i64, operation: i32, operation_context: &str, operator: &str) -> anyhow::Result<apollo_release_history::Model>;
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
    async fn get_consumer(&self, id: i64) -> anyhow::Result<Option<apollo_consumer::Model>> {
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
    async fn create_consumer_token(&self, consumer_id: i64, created_by: &str) -> anyhow::Result<apollo_consumer_token::Model> {
        ConsumerTokenPersistence::create_consumer_token(&**self, consumer_id, created_by).await
    }
    async fn list_tokens_by_consumer(&self, consumer_id: i64) -> anyhow::Result<Vec<apollo_consumer_token::Model>> {
        ConsumerTokenPersistence::list_tokens_by_consumer(&**self, consumer_id).await
    }
    async fn delete_consumer_token(&self, id: i64) -> anyhow::Result<()> {
        ConsumerTokenPersistence::delete_consumer_token(&**self, id).await
    }
    async fn get_consumer_token_by_token(&self, token: &str) -> anyhow::Result<Option<apollo_consumer_token::Model>> {
        ConsumerTokenPersistence::get_consumer_token_by_token(&**self, token).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> UserPersistence for Arc<T> {
    async fn create_user(&self, dto: UserDTO) -> anyhow::Result<apollo_users::Model> {
        UserPersistence::create_user(&**self, dto).await
    }
    async fn get_user(&self, username: &str) -> anyhow::Result<Option<apollo_users::Model>> {
        UserPersistence::get_user(&**self, username).await
    }
    async fn list_users(&self) -> anyhow::Result<Vec<apollo_users::Model>> {
        UserPersistence::list_users(&**self).await
    }
    async fn update_user(&self, username: &str, dto: UserDTO) -> anyhow::Result<apollo_users::Model> {
        UserPersistence::update_user(&**self, username, dto).await
    }
    async fn delete_user(&self, username: &str) -> anyhow::Result<()> {
        UserPersistence::delete_user(&**self, username).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> ConsumerRolePersistence for Arc<T> {
    async fn assign_role_to_consumer(
        &self,
        consumer_id: i64,
        role_id: i64,
        created_by: &str,
    ) -> anyhow::Result<()> {
        ConsumerRolePersistence::assign_role_to_consumer(&**self, consumer_id, role_id, created_by)
            .await
    }
    async fn remove_role_from_consumer(
        &self,
        consumer_id: i64,
        role_id: i64,
    ) -> anyhow::Result<()> {
        ConsumerRolePersistence::remove_role_from_consumer(&**self, consumer_id, role_id).await
    }
    async fn list_consumer_roles(
        &self,
        consumer_id: i64,
    ) -> anyhow::Result<Vec<apollo_role::Model>> {
        ConsumerRolePersistence::list_consumer_roles(&**self, consumer_id).await
    }
    async fn list_consumer_role_links(&self) -> anyhow::Result<Vec<apollo_consumer_role::Model>> {
        ConsumerRolePersistence::list_consumer_role_links(&**self).await
    }
    async fn create_consumer_role(
        &self,
        dto: ConsumerRoleDTO,
    ) -> anyhow::Result<apollo_consumer_role::Model> {
        ConsumerRolePersistence::create_consumer_role(&**self, dto).await
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
    async fn get_permission_by_id(&self, id: i64) -> anyhow::Result<Option<apollo_permission::Model>> {
        PermissionPersistence::get_permission_by_id(&**self, id).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> RolePersistence for Arc<T> {
    async fn create_role(&self, dto: RoleDTO) -> anyhow::Result<apollo_role::Model> {
        RolePersistence::create_role(&**self, dto).await
    }
    async fn get_role(&self, id: i64) -> anyhow::Result<Option<apollo_role::Model>> {
        RolePersistence::get_role(&**self, id).await
    }
    async fn list_role_by_target(&self, target_id: &str) -> anyhow::Result<Vec<apollo_role::Model>> {
        RolePersistence::list_role_by_target(&**self, target_id).await
    }
    async fn get_role_by_name(&self, role_name: &str) -> anyhow::Result<Option<apollo_role::Model>> {
        RolePersistence::get_role_by_name(&**self, role_name).await
    }
    async fn delete_role(&self, id: i64) -> anyhow::Result<()> {
        RolePersistence::delete_role(&**self, id).await
    }
    async fn assign_role_permission(&self, role_id: i64, permission_id: i64, created_by: &str) -> anyhow::Result<()> {
        RolePersistence::assign_role_permission(&**self, role_id, permission_id, created_by).await
    }
    async fn remove_role_permission(&self, role_id: i64, permission_id: i64) -> anyhow::Result<()> {
        RolePersistence::remove_role_permission(&**self, role_id, permission_id).await
    }
    async fn list_role_permissions(&self, role_id: i64) -> anyhow::Result<Vec<i64>> {
        RolePersistence::list_role_permissions(&**self, role_id).await
    }
    async fn assign_role_to_user(&self, user_id: &str, role_id: i64, created_by: &str) -> anyhow::Result<()> {
        RolePersistence::assign_role_to_user(&**self, user_id, role_id, created_by).await
    }
    async fn remove_role_from_user(&self, user_id: &str, role_id: i64) -> anyhow::Result<()> {
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
    async fn delete_favorite(&self, id: i64, user_id: &str) -> anyhow::Result<()> {
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
    async fn get_instance_config_by_instance(&self, instance_id: i64) -> anyhow::Result<Vec<apollo_instance_config::Model>> {
        InstanceConfigPersistence::get_instance_config_by_instance(&**self, instance_id).await
    }
    async fn list_instance_config_by_app_cluster(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Vec<apollo_instance_config::Model>> {
        InstanceConfigPersistence::list_instance_config_by_app_cluster(&**self, app_id, cluster_name, namespace_name).await
    }
    async fn delete_instance_config_by_instance(&self, instance_id: i64) -> anyhow::Result<()> {
        InstanceConfigPersistence::delete_instance_config_by_instance(&**self, instance_id).await
    }
    async fn list_instance_config_paged(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
        config_app_id: Option<&str>,
        page: u64,
        size: u64,
    ) -> anyhow::Result<(Vec<apollo_instance_config::Model>, u64)> {
        InstanceConfigPersistence::list_instance_config_paged(&**self, app_id, cluster_name, namespace_name, config_app_id, page, size).await
    }
    async fn count_instance_config(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<u64> {
        InstanceConfigPersistence::count_instance_config(&**self, app_id, cluster_name, namespace_name).await
    }
    async fn list_instance_config_by_release_keys(
        &self,
        release_keys: &[String],
    ) -> anyhow::Result<Vec<apollo_instance_config::Model>> {
        InstanceConfigPersistence::list_instance_config_by_release_keys(&**self, release_keys).await
    }
    async fn list_instance_config_by_release_keys_not_in(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
        release_keys: &[String],
    ) -> anyhow::Result<Vec<apollo_instance_config::Model>> {
        InstanceConfigPersistence::list_instance_config_by_release_keys_not_in(&**self, app_id, cluster_name, namespace_name, release_keys).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> ReleaseHistoryPersistence for Arc<T> {
    async fn find_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, page: u64, size: u64) -> anyhow::Result<(Vec<apollo_release_history::Model>, u64)> {
        ReleaseHistoryPersistence::find_release_history(&**self, app_id, cluster_name, namespace_name, page, size).await
    }
    async fn record_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, branch_name: &str, release_id: i64, previous_release_id: i64, operation: i32, operation_context: &str, operator: &str) -> anyhow::Result<apollo_release_history::Model> {
        ReleaseHistoryPersistence::record_release_history(&**self, app_id, cluster_name, namespace_name, branch_name, release_id, previous_release_id, operation, operation_context, operator).await
    }
}
