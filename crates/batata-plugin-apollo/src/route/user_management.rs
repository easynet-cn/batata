//! Shared helpers for Apollo user/role/permission HTTP handlers.

use std::sync::Arc;

use actix_web::HttpRequest;
use serde_json::{Value, json};

use crate::api::dto::{NewAuditLog, NewAuditLogDataInfluence, RoleDTO};
use crate::auth::permission::{
    Identity, PermissionResolver, current_identity, current_token_scope, permission_type,
};
use crate::auth::token_scope::{TokenScope, operation, operation_for_permission};
use crate::persistence::traits::{
    ApolloPersistenceService, AuthorityPersistence, PermissionPersistence, RolePersistence,
    ServerConfigPersistence, UserPersistence,
};
use crate::service::{AuditLogService, RoleService};

/// Upstream system-level permission target id.
pub const SYSTEM_TARGET: &str = "SystemRole";

/// Resolves the operator id using upstream `OpenApiOperatorResolver` rules.
///
/// USER / USER_TOKEN identities act as themselves. A CONSUMER must name an
/// existing operator explicitly.
pub async fn resolve_operator(
    persistence: &Arc<dyn ApolloPersistenceService>,
    identity: &Identity,
    explicit: Option<&str>,
) -> anyhow::Result<String> {
    match identity {
        Identity::User(user) => Ok(user.username.clone()),
        Identity::Consumer { .. } => {
            let operator = explicit
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("consumer requests require an operator"))?;
            if UserPersistence::get_user(&**persistence, operator)
                .await?
                .is_none()
            {
                anyhow::bail!("operator does not exist: {}", operator);
            }
            Ok(operator.to_string())
        }
        Identity::Anonymous => anyhow::bail!("authentication required"),
    }
}

/// Whether the principal is Apollo's super administrator.
///
/// When a PAT authenticated the request, upstream requires both the user's
/// super-admin role and the token's `system:admin` operation.
pub async fn is_super_admin_with_scope(
    persistence: &Arc<dyn ApolloPersistenceService>,
    identity: &Identity,
    scope: Option<&TokenScope>,
) -> anyhow::Result<bool> {
    if scope.is_some_and(|scope| !scope.allows_operation(operation::SYSTEM_ADMIN)) {
        return Ok(false);
    }
    is_rbac_super_admin(persistence, identity).await
}

/// Identity-only superadmin check used by permission resolution.
///
/// Upstream `UserPermissionValidator` grants every RBAC permission to a
/// superadmin; the PAT scope is applied separately and must still hold.
pub async fn is_rbac_super_admin(
    persistence: &Arc<dyn ApolloPersistenceService>,
    identity: &Identity,
) -> anyhow::Result<bool> {
    let Some(username) = identity.user_id() else {
        return Ok(false);
    };
    if AuthorityPersistence::has_authority(&**persistence, username, "ROLE_SUPER_ADMIN").await? {
        return Ok(true);
    }
    Ok(
        ServerConfigPersistence::get_server_config(&**persistence, "superAdmin")
            .await?
            .is_some_and(|config| config.value == username),
    )
}

/// Convenience wrapper for callers without request scope information.
pub async fn is_super_admin(
    persistence: &Arc<dyn ApolloPersistenceService>,
    identity: &Identity,
) -> anyhow::Result<bool> {
    is_super_admin_with_scope(persistence, identity, None).await
}

/// Request-aware superadmin check including the PAT scope.
pub async fn request_is_super_admin(
    req: &HttpRequest,
    persistence: &Arc<dyn ApolloPersistenceService>,
) -> anyhow::Result<bool> {
    let scope = current_token_scope(req);
    is_super_admin_with_scope(persistence, &current_identity(req), Some(&scope)).await
}

/// Whether the principal holds a permission, with superadmin as a blanket grant.
///
/// A PAT's operation/app restrictions are ANDed with the user's RBAC result,
/// mirroring upstream `UserTokenPermissionValidator`.
pub async fn has_permission_with_scope(
    persistence: &Arc<dyn ApolloPersistenceService>,
    identity: &Identity,
    scope: Option<&TokenScope>,
    permission: &str,
    target_id: &str,
) -> anyhow::Result<bool> {
    // Token scope is always applied, including for superadmins; upstream's
    // UserTokenPermissionValidator ANDs it with the RBAC result.
    if let Some(scope) = scope {
        let Some(operation) = operation_for_permission(permission) else {
            return Ok(false);
        };
        if !scope.allows_operation(operation)
            || !scope.allows_permission_target(permission, target_id)
        {
            return Ok(false);
        }
    }
    if is_rbac_super_admin(persistence, identity).await? {
        return Ok(true);
    }
    PermissionResolver::new(persistence.clone())
        .has_permission(identity, permission, target_id)
        .await
}

/// Convenience wrapper for callers without request scope information.
pub async fn has_permission(
    persistence: &Arc<dyn ApolloPersistenceService>,
    identity: &Identity,
    permission: &str,
    target_id: &str,
) -> anyhow::Result<bool> {
    has_permission_with_scope(persistence, identity, None, permission, target_id).await
}

/// Request-aware permission check including the PAT scope.
pub async fn request_has_permission(
    req: &HttpRequest,
    persistence: &Arc<dyn ApolloPersistenceService>,
    permission: &str,
    target_id: &str,
) -> anyhow::Result<bool> {
    let scope = current_token_scope(req);
    has_permission_with_scope(
        persistence,
        &current_identity(req),
        Some(&scope),
        permission,
        target_id,
    )
    .await
}

/// Parses the user id accepted by role assignment endpoints.
pub fn user_from_body(body: &Value) -> Option<String> {
    body.as_str()
        .or_else(|| body.get("userId").and_then(Value::as_str))
        .or_else(|| body.get("username").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Builds an upstream role name by joining the role type with its target parts.
pub fn role_name(role_type: &str, parts: &[&str]) -> String {
    std::iter::once(role_type)
        .chain(parts.iter().copied())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(crate::auth::role::ROLE_NAME_SEPARATOR)
}

/// Gets or lazily creates a role with the upstream name shape.
pub async fn find_or_create_role(
    persistence: &Arc<dyn ApolloPersistenceService>,
    name: &str,
    target_id: &str,
    operator: &str,
) -> anyhow::Result<i64> {
    if let Some(role) = RolePersistence::get_role_by_name(&**persistence, name).await? {
        return Ok(role.id);
    }
    let role = RoleService::new(persistence.clone())
        .create(RoleDTO {
            id: None,
            role_name: name.to_string(),
            role_type: 0,
            target_id: target_id.to_string(),
            data_change_created_by: Some(operator.to_string()),
            data_change_created_time: None,
        })
        .await?;
    role.id
        .ok_or_else(|| anyhow::anyhow!("created role has no id"))
}

/// Creates missing permissions and attaches them to the corresponding role.
pub async fn init_role_permissions(
    persistence: &Arc<dyn ApolloPersistenceService>,
    role_id: i64,
    target_id: &str,
    permissions: &[&str],
    operator: &str,
) -> anyhow::Result<()> {
    let existing = PermissionPersistence::list_permission_by_target(&**persistence, target_id)
        .await?
        .into_iter()
        .filter_map(|row| {
            crate::auth::permission::permission_type_name(row.permission_type)
                .map(|name| (name.to_string(), row.id))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let attached = RolePersistence::list_role_permissions(&**persistence, role_id).await?;
    for permission in permissions {
        let permission_id = match existing.get(*permission) {
            Some(id) => *id,
            None => {
                let type_id = crate::auth::permission::permission_type_id(permission)
                    .ok_or_else(|| anyhow::anyhow!("unknown permission type: {}", permission))?;
                PermissionPersistence::create_permission(
                    &**persistence,
                    type_id,
                    target_id,
                    operator,
                )
                .await?
                .id
            }
        };
        if !attached.contains(&permission_id) {
            RolePersistence::assign_role_permission(
                &**persistence,
                role_id,
                permission_id,
                operator,
            )
            .await?;
        }
    }
    Ok(())
}

/// Builds upstream `OpenAppRoleUserDTO` users for a stored role.
pub async fn role_users(
    persistence: &Arc<dyn ApolloPersistenceService>,
    role_id: Option<i64>,
) -> anyhow::Result<Vec<Value>> {
    let Some(role_id) = role_id else {
        return Ok(Vec::new());
    };
    let users = RoleService::new(persistence.clone())
        .list_role_users(role_id)
        .await?;
    let mut result = Vec::with_capacity(users.len());
    for user_id in users {
        result.push(
            user_view(persistence, &user_id).await?.unwrap_or_else(
                || json!({ "userId": user_id, "username": user_id, "name": user_id }),
            ),
        );
    }
    Ok(result)
}

/// Builds an upstream user view with batata's legacy `username` kept as an alias.
pub async fn user_view(
    persistence: &Arc<dyn ApolloPersistenceService>,
    username: &str,
) -> anyhow::Result<Option<Value>> {
    let Some(user) = UserPersistence::get_user(&**persistence, username).await? else {
        return Ok(None);
    };
    let authorities = AuthorityPersistence::list_authorities(&**persistence, username).await?;
    Ok(Some(json!({
        "userId": user.username,
        "username": user.username,
        "name": user.username,
        "email": user.email,
        "realName": user.username,
        "enabled": user.enabled,
        "roles": authorities,
    })))
}

/// Permission types initialized when an application's permissions are created.
pub const APP_PERMISSIONS: &[&str] = &[
    permission_type::CREATE_NAMESPACE,
    permission_type::CREATE_CLUSTER,
    permission_type::ASSIGN_ROLE,
    permission_type::MANAGE_APP_MASTER,
];

/// Permission types initialized for a namespace.
pub const NAMESPACE_PERMISSIONS: &[&str] = &[
    permission_type::MODIFY_NAMESPACE,
    permission_type::RELEASE_NAMESPACE,
];

/// Permission types initialized for a cluster.
pub const CLUSTER_PERMISSIONS: &[&str] = &[
    permission_type::MODIFY_NAMESPACES_IN_CLUSTER,
    permission_type::RELEASE_NAMESPACES_IN_CLUSTER,
];

/// Records a user-management operation in the distributed audit-log tables.
///
/// Auditing is best-effort: a failed audit write must not roll back the
/// operation that already succeeded, but it is logged so operators can see it.
pub async fn record_management_audit(
    req: &HttpRequest,
    persistence: &Arc<dyn ApolloPersistenceService>,
    op_type: &str,
    op_name: &str,
    entity_id: &str,
    entity_name: &str,
    old_value: Option<&str>,
    new_value: Option<&str>,
) {
    let operator = current_identity(req)
        .user_id()
        .unwrap_or("system")
        .to_string();
    let trace_id = req
        .headers()
        .get("x-b3-traceid")
        .or_else(|| req.headers().get("x-request-id"))
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let span_id = uuid::Uuid::new_v4().simple().to_string();
    let service = AuditLogService::new(persistence.clone());
    let result = service
        .record(
            NewAuditLog {
                trace_id,
                span_id: span_id.clone(),
                operator,
                op_type: op_type.to_string(),
                op_name: op_name.to_string(),
                ..NewAuditLog::default()
            },
            vec![NewAuditLogDataInfluence {
                span_id,
                entity_id: entity_id.to_string(),
                entity_name: entity_name.to_string(),
                old_value: old_value.map(str::to_string),
                new_value: new_value.map(str::to_string),
                ..NewAuditLogDataInfluence::default()
            }],
        )
        .await;
    if let Err(error) = result {
        tracing::warn!(%error, "failed to record Apollo management audit");
    }
}
