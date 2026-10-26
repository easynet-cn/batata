//! Remote auth data backend for an independently deployed console.
//!
//! Counterpart of Nacos' `NacosUserServiceRemoteImpl` / `NacosRoleServiceRemoteImpl`:
//! when `batata.deployment.type = console` the console still runs the real
//! [`DefaultAuthPlugin`](crate::plugin::default_auth::DefaultAuthPlugin) locally,
//! but reads users, roles and permissions from the core over HTTP instead of
//! from a local store — the console is not allowed to depend on the database.
//!
//! Calls target the core's `/v3/auth/**` endpoints and carry the server identity
//! header, the same trusted channel the console already uses for proxied data
//! requests.
//!
//! Two consumers share this backend in a separated deployment:
//!
//! - the local auth plugin (login, RBAC), and
//! - the `/v3/auth/**` management handlers the console serves to its own UI
//!   (user, role and permission CRUD).

use std::time::Duration;

use batata_persistence::model::{Page, PermissionInfo, RoleInfo, UserInfo};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::model::GLOBAL_ADMIN_ROLE;
use crate::service::auth_data::AuthDataService;
use crate::service::rbac_cache::RbacSource;
use crate::service::visibility_grant::VISIBILITY_PATH;

/// Envelope returned by the Batata HTTP APIs.
#[derive(Debug, Deserialize)]
struct ApiResult<T> {
    #[serde(default)]
    code: i32,
    #[serde(default)]
    message: String,
    data: Option<T>,
}

/// Camel-cased page payload, matching `batata_common::model::Page<T>`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", bound(deserialize = "T: Deserialize<'de>"))]
struct ApiPage<T> {
    #[serde(default)]
    total_count: u64,
    #[serde(default)]
    page_number: u64,
    #[serde(default)]
    pages_available: u64,
    #[serde(default)]
    page_items: Vec<T>,
}

/// User record as returned by `/v3/auth/user/list`.
#[derive(Debug, Deserialize)]
struct ApiUser {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    source: String,
}

/// Page size used for role and permission lookups.
///
/// A user holds a handful of roles and each role a handful of permissions, so a
/// single generous page avoids paging without risking a large payload.
const LOOKUP_PAGE_SIZE: u64 = 200;

/// Auth data backend that reads from a remote core over HTTP.
pub struct RemoteAuthDataService {
    client: reqwest::Client,
    base: String,
    identity_key: String,
    identity_value: String,
}

impl RemoteAuthDataService {
    /// Build a backend rooted at `base` (scheme, host, port and context path).
    pub fn new(
        base: &str,
        identity_key: String,
        identity_value: String,
        connect_timeout_ms: u64,
        read_timeout_ms: u64,
    ) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(connect_timeout_ms))
            .timeout(Duration::from_millis(read_timeout_ms))
            .no_proxy()
            .build()?;

        Ok(Self {
            client,
            base: base.trim_end_matches('/').to_string(),
            identity_key,
            identity_value,
        })
    }

    /// Join the resolved server addresses and the core context path into a base.
    ///
    /// Mirrors the console's remote datasource resolution: the first reachable
    /// address wins, and the context path defaults to `/nacos` on the core.
    pub fn base_from_addrs(addrs: &[String], context_path: &str) -> String {
        let authority = addrs
            .first()
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:8848".to_string());

        let authority = authority.trim_end_matches('/').to_string();
        let context_path = context_path.trim();
        let context_path = context_path.trim_matches('/');

        if context_path.is_empty() {
            authority
        } else {
            format!("{}/{}", authority, context_path)
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn with_identity(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.identity_key.is_empty() {
            return request;
        }

        request.header(&self.identity_key, &self.identity_value)
    }

    /// Decode a successful envelope, turning a non-zero `code` into an error.
    async fn unwrap_envelope<T: DeserializeOwned>(
        &self,
        path: &str,
        response: reqwest::Response,
    ) -> anyhow::Result<Option<T>> {
        let status = response.status();

        if !status.is_success() {
            anyhow::bail!("auth data request to {} failed with {}", path, status);
        }

        let envelope: ApiResult<T> = response.json().await?;

        if envelope.code != 0 {
            anyhow::bail!("auth data request to {} failed: {}", path, envelope.message);
        }

        Ok(envelope.data)
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> anyhow::Result<Option<T>> {
        let response = self
            .with_identity(self.client.get(self.url(path)))
            .query(query)
            .send()
            .await?;

        self.unwrap_envelope(path, response).await
    }

    async fn get_page<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> anyhow::Result<Vec<T>> {
        Ok(self
            .get::<ApiPage<T>>(path, query)
            .await?
            .map(|page| page.page_items)
            .unwrap_or_default())
    }

    async fn get_page_envelope<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> anyhow::Result<Page<T>> {
        Ok(self
            .get::<ApiPage<T>>(path, query)
            .await?
            .map(|page| Page {
                total_count: page.total_count,
                page_number: page.page_number,
                pages_available: page.pages_available,
                page_items: page.page_items,
            })
            .unwrap_or_default())
    }

    /// Send a form-encoded mutation and require a zero `code` in return.
    async fn mutate(&self, path: &str, request: reqwest::RequestBuilder) -> anyhow::Result<()> {
        let response = self.with_identity(request).send().await?;
        self.unwrap_envelope::<serde_json::Value>(path, response)
            .await?;
        Ok(())
    }

    async fn post_form(&self, path: &str, form: &[(&str, &str)]) -> anyhow::Result<()> {
        self.mutate(path, self.client.post(self.url(path)).form(form))
            .await
    }

    async fn put_form(&self, path: &str, form: &[(&str, &str)]) -> anyhow::Result<()> {
        self.mutate(path, self.client.put(self.url(path)).form(form))
            .await
    }

    async fn delete_query(&self, path: &str, query: &[(&str, String)]) -> anyhow::Result<()> {
        self.mutate(path, self.client.delete(self.url(path)).query(query))
            .await
    }

    /// Translate the API's `search` flag the way the handlers do.
    fn search_mode(accurate: bool) -> &'static str {
        if accurate { "accurate" } else { "blur" }
    }
}

#[async_trait::async_trait]
impl RbacSource for RemoteAuthDataService {
    async fn load_roles(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        self.get_page(
            "/v3/auth/role/list",
            &[
                ("username", username.to_string()),
                ("pageNo", "1".to_string()),
                ("pageSize", LOOKUP_PAGE_SIZE.to_string()),
            ],
        )
        .await
    }

    async fn load_permissions(&self, roles: Vec<String>) -> anyhow::Result<Vec<PermissionInfo>> {
        let mut merged = Vec::new();

        for role in roles {
            let mut page: Vec<PermissionInfo> = self
                .get_page(
                    "/v3/auth/permission/list",
                    &[
                        ("role", role),
                        ("pageNo", "1".to_string()),
                        ("pageSize", LOOKUP_PAGE_SIZE.to_string()),
                    ],
                )
                .await?;

            merged.append(&mut page);
        }

        Ok(merged)
    }
}

#[async_trait::async_trait]
impl AuthDataService for RemoteAuthDataService {
    async fn user_find_by_username(&self, username: &str) -> anyhow::Result<Option<UserInfo>> {
        let users: Vec<ApiUser> = self
            .get_page(
                "/v3/auth/user/list",
                &[
                    ("username", username.to_string()),
                    ("search", "accurate".to_string()),
                    ("pageNo", "1".to_string()),
                    ("pageSize", "1".to_string()),
                ],
            )
            .await?;

        Ok(users.into_iter().next().map(|user| UserInfo {
            username: user.username,
            password: user.password,
            enabled: true,
            source: user.source,
        }))
    }

    async fn user_find_page(
        &self,
        username: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<UserInfo>> {
        let page: Page<ApiUser> = self
            .get_page_envelope(
                "/v3/auth/user/list",
                &[
                    ("username", username.to_string()),
                    ("search", Self::search_mode(accurate).to_string()),
                    ("pageNo", page_no.to_string()),
                    ("pageSize", page_size.to_string()),
                ],
            )
            .await?;

        Ok(Page {
            total_count: page.total_count,
            page_number: page.page_number,
            pages_available: page.pages_available,
            page_items: page
                .page_items
                .into_iter()
                .map(|user| UserInfo {
                    username: user.username,
                    password: user.password,
                    enabled: true,
                    source: user.source,
                })
                .collect(),
        })
    }

    async fn user_create_with_source(
        &self,
        username: &str,
        password_hash: &str,
        _enabled: bool,
        _source: &str,
    ) -> anyhow::Result<()> {
        self.post_form(
            "/v3/auth/user",
            &[("username", username), ("password", password_hash)],
        )
        .await
    }

    async fn user_update_password(
        &self,
        username: &str,
        password_hash: &str,
    ) -> anyhow::Result<()> {
        self.put_form(
            "/v3/auth/user",
            &[("username", username), ("newPassword", password_hash)],
        )
        .await
    }

    async fn user_delete(&self, username: &str) -> anyhow::Result<()> {
        self.delete_query("/v3/auth/user", &[("username", username.to_string())])
            .await
    }

    async fn user_search(&self, username: &str) -> anyhow::Result<Vec<String>> {
        Ok(self
            .get::<Vec<String>>(
                "/v3/auth/user/search",
                &[("username", username.to_string())],
            )
            .await?
            .unwrap_or_default())
    }

    async fn role_find_by_username(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        RbacSource::load_roles(self, username).await
    }

    async fn role_find_page(
        &self,
        username: &str,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<RoleInfo>> {
        self.get_page_envelope(
            "/v3/auth/role/list",
            &[
                ("username", username.to_string()),
                ("role", role.to_string()),
                ("search", Self::search_mode(accurate).to_string()),
                ("pageNo", page_no.to_string()),
                ("pageSize", page_size.to_string()),
            ],
        )
        .await
    }

    async fn role_create(&self, role: &str, username: &str) -> anyhow::Result<()> {
        self.post_form("/v3/auth/role", &[("role", role), ("username", username)])
            .await
    }

    async fn role_delete(&self, role: &str, username: &str) -> anyhow::Result<()> {
        self.delete_query(
            "/v3/auth/role",
            &[
                ("role", role.to_string()),
                ("username", username.to_string()),
            ],
        )
        .await
    }

    async fn role_search(&self, role: &str) -> anyhow::Result<Vec<String>> {
        Ok(self
            .get::<Vec<String>>("/v3/auth/role/search", &[("role", role.to_string())])
            .await?
            .unwrap_or_default())
    }

    async fn role_has_global_admin_by_username(&self, username: &str) -> anyhow::Result<bool> {
        // The core has no dedicated endpoint for this; the role list is small
        // and already cached on the caller side, so derive it locally.
        Ok(RbacSource::load_roles(self, username)
            .await?
            .iter()
            .any(|role| role.role == GLOBAL_ADMIN_ROLE))
    }

    async fn role_has_global_admin(&self) -> anyhow::Result<bool> {
        // No dedicated endpoint either. Nacos answers the same question from
        // `getAllRoles()`, i.e. a full role scan; asking for the reserved role by
        // name gets the same answer from a single small page.
        let page =
            AuthDataService::role_find_page(self, "", GLOBAL_ADMIN_ROLE, 1, LOOKUP_PAGE_SIZE, true)
                .await?;

        Ok(page.total_count > 0)
    }

    async fn create_admin_user(&self, username: &str, password: &str) -> anyhow::Result<()> {
        // The core hashes the password and assigns the reserved admin role, so
        // the console forwards the bootstrap instead of replaying it as a user
        // create plus role create pair. Replaying would not even work: creating
        // `ROLE_ADMIN` through `/v3/auth/role` is rejected as reserved.
        self.post_form(
            "/v3/auth/user/admin",
            &[("username", username), ("password", password)],
        )
        .await
    }

    async fn permission_find_page(
        &self,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<PermissionInfo>> {
        self.get_page_envelope(
            "/v3/auth/permission/list",
            &[
                ("role", role.to_string()),
                ("search", Self::search_mode(accurate).to_string()),
                ("pageNo", page_no.to_string()),
                ("pageSize", page_size.to_string()),
            ],
        )
        .await
    }

    async fn permission_find_by_id(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<Option<PermissionInfo>> {
        // `/v3/auth/permission` answers "does this permission exist?"; the
        // tuple is fully determined by the request, so rebuild it locally.
        let exists = self
            .get::<bool>(
                "/v3/auth/permission",
                &[
                    ("role", role.to_string()),
                    ("resource", resource.to_string()),
                    ("action", action.to_string()),
                ],
            )
            .await?
            .unwrap_or(false);

        Ok(exists.then(|| PermissionInfo {
            role: role.to_string(),
            resource: resource.to_string(),
            action: action.to_string(),
        }))
    }

    async fn permission_grant(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        self.post_form(
            "/v3/auth/permission",
            &[("role", role), ("resource", resource), ("action", action)],
        )
        .await
    }

    async fn permission_revoke(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        self.delete_query(
            "/v3/auth/permission",
            &[
                ("role", role.to_string()),
                ("resource", resource.to_string()),
                ("action", action.to_string()),
            ],
        )
        .await
    }

    async fn visibility_grant(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        // The dedicated role and the permission row are built by the core, so
        // the console forwards the grant itself instead of replaying it as a
        // role + permission pair — the same split Nacos makes in
        // `RemoteVisibilityGrantService`.
        self.post_form(
            VISIBILITY_PATH,
            &[
                ("namespaceId", namespace_id),
                ("resourceType", resource_type),
                ("resourceName", resource_name),
                ("username", username),
                ("action", action),
            ],
        )
        .await
    }

    async fn visibility_revoke(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        self.delete_query(
            VISIBILITY_PATH,
            &[
                ("namespaceId", namespace_id.to_string()),
                ("resourceType", resource_type.to_string()),
                ("resourceName", resource_name.to_string()),
                ("username", username.to_string()),
                ("action", action.to_string()),
            ],
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_joins_authority_and_context_path() {
        assert_eq!(
            RemoteAuthDataService::base_from_addrs(&["http://127.0.0.1:8848".to_string()], "nacos"),
            "http://127.0.0.1:8848/nacos"
        );
    }

    #[test]
    fn base_tolerates_leading_and_trailing_slashes() {
        assert_eq!(
            RemoteAuthDataService::base_from_addrs(&["http://host:8848/".to_string()], "/nacos/"),
            "http://host:8848/nacos"
        );
    }

    #[test]
    fn base_omits_an_empty_context_path() {
        assert_eq!(
            RemoteAuthDataService::base_from_addrs(&["http://host:8848".to_string()], ""),
            "http://host:8848"
        );
    }

    #[test]
    fn base_falls_back_to_localhost_without_addresses() {
        assert_eq!(
            RemoteAuthDataService::base_from_addrs(&[], "nacos"),
            "http://127.0.0.1:8848/nacos"
        );
    }

    #[test]
    fn search_mode_maps_the_accuracy_flag() {
        assert_eq!(RemoteAuthDataService::search_mode(true), "accurate");
        assert_eq!(RemoteAuthDataService::search_mode(false), "blur");
    }
}
