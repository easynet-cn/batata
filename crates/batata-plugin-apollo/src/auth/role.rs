//! Role naming helpers ported from upstream Apollo.
//!
//! Upstream sources:
//! - `apollo-portal/.../portal/constant/RoleType.java` — the five role types.
//! - `apollo-portal/.../portal/util/RoleUtils.java` — the naming rules.
//!
//! Apollo composes both role names and permission target ids by joining their
//! parts with the cluster/namespace separator (`+`), e.g. `Master+appId` for an
//! application master role and `ModifyNamespace+appId+ns` for a namespace level
//! role. Getting these strings exactly right matters because they are stored in
//! `apollo_role.role_name` and matched against by the permission checks.

/// Separator used to build role names and permission target ids.
///
/// Upstream `ConfigConsts.CLUSTER_NAMESPACE_SEPARATOR`.
pub const ROLE_NAME_SEPARATOR: &str = "+";

/// Upstream `RoleType` constants.
pub mod role_type {
    /// Application master role — full control over one application.
    pub const MASTER: &str = "Master";
    /// Permission to modify one specific namespace.
    pub const MODIFY_NAMESPACE: &str = "ModifyNamespace";
    /// Permission to release one specific namespace.
    pub const RELEASE_NAMESPACE: &str = "ReleaseNamespace";
    /// Permission to modify every namespace within a cluster.
    pub const MODIFY_NAMESPACES_IN_CLUSTER: &str = "ModifyNamespacesInCluster";
    /// Permission to release every namespace within a cluster.
    pub const RELEASE_NAMESPACES_IN_CLUSTER: &str = "ReleaseNamespacesInCluster";

    /// Whether the given role type is one of the known upstream types.
    ///
    /// Upstream `RoleType.isValidRoleType`.
    pub fn is_valid(role_type: &str) -> bool {
        matches!(
            role_type,
            MASTER
                | MODIFY_NAMESPACE
                | RELEASE_NAMESPACE
                | MODIFY_NAMESPACES_IN_CLUSTER
                | RELEASE_NAMESPACES_IN_CLUSTER
        )
    }
}

/// Joins role-name parts with the upstream separator, skipping empty ones.
///
/// Upstream builds names with Guava's `Joiner.on("+").skipNulls()`, so a missing
/// trailing `env` simply shortens the name instead of producing a dangling `+`.
fn join(parts: &[Option<&str>]) -> String {
    parts
        .iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(ROLE_NAME_SEPARATOR)
}

/// Builds the application master role name.
///
/// Upstream `RoleUtils.buildAppMasterRoleName` — `Master+{appId}`.
pub fn build_app_master_role_name(app_id: &str) -> String {
    join(&[Some(role_type::MASTER), Some(app_id)])
}

/// Builds a generic application level role name.
///
/// Upstream `RoleUtils.buildAppRoleName` — `{roleType}+{appId}`.
pub fn build_app_role_name(app_id: &str, role_type: &str) -> String {
    join(&[Some(role_type), Some(app_id)])
}

/// Builds the role granting modification rights on one namespace.
///
/// Upstream `RoleUtils.buildModifyNamespaceRoleName` —
/// `ModifyNamespace+{appId}+{namespaceName}` (plus `+{env}` when given).
pub fn build_modify_namespace_role_name(
    app_id: &str,
    namespace_name: &str,
    env: Option<&str>,
) -> String {
    join(&[
        Some(role_type::MODIFY_NAMESPACE),
        Some(app_id),
        Some(namespace_name),
        env,
    ])
}

/// Builds the role granting modification rights on every namespace of a cluster.
///
/// Upstream `RoleUtils.buildModifyNamespacesInClusterRoleName` —
/// `ModifyNamespacesInCluster+{appId}+{env}+{clusterName}`.
pub fn build_modify_namespaces_in_cluster_role_name(
    app_id: &str,
    env: &str,
    cluster_name: &str,
) -> String {
    join(&[
        Some(role_type::MODIFY_NAMESPACES_IN_CLUSTER),
        Some(app_id),
        Some(env),
        Some(cluster_name),
    ])
}

/// Builds the role granting release rights on one namespace.
///
/// Upstream `RoleUtils.buildReleaseNamespaceRoleName` —
/// `ReleaseNamespace+{appId}+{namespaceName}` (plus `+{env}` when given).
pub fn build_release_namespace_role_name(
    app_id: &str,
    namespace_name: &str,
    env: Option<&str>,
) -> String {
    join(&[
        Some(role_type::RELEASE_NAMESPACE),
        Some(app_id),
        Some(namespace_name),
        env,
    ])
}

/// Builds the role granting release rights on every namespace of a cluster.
///
/// Upstream `RoleUtils.buildReleaseNamespacesInClusterRoleName` —
/// `ReleaseNamespacesInCluster+{appId}+{env}+{clusterName}`.
pub fn build_release_namespaces_in_cluster_role_name(
    app_id: &str,
    env: &str,
    cluster_name: &str,
) -> String {
    join(&[
        Some(role_type::RELEASE_NAMESPACES_IN_CLUSTER),
        Some(app_id),
        Some(env),
        Some(cluster_name),
    ])
}

/// Builds the target id identifying a namespace.
///
/// Upstream `RoleUtils.buildNamespaceTargetId` — `{appId}+{namespaceName}`
/// (plus `+{env}` when given).
pub fn build_namespace_target_id(app_id: &str, namespace_name: &str, env: Option<&str>) -> String {
    join(&[Some(app_id), Some(namespace_name), env])
}

/// Builds the target id identifying a cluster.
///
/// Upstream `RoleUtils.buildClusterTargetId` — `{appId}+{env}+{clusterName}`.
pub fn build_cluster_target_id(app_id: &str, env: &str, cluster_name: &str) -> String {
    join(&[Some(app_id), Some(env), Some(cluster_name)])
}

/// Extracts the application id from an application master role name.
///
/// Upstream `RoleUtils.extractAppIdFromMasterRoleName`. Returns `None` when the
/// name does not start with the master role type.
pub fn extract_app_id_from_master_role_name(master_role_name: &str) -> Option<&str> {
    let mut parts = master_role_name.split(ROLE_NAME_SEPARATOR);
    match parts.next() {
        Some(role_type::MASTER) => parts.next(),
        _ => None,
    }
}

/// Extracts the application id from any known role name.
///
/// Upstream `RoleUtils.extractAppIdFromRoleName`. Returns `None` when the leading
/// segment is not a valid role type.
pub fn extract_app_id_from_role_name(role_name: &str) -> Option<&str> {
    let mut parts = role_name.split(ROLE_NAME_SEPARATOR);
    let role_type = parts.next()?;
    if role_type::is_valid(role_type) {
        parts.next()
    } else {
        None
    }
}
