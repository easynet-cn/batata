//! Permission types and the permission check itself.
//!
//! Upstream sources:
//! - `apollo-common/.../constants/PermissionType.java` — the fine-grained
//!   permission types guarded by `@PreAuthorize`.
//! - `apollo-portal/.../portal/service/RolePermissionService` +
//!   `PermissionService` — how a principal's roles are resolved into permissions.

use std::sync::Arc;

use crate::api::dto::UserDTO;
use crate::persistence::traits::{
    ApolloPersistenceService, ConsumerRolePersistence, RolePersistence,
};

/// Upstream `PermissionType` values.
///
/// The upstream enum also carries a `description`; only the wire values are
/// reproduced here because that is what `@PreAuthorize` expressions match on.
pub mod permission_type {
    /// Create an application.
    pub const CREATE_APPLICATION: &str = "CreateApplication";
    /// Create a cluster.
    pub const CREATE_CLUSTER: &str = "CreateCluster";
    /// Create a namespace.
    pub const CREATE_NAMESPACE: &str = "CreateNamespace";
    /// Modify a namespace.
    pub const MODIFY_NAMESPACE: &str = "ModifyNamespace";
    /// Delete a namespace.
    pub const DELETE_NAMESPACE: &str = "DeleteNamespace";
    /// Create a gray release branch.
    pub const CREATE_GRAY_RELEASE: &str = "CreateGrayRelease";
    /// Merge a gray release branch.
    pub const MERGE_GRAY_RELEASE: &str = "MergeGrayRelease";
    /// Delete a gray release branch.
    pub const DELETE_GRAY_RELEASE: &str = "DeleteGrayRelease";
    /// Delete a cluster.
    pub const DELETE_CLUSTER: &str = "DeleteCluster";
    /// Delete an application.
    pub const DELETE_APPLICATION: &str = "DeleteApplication";
    /// Assign a role to another user.
    pub const ASSIGN_ROLE: &str = "AssignRole";
    /// Manage users.
    pub const MANAGE_USER: &str = "ManageUser";
    /// Manage the open platform consumers.
    pub const MANAGE_CONSUMER: &str = "ManageConsumer";
}

/// The principal a request runs as.
///
/// Upstream distinguishes two authenticated principals: a portal user (session
/// based, from `UserInfoHolder`) and an open-platform consumer (request signed
/// with a consumer token, from `ConsumerAuthenticationFilter`). Both carry their
/// permissions through roles, so they share the same resolution path.
#[derive(Debug, Clone)]
pub enum Identity {
    /// An anonymous request. Holds no roles and therefore no permissions.
    Anonymous,
    /// A portal user identified by account name.
    User(UserDTO),
    /// An open-platform consumer identified by its `apollo_consumer.id`.
    Consumer {
        /// The consumer id.
        consumer_id: i64,
        /// The application the consumer belongs to, when configured.
        app_id: Option<String>,
    },
}

impl Identity {
    /// Whether the principal is authenticated.
    pub fn is_authenticated(&self) -> bool {
        !matches!(self, Identity::Anonymous)
    }

    /// The account name for a user principal, `None` otherwise.
    ///
    /// Upstream `UserInfoHolder.getUser().getUserId()`.
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Identity::User(user) => Some(user.username.as_str()),
            _ => None,
        }
    }
}

/// Resolves an [`Identity`] into permissions.
///
/// A permission is granted when any role attached to the principal carries it.
/// Upstream reaches the same conclusion via
/// `rolePermissionService.userHasPermission(user, permissionType, targetId)`,
/// which walks user→roles→permissions.
pub struct PermissionResolver {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl PermissionResolver {
    /// Creates a resolver backed by the given persistence service.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Resolves the principal's roles.
    ///
    /// Users are resolved through `apollo_user_role`, consumers through
    /// `apollo_consumer_role`. An anonymous principal has no roles.
    pub async fn resolve_roles(
        &self,
        identity: &Identity,
    ) -> anyhow::Result<Vec<crate::entity::apollo_role::Model>> {
        match identity {
            Identity::Anonymous => Ok(vec![]),
            Identity::User(user) => {
                <dyn RolePersistence>::list_user_roles(&self.persistence, &user.username).await
            }
            Identity::Consumer { consumer_id, .. } => {
                <dyn ConsumerRolePersistence>::list_consumer_roles(&self.persistence, *consumer_id)
                    .await
            }
        }
    }

    /// Whether the principal holds the given permission on the target.
    ///
    /// The `target_id` must match the id the permission row was created with,
    /// which upstream builds through `RoleUtils` (see [`crate::auth::role`]).
    /// Passing an empty `target_id` matches upstream's "any target" check and
    /// grants the permission when the principal holds it anywhere.
    pub async fn has_permission(
        &self,
        identity: &Identity,
        permission_type: &str,
        target_id: &str,
    ) -> anyhow::Result<bool> {
        let roles = self.resolve_roles(identity).await?;

        for role in roles {
            // The application master role is upstream's blanket grant: whoever
            // holds it may do anything within that application.
            if role.role_name
                == crate::auth::role::build_app_master_role_name(&extract_app_id(target_id))
            {
                return Ok(true);
            }

            let permission_ids =
                <dyn RolePersistence>::list_role_permissions(&self.persistence, role.id).await?;
            for permission_id in permission_ids {
                let Some(permission) =
                    crate::persistence::traits::PermissionPersistence::get_permission_by_id(
                        &self.persistence,
                        permission_id,
                    )
                    .await?
                else {
                    continue;
                };

                if permission.permission_type.to_string() == permission_type {
                    continue;
                }

                if target_id.is_empty() || permission.target_id == target_id {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }
}

/// Extracts the application id from a `RoleUtils`-built target id.
///
/// Target ids are `{appId}`, `{appId}+{namespaceName}` or
/// `{appId}+{env}+{clusterName}`, so the first segment is always the app id.
fn extract_app_id(target_id: &str) -> String {
    target_id
        .split(crate::auth::role::ROLE_NAME_SEPARATOR)
        .next()
        .unwrap_or(target_id)
        .to_string()
}
