//! Constraints carried by an Apollo user token.
//!
//! Upstream `UserTokenService.parseScope` stores a JSON document in
//! `apollo_user_token.scopes`. An empty value means "deny all"; `{}` means
//! "allow all"; a populated document restricts operations and resources.
//! The token's own scope is always ANDed with the user's RBAC permissions.

use serde::{Deserialize, Serialize};

/// Upstream `UserTokenOperation` values.
pub mod operation {
    /// Read configuration.
    pub const CONFIG_READ: &str = "config:read";
    /// Modify configuration items.
    pub const CONFIG_MODIFY: &str = "config:modify";
    /// Publish a release.
    pub const CONFIG_RELEASE: &str = "config:release";
    /// Create a namespace.
    pub const NAMESPACE_CREATE: &str = "namespace:create";
    /// Delete a namespace.
    pub const NAMESPACE_DELETE: &str = "namespace:delete";
    /// Create a cluster.
    pub const CLUSTER_CREATE: &str = "cluster:create";
    /// Create an application.
    pub const APP_CREATE: &str = "app:create";
    /// Assign application roles.
    pub const APP_MANAGE_ROLE: &str = "app:manage-role";
    /// Manage portal users.
    pub const USER_MANAGE: &str = "user:manage";
    /// System-level administration.
    pub const SYSTEM_ADMIN: &str = "system:admin";
}

/// Maps a batata permission name to the upstream token operation it requires.
pub fn operation_for_permission(permission: &str) -> Option<&'static str> {
    use crate::auth::permission::permission_type as p;
    use operation as op;
    Some(match permission {
        p::CREATE_APPLICATION => op::APP_CREATE,
        p::MANAGE_USERS => op::USER_MANAGE,
        p::MANAGE_APP_MASTER | p::ASSIGN_ROLE => op::APP_MANAGE_ROLE,
        p::CREATE_NAMESPACE => op::NAMESPACE_CREATE,
        p::DELETE_NAMESPACE => op::NAMESPACE_DELETE,
        p::CREATE_CLUSTER => op::CLUSTER_CREATE,
        p::MODIFY_NAMESPACE | p::MODIFY_NAMESPACES_IN_CLUSTER => op::CONFIG_MODIFY,
        p::RELEASE_NAMESPACE | p::RELEASE_NAMESPACES_IN_CLUSTER => op::CONFIG_RELEASE,
        p::CREATE_GRAY_RELEASE | p::MERGE_GRAY_RELEASE | p::DELETE_GRAY_RELEASE => {
            op::CONFIG_MODIFY
        }
        p::DELETE_CLUSTER | p::DELETE_APPLICATION => op::SYSTEM_ADMIN,
        p::MANAGE_CONSUMER => op::SYSTEM_ADMIN,
        _ => return None,
    })
}

/// Upstream `UserTokenScope` wire model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TokenScope {
    /// Allowed operation names. Empty means every operation.
    pub operations: Vec<String>,
    /// Allowed application ids. Empty means every application.
    pub app_ids: Vec<String>,
    /// Allowed environments. Empty means every environment.
    pub envs: Vec<String>,
    /// Allowed namespace selectors. Empty means every namespace.
    pub namespaces: Vec<TokenNamespaceScope>,
    /// Set when the token has no usable scope document.
    #[serde(skip)]
    pub deny_all: bool,
}

/// One namespace selector inside a token scope.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TokenNamespaceScope {
    /// Application id; empty or `*` acts as a wildcard.
    pub app_id: String,
    /// Environment; empty or `*` acts as a wildcard.
    pub env: String,
    /// Cluster name; empty or `*` acts as a wildcard.
    pub cluster_name: String,
    /// Namespace name; empty or `*` acts as a wildcard.
    pub namespace_name: String,
}

impl TokenScope {
    /// Builds the scope represented by a stored `scopes` column.
    ///
    /// Mirrors upstream: missing, blank or malformed JSON denies everything.
    pub fn parse(raw: Option<&str>) -> Self {
        let Some(raw) = raw else {
            return Self::deny_all();
        };
        let raw = raw.trim();
        if raw.is_empty() {
            return Self::deny_all();
        }
        match serde_json::from_str::<Self>(raw) {
            Ok(scope) => scope,
            Err(_) => Self::deny_all(),
        }
    }

    /// Builds a scope that denies every operation.
    pub fn deny_all() -> Self {
        Self {
            deny_all: true,
            ..Self::default()
        }
    }

    /// Builds a scope that allows every operation and resource.
    pub fn allow_all() -> Self {
        Self::default()
    }

    /// Whether an operation is allowed by this token.
    pub fn allows_operation(&self, operation: &str) -> bool {
        !self.deny_all
            && (self.operations.is_empty()
                || self
                    .operations
                    .iter()
                    .any(|candidate| candidate == operation))
    }

    /// Whether an application is allowed by this token.
    pub fn allows_app(&self, app_id: &str) -> bool {
        !self.deny_all && (self.app_ids.is_empty() || self.app_ids.iter().any(|id| id == app_id))
    }

    /// Whether an environment is allowed by this token.
    ///
    /// Mirrors upstream `UserTokenScope.allowsEnv`: the presented environment
    /// passes through `Env.transformEnv` before being compared with the stored
    /// (canonical) values.
    pub fn allows_env(&self, env: &str) -> bool {
        let env = normalize_env(env);
        !self.deny_all
            && (self.envs.is_empty() || self.envs.iter().any(|candidate| candidate == &env))
    }

    /// Whether a permission target is allowed by this token.
    ///
    /// Upstream's permission validator applies `allowsApp` to application level
    /// checks and `allowsNamespace` to namespace / cluster level checks. The
    /// target id shape tells us which resource coordinates are present:
    /// `{app}`, `{app}+{namespace}[+{env}]` or `{app}+{env}+{cluster}`.
    pub fn allows_permission_target(&self, permission: &str, target_id: &str) -> bool {
        let parts: Vec<&str> = target_id
            .split(crate::auth::role::ROLE_NAME_SEPARATOR)
            .filter(|part| !part.is_empty())
            .collect();
        let Some(app_id) = parts.first().copied() else {
            return false;
        };
        if !self.allows_app(app_id) {
            return false;
        }
        if parts.len() == 1 {
            return true;
        }
        use crate::auth::permission::permission_type as p;
        let cluster_target = matches!(
            permission,
            p::CREATE_CLUSTER
                | p::DELETE_CLUSTER
                | p::MODIFY_NAMESPACES_IN_CLUSTER
                | p::RELEASE_NAMESPACES_IN_CLUSTER
        );
        if cluster_target {
            self.allows_namespace(
                app_id,
                parts.get(1).copied().unwrap_or(""),
                parts.get(2).copied().unwrap_or(""),
                "",
            )
        } else {
            self.allows_namespace(
                app_id,
                parts.get(2).copied().unwrap_or(""),
                "",
                parts.get(1).copied().unwrap_or(""),
            )
        }
    }

    /// Whether a namespace is allowed by this token.
    ///
    /// Upstream `UserTokenScope.allowsNamespace` normalizes both the selector
    /// environment and the presented environment before matching.
    pub fn allows_namespace(
        &self,
        app_id: &str,
        env: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> bool {
        if !self.allows_app(app_id) || !self.allows_env(env) {
            return false;
        }
        let env = normalize_env(env);
        self.namespaces.is_empty()
            || self.namespaces.iter().any(|scope| {
                matches_selector(&scope.app_id, app_id)
                    && matches_selector(&normalize_env(&scope.env), &env)
                    && matches_selector(&scope.cluster_name, cluster_name)
                    && matches_selector(&scope.namespace_name, namespace_name)
            })
    }
}

/// Maps an environment name to Apollo's canonical form.
///
/// Upstream `Env.transformEnv` trims and upper-cases known names, maps the
/// `PROD` typo and `FWS` to `PRO` / `FAT`, and keeps unknown names unchanged
/// (`Env.UNKNOWN` makes `UserTokenScope.normalizeEnv` return the input).
pub fn normalize_env(env: &str) -> String {
    let trimmed = env.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let upper = trimmed.to_uppercase();
    let canonical = match upper.as_str() {
        "PROD" => "PRO",
        "FWS" => "FAT",
        other => other,
    };
    if matches!(
        canonical,
        "LOCAL" | "DEV" | "FAT" | "UAT" | "LPT" | "PRO" | "TOOLS"
    ) {
        canonical.to_string()
    } else {
        trimmed.to_string()
    }
}

fn matches_selector(expected: &str, actual: &str) -> bool {
    expected.is_empty() || expected == "*" || expected == actual
}

/// Checks a PAT scope against an OpenAPI data request.
///
/// Upstream's `UserTokenPermissionValidator.shouldHideConfigToCurrentUser`
/// gates config reads with `CONFIG_READ` plus `allowsNamespace`. Management
/// endpoints perform their own permission check; this guard covers the data
/// paths where batata historically relied on authentication alone. Returns
/// `false` only when the path fully identifies an app/env/cluster/namespace.
pub fn scope_allows_request(scope: &TokenScope, path: &str, method: &str) -> bool {
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let segment_after = |marker: &str| -> Option<&str> {
        segments
            .iter()
            .position(|part| *part == marker)
            .and_then(|index| segments.get(index + 1))
            .copied()
    };
    let (Some(app_id), Some(env), Some(cluster), Some(namespace)) = (
        segment_after("apps"),
        segment_after("envs"),
        segment_after("clusters"),
        segment_after("namespaces"),
    ) else {
        return true;
    };
    // `/apps/envs/{env}` is not an app-scoped path.
    if app_id == "envs" || app_id == "v1" {
        return true;
    }
    let operation = if method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD") {
        operation::CONFIG_READ
    } else if path.contains("/releases") {
        operation::CONFIG_RELEASE
    } else {
        operation::CONFIG_MODIFY
    };
    scope.allows_operation(operation) && scope.allows_namespace(app_id, env, cluster, namespace)
}

#[cfg(test)]
mod tests {
    use super::{TokenScope, normalize_env, scope_allows_request};

    #[test]
    fn missing_or_malformed_scope_denies_everything() {
        assert!(TokenScope::parse(None).deny_all);
        assert!(TokenScope::parse(Some("  ")).deny_all);
        assert!(TokenScope::parse(Some("not-json")).deny_all);
    }

    #[test]
    fn empty_json_scope_allows_everything() {
        let scope = TokenScope::parse(Some("{}"));
        assert!(!scope.deny_all);
        assert!(scope.allows_operation("config:read"));
        assert!(scope.allows_app("app"));
        assert!(scope.allows_namespace("app", "DEV", "default", "application"));
    }

    #[test]
    fn populated_scope_restricts_operations_and_resources() {
        let scope = TokenScope::parse(Some(
            r#"{"operations":["config:read"],"appIds":["app-a"],"envs":["DEV"]}"#,
        ));
        assert!(scope.allows_operation("config:read"));
        assert!(!scope.allows_operation("config:modify"));
        assert!(scope.allows_app("app-a"));
        assert!(!scope.allows_app("app-b"));
        assert!(scope.allows_env("DEV"));
        assert!(!scope.allows_env("PRO"));
    }

    #[test]
    fn environment_normalization_follows_upstream_env_transform() {
        assert_eq!(normalize_env(" dev "), "DEV");
        assert_eq!(normalize_env("Prod"), "PRO");
        assert_eq!(normalize_env("fws"), "FAT");
        assert_eq!(normalize_env("custom"), "custom");
        assert_eq!(normalize_env(""), "");
    }

    #[test]
    fn namespace_selectors_match_normalized_env() {
        let scope = TokenScope::parse(Some(
            r#"{"envs":["PRO"],"namespaces":[{"appId":"app","env":"PROD","namespaceName":"application"}]}"#,
        ));
        assert!(scope.allows_namespace("app", "prod", "default", "application"));
        assert!(!scope.allows_namespace("app", "prod", "default", "other"));
        assert!(!scope.allows_namespace("app", "DEV", "default", "application"));
    }

    #[test]
    fn data_path_scope_guard_checks_operation_and_resource() {
        let path = "/openapi/v1/apps/app-a/envs/DEV/clusters/default/namespaces/application/items";
        let read_only = TokenScope::parse(Some(
            r#"{"operations":["config:read"],"envs":["DEV"],"namespaces":[{"appId":"app-a","namespaceName":"application"}]}"#,
        ));
        assert!(scope_allows_request(&read_only, path, "GET"));
        assert!(!scope_allows_request(&read_only, path, "PUT"));
        assert!(!scope_allows_request(
            &read_only,
            &path.replace("DEV", "PRO"),
            "GET"
        ));
        assert!(!scope_allows_request(
            &read_only,
            &path.replace("application", "other"),
            "GET"
        ));
        // Management paths are covered by the handler-level permission checks.
        assert!(scope_allows_request(&read_only, "/openapi/v1/users", "GET"));
    }
}
