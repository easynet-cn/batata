//! Idempotent bootstrap for Apollo portal user-management data.
//!
//! Upstream Apollo seeds an administrator, authorities and system roles at
//! startup. batata previously only seeded five `apollo_server_config` rows in
//! a SQL migration, so the embedded backend had no usable identity/RBAC data
//! and the OpenAPI permission endpoints could only return fixed responses.
//! This service seeds the same logical data through the persistence traits,
//! which keeps RocksDB, MySQL and PostgreSQL behavior identical.

use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::api::dto::{ServerConfigDTO, UserDTO};
use crate::auth::permission::{permission_type, permission_type_ids};
use crate::persistence::traits::{
    ApolloPersistenceService, AuthorityPersistence, PermissionPersistence, RolePersistence,
    ServerConfigPersistence, UserPersistence,
};

/// The target id upstream uses for system-level permissions and roles.
pub const SYSTEM_PERMISSION_TARGET_ID: &str = "SystemRole";
/// The bootstrap administrator account.
pub const DEFAULT_ADMIN_USER: &str = "apollo";

/// Seeds missing users, authorities, permissions, roles and configuration.
pub struct PortalBootstrapService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl PortalBootstrapService {
    /// Creates the bootstrap service.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Ensures all bootstrap rows exist. Safe to call on every startup.
    pub async fn ensure_initialized(&self) -> anyhow::Result<()> {
        self.seed_server_configs().await?;
        self.seed_user().await?;
        self.seed_authorities().await?;
        self.seed_system_roles().await?;
        Ok(())
    }

    async fn seed_server_configs(&self) -> anyhow::Result<()> {
        let configs = [
            (
                "superAdmin",
                DEFAULT_ADMIN_USER,
                "Portal super administrator",
            ),
            (
                "role.create-application.enabled",
                "true",
                "Enable system CreateApplication permission",
            ),
            (
                "role.manage-app-master.enabled",
                "true",
                "Enable application master management permission",
            ),
        ];
        for (key, value, comment) in configs {
            if ServerConfigPersistence::get_server_config(&self.persistence, key)
                .await?
                .is_none()
            {
                ServerConfigPersistence::create_or_update_server_config(
                    &self.persistence,
                    ServerConfigDTO {
                        key: key.to_string(),
                        value: value.to_string(),
                        comment: Some(comment.to_string()),
                        data_change_created_by: Some("system".to_string()),
                        data_change_created_time: None,
                    },
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn seed_user(&self) -> anyhow::Result<()> {
        // `apollo_users.password` is varchar(64), so store the bare
        // 64-char hex digest (upstream Apollo's SHA-256 representation).
        // A "sha256:" prefix would make this 71 chars and break SQL
        // backends on startup.
        let digest = Sha256::digest(DEFAULT_ADMIN_USER.as_bytes());
        let password = digest
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<String>();
        UserPersistence::ensure_user(
            &self.persistence,
            UserDTO {
                id: None,
                username: DEFAULT_ADMIN_USER.to_string(),
                password,
                email: Some("apollo@localhost".to_string()),
                enabled: true,
                data_change_created_by: Some("system".to_string()),
                data_change_created_time: None,
            },
        )
        .await?;
        Ok(())
    }

    async fn seed_authorities(&self) -> anyhow::Result<()> {
        for authority in ["ROLE_USER", "ROLE_ADMIN", "ROLE_SUPER_ADMIN"] {
            AuthorityPersistence::assign_authority(
                &self.persistence,
                DEFAULT_ADMIN_USER,
                authority,
            )
            .await?;
        }
        Ok(())
    }

    async fn seed_system_roles(&self) -> anyhow::Result<()> {
        let definitions = [
            (
                format!(
                    "{}|{}",
                    permission_type::CREATE_APPLICATION,
                    SYSTEM_PERMISSION_TARGET_ID
                ),
                permission_type_ids::CREATE_APPLICATION,
            ),
            (
                format!(
                    "{}|{}",
                    permission_type::MANAGE_USERS,
                    SYSTEM_PERMISSION_TARGET_ID
                ),
                permission_type_ids::MANAGE_USERS,
            ),
            (
                format!(
                    "{}|{}",
                    permission_type::MANAGE_CONSUMER,
                    SYSTEM_PERMISSION_TARGET_ID
                ),
                permission_type_ids::MANAGE_CONSUMER,
            ),
            (
                format!(
                    "{}|{}",
                    permission_type::MANAGE_APP_MASTER,
                    SYSTEM_PERMISSION_TARGET_ID
                ),
                permission_type_ids::MANAGE_APP_MASTER,
            ),
        ];

        for (role_name, permission_id) in definitions {
            let role_name = role_name.replace('|', crate::auth::role::ROLE_NAME_SEPARATOR);
            let role = RolePersistence::ensure_role(
                &self.persistence,
                crate::api::dto::RoleDTO {
                    id: None,
                    role_name: role_name.clone(),
                    role_type: 0,
                    target_id: SYSTEM_PERMISSION_TARGET_ID.to_string(),
                    data_change_created_by: Some("system".to_string()),
                    data_change_created_time: None,
                },
            )
            .await?;

            let permission = PermissionPersistence::ensure_permission(
                &self.persistence,
                permission_id,
                SYSTEM_PERMISSION_TARGET_ID,
                "system",
            )
            .await?;

            RolePersistence::ensure_role_permission(
                &self.persistence,
                role.id,
                permission.id,
                "system",
            )
            .await?;
            RolePersistence::ensure_role_to_user(
                &self.persistence,
                DEFAULT_ADMIN_USER,
                role.id,
                "system",
            )
            .await?;
        }
        Ok(())
    }
}
