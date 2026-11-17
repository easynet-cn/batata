//! Permission types and the permission check itself.
//!
//! Upstream sources:
//! - `apollo-common/.../constants/PermissionType.java` — the fine-grained
//!   permission types guarded by `@PreAuthorize`.
//! - `apollo-portal/.../portal/service/RolePermissionService` +
//!   `PermissionService` — how a principal's roles are resolved into permissions.

use std::sync::Arc;

use actix_web::HttpMessage;

use crate::api::dto::UserDTO;
use crate::auth::token_scope::TokenScope;
use crate::entity::apollo_user_token;
use crate::persistence::traits::{
    ApolloPersistenceService, ConsumerRolePersistence, RolePersistence,
};

/// Upstream `PermissionType` values.
///
/// The upstream enum also carries a `description`; only the wire values are
/// reproduced here because that is what `@PreAuthorize` expressions match on.
pub mod permission_type {
    /// Create an application (system level).
    pub const CREATE_APPLICATION: &str = "CreateApplication";
    /// Manage portal users (system level).
    pub const MANAGE_USERS: &str = "ManageUsers";
    /// Backwards compatible spelling used by older batata call sites.
    pub const MANAGE_USER: &str = MANAGE_USERS;
    /// Manage application masters (application level).
    pub const MANAGE_APP_MASTER: &str = "ManageAppMaster";
    /// Create a cluster.
    pub const CREATE_CLUSTER: &str = "CreateCluster";
    /// Create a namespace.
    pub const CREATE_NAMESPACE: &str = "CreateNamespace";
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
    /// Manage the open platform consumers.
    pub const MANAGE_CONSUMER: &str = "ManageConsumer";
    /// Modify one namespace.
    pub const MODIFY_NAMESPACE: &str = "ModifyNamespace";
    /// Release one namespace.
    pub const RELEASE_NAMESPACE: &str = "ReleaseNamespace";
    /// Modify every namespace in one cluster.
    pub const MODIFY_NAMESPACES_IN_CLUSTER: &str = "ModifyNamespacesInCluster";
    /// Release every namespace in one cluster.
    pub const RELEASE_NAMESPACES_IN_CLUSTER: &str = "ReleaseNamespacesInCluster";
}

/// Numeric ids used by batata's `apollo_permission.permission_type` column.
///
/// Upstream stores the string directly; batata stores a stable id and maps it
/// here, so every wire/application comparison goes through these helpers.
pub mod permission_type_ids {
    /// Create an application.
    pub const CREATE_APPLICATION: i32 = 1;
    /// Manage portal users.
    pub const MANAGE_USERS: i32 = 2;
    /// Manage application masters.
    pub const MANAGE_APP_MASTER: i32 = 3;
    /// Create a namespace.
    pub const CREATE_NAMESPACE: i32 = 4;
    /// Create a cluster.
    pub const CREATE_CLUSTER: i32 = 5;
    /// Assign a role.
    pub const ASSIGN_ROLE: i32 = 6;
    /// Modify one namespace.
    pub const MODIFY_NAMESPACE: i32 = 7;
    /// Release one namespace.
    pub const RELEASE_NAMESPACE: i32 = 8;
    /// Modify every namespace in a cluster.
    pub const MODIFY_NAMESPACES_IN_CLUSTER: i32 = 9;
    /// Release every namespace in a cluster.
    pub const RELEASE_NAMESPACES_IN_CLUSTER: i32 = 10;
    /// Delete a namespace.
    pub const DELETE_NAMESPACE: i32 = 11;
    /// Delete a cluster.
    pub const DELETE_CLUSTER: i32 = 12;
    /// Delete an application.
    pub const DELETE_APPLICATION: i32 = 13;
    /// Create a gray release branch.
    pub const CREATE_GRAY_RELEASE: i32 = 14;
    /// Merge a gray release branch.
    pub const MERGE_GRAY_RELEASE: i32 = 15;
    /// Delete a gray release branch.
    pub const DELETE_GRAY_RELEASE: i32 = 16;
    /// Manage open-platform consumers.
    pub const MANAGE_CONSUMER: i32 = 17;
}

/// Maps an upstream permission name to its batata numeric id.
pub fn permission_type_id(name: &str) -> Option<i32> {
    use permission_type as p;
    use permission_type_ids as id;
    Some(match name {
        p::CREATE_APPLICATION => id::CREATE_APPLICATION,
        p::MANAGE_USERS => id::MANAGE_USERS,
        p::MANAGE_APP_MASTER => id::MANAGE_APP_MASTER,
        p::CREATE_NAMESPACE => id::CREATE_NAMESPACE,
        p::CREATE_CLUSTER => id::CREATE_CLUSTER,
        p::ASSIGN_ROLE => id::ASSIGN_ROLE,
        p::MODIFY_NAMESPACE => id::MODIFY_NAMESPACE,
        p::RELEASE_NAMESPACE => id::RELEASE_NAMESPACE,
        p::MODIFY_NAMESPACES_IN_CLUSTER => id::MODIFY_NAMESPACES_IN_CLUSTER,
        p::RELEASE_NAMESPACES_IN_CLUSTER => id::RELEASE_NAMESPACES_IN_CLUSTER,
        p::DELETE_NAMESPACE => id::DELETE_NAMESPACE,
        p::DELETE_CLUSTER => id::DELETE_CLUSTER,
        p::DELETE_APPLICATION => id::DELETE_APPLICATION,
        p::CREATE_GRAY_RELEASE => id::CREATE_GRAY_RELEASE,
        p::MERGE_GRAY_RELEASE => id::MERGE_GRAY_RELEASE,
        p::DELETE_GRAY_RELEASE => id::DELETE_GRAY_RELEASE,
        p::MANAGE_CONSUMER => id::MANAGE_CONSUMER,
        _ => return None,
    })
}

/// Maps a batata numeric permission id back to its upstream name.
pub fn permission_type_name(id: i32) -> Option<&'static str> {
    use permission_type as p;
    use permission_type_ids as type_id;
    Some(match id {
        type_id::CREATE_APPLICATION => p::CREATE_APPLICATION,
        type_id::MANAGE_USERS => p::MANAGE_USERS,
        type_id::MANAGE_APP_MASTER => p::MANAGE_APP_MASTER,
        type_id::CREATE_NAMESPACE => p::CREATE_NAMESPACE,
        type_id::CREATE_CLUSTER => p::CREATE_CLUSTER,
        type_id::ASSIGN_ROLE => p::ASSIGN_ROLE,
        type_id::MODIFY_NAMESPACE => p::MODIFY_NAMESPACE,
        type_id::RELEASE_NAMESPACE => p::RELEASE_NAMESPACE,
        type_id::MODIFY_NAMESPACES_IN_CLUSTER => p::MODIFY_NAMESPACES_IN_CLUSTER,
        type_id::RELEASE_NAMESPACES_IN_CLUSTER => p::RELEASE_NAMESPACES_IN_CLUSTER,
        type_id::DELETE_NAMESPACE => p::DELETE_NAMESPACE,
        type_id::DELETE_CLUSTER => p::DELETE_CLUSTER,
        type_id::DELETE_APPLICATION => p::DELETE_APPLICATION,
        type_id::CREATE_GRAY_RELEASE => p::CREATE_GRAY_RELEASE,
        type_id::MERGE_GRAY_RELEASE => p::MERGE_GRAY_RELEASE,
        type_id::DELETE_GRAY_RELEASE => p::DELETE_GRAY_RELEASE,
        type_id::MANAGE_CONSUMER => p::MANAGE_CONSUMER,
        _ => return None,
    })
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

/// Reads the authenticated principal inserted by `OpenApiAuthMiddleware`.
///
/// Handlers fall back to `Anonymous` when a route is mounted without the
/// middleware, which keeps direct unit tests deterministic and fails closed.
pub fn current_identity(req: &actix_web::HttpRequest) -> Identity {
    req.extensions()
        .get::<Identity>()
        .cloned()
        .unwrap_or(Identity::Anonymous)
}

/// Reads the PAT scope attached by [`crate::middleware::auth::OpenApiAuthMiddleware`].
///
/// Requests without a PAT (consumer token, or legacy openapi auth disabled)
/// keep the unrestricted scope so existing callers are unaffected.
pub fn current_token_scope(req: &actix_web::HttpRequest) -> TokenScope {
    req.extensions()
        .get::<apollo_user_token::Model>()
        .map(|model| TokenScope::parse(model.scopes.as_deref()))
        .unwrap_or_else(TokenScope::allow_all)
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

                if permission_type_name(permission.permission_type) != Some(permission_type) {
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
pub fn extract_app_id(target_id: &str) -> String {
    target_id
        .split(crate::auth::role::ROLE_NAME_SEPARATOR)
        .next()
        .unwrap_or(target_id)
        .to_string()
}
