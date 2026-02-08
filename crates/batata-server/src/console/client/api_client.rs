// API client for remote console operations
// Provides typed methods for each console API endpoint

use serde::{Deserialize, Serialize};

use batata_config::Namespace;

use crate::{
    api::{
        config::model::{
            ConfigBasicInfo, ConfigGrayInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo,
        },
        model::{Member, Page},
    },
    config::{
        export_model::{ImportResult, SameConfigPolicy},
        model::ConfigAllInfo,
    },
    console::v3::cluster::{
        ClusterHealthResponse, ClusterHealthSummaryResponse, SelfMemberResponse,
    },
};

use super::ConsoleHttpClient;

/// API client wrapper providing typed access to console APIs
pub struct ConsoleApiClient {
    http_client: ConsoleHttpClient,
}

// Generic API response wrapper
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    #[allow(dead_code)]
    pub code: i32,
    #[allow(dead_code)]
    pub message: String,
    pub data: T,
}

impl ConsoleApiClient {
    pub fn new(http_client: ConsoleHttpClient) -> Self {
        Self { http_client }
    }

    // ============== Namespace APIs ==============

    pub async fn namespace_find_all(&self) -> anyhow::Result<Vec<Namespace>> {
        let response: ApiResponse<Vec<Namespace>> = self
            .http_client
            .get("/v3/admin/core/namespace/list")
            .await?;
        Ok(response.data)
    }

    pub async fn namespace_get_by_id(&self, namespace_id: &str) -> anyhow::Result<Namespace> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
        }

        let response: ApiResponse<Namespace> = self
            .http_client
            .get_with_query("/v3/admin/core/namespace", &Query { namespace_id })
            .await?;
        Ok(response.data)
    }

    pub async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            custom_namespace_id: &'a str,
            namespace_name: &'a str,
            namespace_desc: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .post_form(
                "/v3/admin/core/namespace",
                &Form {
                    custom_namespace_id: namespace_id,
                    namespace_name,
                    namespace_desc,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn namespace_update(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            namespace_name: &'a str,
            namespace_desc: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .put_form(
                "/v3/admin/core/namespace",
                &Form {
                    namespace_id,
                    namespace_name,
                    namespace_desc,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .delete_with_query("/v3/admin/core/namespace", &Query { namespace_id })
            .await?;
        Ok(response.data)
    }

    pub async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            custom_namespace_id: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .get_with_query(
                "/v3/admin/core/namespace/exist",
                &Query {
                    custom_namespace_id: namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============== Config APIs ==============

    pub async fn config_find_one(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigAllInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<Option<ConfigAllInfo>> = self
            .http_client
            .get_with_query(
                "/v3/admin/cs/config",
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn config_search_page(
        &self,
        page_no: u64,
        page_size: u64,
        namespace_id: &str,
        data_id: &str,
        group_name: &str,
        app_name: &str,
        tags: &str,
        types: &str,
        content: &str,
    ) -> anyhow::Result<Page<ConfigBasicInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            page_no: u64,
            page_size: u64,
            namespace_id: &'a str,
            data_id: &'a str,
            group_name: &'a str,
            app_name: &'a str,
            config_tags: &'a str,
            r#type: &'a str,
            content: &'a str,
        }

        let response: ApiResponse<Page<ConfigBasicInfo>> = self
            .http_client
            .get_with_query(
                "/v3/admin/cs/config/list",
                &Query {
                    page_no,
                    page_size,
                    namespace_id,
                    data_id,
                    group_name,
                    app_name,
                    config_tags: tags,
                    r#type: types,
                    content,
                },
            )
            .await?;
        Ok(response.data)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn config_create_or_update(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        content: &str,
        app_name: &str,
        config_tags: &str,
        desc: &str,
        r#use: &str,
        effect: &str,
        r#type: &str,
        schema: &str,
        encrypted_data_key: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
            content: &'a str,
            app_name: &'a str,
            config_tags: &'a str,
            desc: &'a str,
            r#use: &'a str,
            effect: &'a str,
            r#type: &'a str,
            schema: &'a str,
            encrypted_data_key: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .post_form(
                "/v3/admin/cs/config",
                &Form {
                    data_id,
                    group_name,
                    namespace_id,
                    content,
                    app_name,
                    config_tags,
                    desc,
                    r#use,
                    effect,
                    r#type,
                    schema,
                    encrypted_data_key,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn config_delete(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            tenant: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .delete_with_query(
                "/v3/admin/cs/config",
                &Query {
                    data_id,
                    group_name,
                    tenant: namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn config_find_gray_one(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigGrayInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<Option<ConfigGrayInfo>> = self
            .http_client
            .get_with_query(
                "/v3/admin/cs/config/beta",
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn config_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<&str>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            group: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            data_ids: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            app_name: Option<&'a str>,
        }

        // Build URL with query params manually for binary response
        let query = Query {
            namespace_id,
            group,
            data_ids,
            app_name,
        };
        let query_string = serde_urlencoded::to_string(&query)?;
        let path = format!("/v3/admin/cs/config/export?{}", query_string);

        self.http_client.get_bytes(&path).await
    }

    /// Import configuration from a ZIP file
    pub async fn config_import(
        &self,
        file_data: Vec<u8>,
        namespace_id: &str,
        policy: SameConfigPolicy,
    ) -> anyhow::Result<ImportResult> {
        // Build query parameters
        let query_string = serde_urlencoded::to_string(&[
            ("namespace_id", namespace_id),
            ("policy", &format!("{}", policy)),
        ])?;
        let path = format!("/v3/admin/cs/config/import?{}", query_string);

        // Create multipart form
        let part = reqwest::multipart::Part::bytes(file_data)
            .file_name("import.zip")
            .mime_str("application/zip")?;
        let form = reqwest::multipart::Form::new().part("file", part);

        let response: ApiResponse<ImportResult> =
            self.http_client.post_multipart(&path, form).await?;
        Ok(response.data)
    }

    // ============== History APIs ==============

    pub async fn history_find_by_id(
        &self,
        nid: u64,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigHistoryDetailInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            nid: u64,
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<Option<ConfigHistoryDetailInfo>> = self
            .http_client
            .get_with_query(
                "/v3/admin/cs/history",
                &Query {
                    nid,
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn history_search_page(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryBasicInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<Page<ConfigHistoryBasicInfo>> = self
            .http_client
            .get_with_query(
                "/v3/admin/cs/history/list",
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn history_find_configs_by_namespace_id(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigBasicInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
        }

        let response: ApiResponse<Vec<ConfigBasicInfo>> = self
            .http_client
            .get_with_query("/v3/admin/cs/history/configs", &Query { namespace_id })
            .await?;
        Ok(response.data)
    }

    // ============== Cluster APIs ==============

    pub async fn cluster_all_members(&self) -> anyhow::Result<Vec<Member>> {
        let response: ApiResponse<Vec<Member>> = self
            .http_client
            .get("/v3/admin/core/cluster/node/list")
            .await?;
        Ok(response.data)
    }

    pub async fn cluster_healthy_members(&self) -> anyhow::Result<Vec<Member>> {
        let response: ApiResponse<Vec<Member>> = self
            .http_client
            .get("/v3/admin/core/cluster/node/list")
            .await?;
        Ok(response.data)
    }

    pub async fn cluster_get_health(&self) -> anyhow::Result<ClusterHealthResponse> {
        // Admin API health endpoint returns {"healthy": bool}
        #[derive(Deserialize)]
        struct HealthResponse {
            healthy: bool,
        }

        let response: ApiResponse<HealthResponse> = self
            .http_client
            .get("/v3/admin/core/cluster/node/self/health")
            .await?;

        let members = self.cluster_all_members().await.unwrap_or_default();
        let up_count = members
            .iter()
            .filter(|m| m.state.to_string() == "UP")
            .count();
        let total = members.len();

        Ok(ClusterHealthResponse {
            is_healthy: response.data.healthy,
            summary: ClusterHealthSummaryResponse {
                total,
                up: up_count,
                down: total.saturating_sub(up_count),
                suspicious: 0,
                starting: 0,
                isolation: 0,
            },
            standalone: total <= 1,
        })
    }

    pub async fn cluster_get_self(&self) -> anyhow::Result<SelfMemberResponse> {
        // Admin API returns a NodeSelfResponse; map to SelfMemberResponse
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct NodeSelfResponse {
            ip: String,
            port: u16,
            address: String,
            state: String,
            #[serde(default)]
            extend_info: Option<serde_json::Value>,
        }

        let response: ApiResponse<NodeSelfResponse> = self
            .http_client
            .get("/v3/admin/core/cluster/node/self")
            .await?;

        let node = response.data;
        let version = node
            .extend_info
            .as_ref()
            .and_then(|info| info.get("version"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let is_standalone = self.cluster_is_standalone().await.unwrap_or(true);

        Ok(SelfMemberResponse {
            ip: node.ip,
            port: node.port,
            address: node.address,
            state: node.state,
            is_standalone,
            version,
        })
    }

    pub async fn cluster_get_member(&self, address: &str) -> anyhow::Result<Option<Member>> {
        #[derive(Serialize)]
        struct Query<'a> {
            keyword: &'a str,
        }

        let response: ApiResponse<Vec<Member>> = self
            .http_client
            .get_with_query(
                "/v3/admin/core/cluster/node/list",
                &Query { keyword: address },
            )
            .await?;
        Ok(response.data.into_iter().find(|m| m.address == address))
    }

    pub async fn cluster_member_count(&self) -> anyhow::Result<usize> {
        // Admin API does not have a dedicated count endpoint; derive from node list
        let members = self.cluster_all_members().await?;
        Ok(members.len())
    }

    pub async fn cluster_is_standalone(&self) -> anyhow::Result<bool> {
        // Admin API does not have a dedicated standalone endpoint; derive from node list
        let members = self.cluster_all_members().await?;
        Ok(members.len() <= 1)
    }

    pub async fn cluster_refresh_self(&self) -> anyhow::Result<bool> {
        // Admin API does not have a refresh endpoint; return true as no-op
        Ok(true)
    }

    // ============== Service APIs ==============

    #[allow(clippy::too_many_arguments)]
    pub async fn service_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<serde_json::Value> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            service_name: &'a str,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .get_with_query(
                "/v3/admin/ns/service/list",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn service_get(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<serde_json::Value> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
        }

        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .get_with_query(
                "/v3/admin/ns/service",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn service_create(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
        metadata: &str,
        selector: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            protect_threshold: f32,
            #[serde(skip_serializing_if = "str::is_empty")]
            metadata: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            selector: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .post_json(
                "/v3/admin/ns/service",
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    protect_threshold,
                    metadata,
                    selector,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn service_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
        metadata: &str,
        selector: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            protect_threshold: f32,
            #[serde(skip_serializing_if = "str::is_empty")]
            metadata: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            selector: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .put_form(
                "/v3/admin/ns/service",
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    protect_threshold,
                    metadata,
                    selector,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn service_delete(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .delete_with_query(
                "/v3/admin/ns/service",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn service_subscriber_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<serde_json::Value> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .get_with_query(
                "/v3/admin/ns/client/subscribe/list",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============== Instance APIs ==============

    pub async fn instance_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> anyhow::Result<serde_json::Value> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            cluster_name: &'a str,
        }

        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .get_with_query(
                "/v3/admin/ns/instance/list",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    cluster_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn instance_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
        weight: f64,
        enabled: bool,
        metadata: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            ip: &'a str,
            port: i32,
            cluster_name: &'a str,
            weight: f64,
            enabled: bool,
            #[serde(skip_serializing_if = "str::is_empty")]
            metadata: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .put_form(
                "/v3/admin/ns/instance",
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    ip,
                    port,
                    cluster_name,
                    weight,
                    enabled,
                    metadata,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============== Config Listener APIs ==============

    pub async fn config_listener_list(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .get_with_query(
                "/v3/admin/cs/listener",
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============== Server State APIs ==============

    pub async fn server_state(
        &self,
    ) -> anyhow::Result<std::collections::HashMap<String, Option<String>>> {
        let response: ApiResponse<std::collections::HashMap<String, Option<String>>> = self
            .http_client
            .get("/v3/admin/core/state")
            .await
            .unwrap_or(ApiResponse {
                code: 200,
                message: String::new(),
                data: std::collections::HashMap::new(),
            });
        Ok(response.data)
    }
}
