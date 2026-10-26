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

use std::time::Duration;

use batata_persistence::model::{PermissionInfo, RoleInfo, UserInfo};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::service::auth_data::AuthDataService;
use crate::service::rbac_cache::RbacSource;

/// Envelope returned by the Batata HTTP APIs.
#[derive(Debug, Deserialize)]
struct ApiResult<T> {
    #[serde(default)]
    code: i32,
    #[serde(default)]
    message: String,
    data: Option<T>,
}

/// Camel-cased page payload, matching `batata_api::model::Page<T>`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiPage<T> {
    page_items: Option<Vec<T>>,
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

    async fn get_page<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> anyhow::Result<Vec<T>> {
        let response = self
            .with_identity(self.client.get(self.url(path)))
            .query(query)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            anyhow::bail!("auth data request to {} failed with {}", path, status);
        }

        let envelope: ApiResult<ApiPage<T>> = response.json().await?;

        if envelope.code != 0 {
            anyhow::bail!("auth data request to {} failed: {}", path, envelope.message);
        }

        Ok(envelope
            .data
            .and_then(|page| page.page_items)
            .unwrap_or_default())
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

    async fn user_create_with_source(
        &self,
        username: &str,
        password_hash: &str,
        _enabled: bool,
        _source: &str,
    ) -> anyhow::Result<()> {
        let response = self
            .with_identity(self.client.post(self.url("/v3/auth/user")))
            .form(&[("username", username), ("password", password_hash)])
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            anyhow::bail!("create user request failed with {}", status);
        }

        let envelope: ApiResult<serde_json::Value> = response.json().await?;

        if envelope.code != 0 {
            anyhow::bail!("create user request failed: {}", envelope.message);
        }

        Ok(())
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
}
