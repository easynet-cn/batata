// MaintainerClient - facade for all Admin API operations

use std::collections::HashMap;

use batata_client::{BatataHttpClient, HttpClientConfig};
use serde::{Deserialize, Serialize};

use crate::{
    config::MaintainerClientConfig,
    constants::admin_api_path,
    model::common::ApiResponse,
    model::{
        AgentCard, AgentCardDetailInfo, AgentCardVersionInfo, AgentVersionDetail,
        ClientPublisherInfo, ClientServiceInfo, ClientSubscriberInfo, ClientSummaryInfo,
        ClusterHealthResponse, ClusterHealthSummary, ClusterInfo, ConfigBasicInfo, ConfigCloneInfo,
        ConfigDetailInfo, ConfigGrayInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo,
        ConfigListenerInfo, ConnectionInfo, IdGeneratorInfo, ImportResult, Instance,
        InstanceMetadataBatchResult, McpEndpointSpec, McpServerBasicInfo, McpServerDetailInfo,
        McpToolSpecification, Member, MetricsInfo, Namespace, Page, SameConfigPolicy,
        SelfMemberResponse, ServerLoaderMetrics, ServiceDetailInfo, ServiceView, SubscriberInfo,
    },
};

/// Admin/maintainer HTTP client for Batata/Nacos
pub struct MaintainerClient {
    http_client: BatataHttpClient,
}

impl MaintainerClient {
    /// Create a new MaintainerClient with the given configuration
    pub async fn new(config: MaintainerClientConfig) -> anyhow::Result<Self> {
        let http_config = HttpClientConfig::with_servers(config.server_addrs)
            .with_auth(&config.username, &config.password)
            .with_timeouts(config.connect_timeout_ms, config.read_timeout_ms)
            .with_context_path(&config.context_path)
            .with_auth_endpoint(admin_api_path::AUTH_LOGIN);

        let http_client = BatataHttpClient::new(http_config).await?;
        Ok(Self { http_client })
    }

    /// Create a new MaintainerClient with a pre-generated token, bypassing HTTP login.
    /// Used for embedded mode where the server generates a JWT token locally.
    pub fn new_with_token(
        config: MaintainerClientConfig,
        token: String,
        ttl_seconds: i64,
    ) -> anyhow::Result<Self> {
        let http_config = HttpClientConfig::with_servers(config.server_addrs)
            .with_auth(&config.username, &config.password)
            .with_timeouts(config.connect_timeout_ms, config.read_timeout_ms)
            .with_context_path(&config.context_path)
            .with_auth_endpoint(admin_api_path::AUTH_LOGIN);

        let http_client = BatataHttpClient::new_with_token(http_config, token, ttl_seconds)?;
        Ok(Self { http_client })
    }

    /// Create a new MaintainerClient from a single server address
    pub async fn from_server_addr(
        addr: &str,
        username: &str,
        password: &str,
    ) -> anyhow::Result<Self> {
        let config = MaintainerClientConfig {
            server_addrs: vec![addr.to_string()],
            username: username.to_string(),
            password: password.to_string(),
            ..Default::default()
        };
        Self::new(config).await
    }

    // ============================================================================
    // Server State / Liveness / Readiness APIs
    // ============================================================================

    pub async fn server_state(&self) -> anyhow::Result<HashMap<String, Option<String>>> {
        let response: ApiResponse<HashMap<String, Option<String>>> = self
            .http_client
            .get(admin_api_path::SERVER_STATE)
            .await
            .unwrap_or(ApiResponse {
                code: 200,
                message: String::new(),
                data: HashMap::new(),
            });
        Ok(response.data)
    }

    pub async fn server_readiness(&self) -> bool {
        self.cluster_health().await.is_ok()
    }

    pub async fn liveness(&self) -> anyhow::Result<bool> {
        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .get(admin_api_path::SERVER_LIVENESS)
            .await?;
        Ok(response.code == 0 || response.code == 200)
    }

    pub async fn readiness(&self) -> anyhow::Result<bool> {
        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .get(admin_api_path::SERVER_READINESS)
            .await?;
        Ok(response.code == 0 || response.code == 200)
    }

    // ============================================================================
    // Core Ops APIs (Raft, ID Generators, Log Level)
    // ============================================================================

    pub async fn raft_ops(
        &self,
        command: &str,
        value: &str,
        group_id: &str,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            command: &'a str,
            value: &'a str,
            group_id: &'a str,
        }

        let response: ApiResponse<String> = self
            .http_client
            .post_form(
                admin_api_path::CORE_OPS_RAFT,
                &Form {
                    command,
                    value,
                    group_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn get_id_generators(&self) -> anyhow::Result<Vec<IdGeneratorInfo>> {
        let response: ApiResponse<Vec<IdGeneratorInfo>> = self
            .http_client
            .get(admin_api_path::CORE_OPS_ID_GENERATOR)
            .await?;
        Ok(response.data)
    }

    pub async fn update_core_log_level(
        &self,
        log_name: &str,
        log_level: &str,
    ) -> anyhow::Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            log_name: &'a str,
            log_level: &'a str,
        }

        let _response: ApiResponse<serde_json::Value> = self
            .http_client
            .put_form(
                admin_api_path::CORE_OPS_LOG,
                &Form {
                    log_name,
                    log_level,
                },
            )
            .await?;
        Ok(())
    }

    // ============================================================================
    // Cluster / Loader APIs
    // ============================================================================

    pub async fn cluster_members(&self) -> anyhow::Result<Vec<Member>> {
        let response: ApiResponse<Vec<Member>> = self
            .http_client
            .get(admin_api_path::CLUSTER_NODE_LIST)
            .await?;
        Ok(response.data)
    }

    pub async fn cluster_members_filtered(
        &self,
        address: Option<&str>,
        state: Option<&str>,
    ) -> anyhow::Result<Vec<Member>> {
        #[derive(Serialize)]
        struct Query<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            keyword: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<&'a str>,
        }

        let response: ApiResponse<Vec<Member>> = self
            .http_client
            .get_with_query(
                admin_api_path::CLUSTER_NODE_LIST,
                &Query {
                    keyword: address,
                    state,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn cluster_health(&self) -> anyhow::Result<ClusterHealthResponse> {
        #[derive(Deserialize)]
        struct HealthResponse {
            healthy: bool,
        }

        let response: ApiResponse<HealthResponse> = self
            .http_client
            .get(admin_api_path::CLUSTER_SELF_HEALTH)
            .await?;

        let members = self.cluster_members().await.unwrap_or_default();
        let up_count = members.iter().filter(|m| m.state == "UP").count();
        let total = members.len();

        Ok(ClusterHealthResponse {
            is_healthy: response.data.healthy,
            summary: ClusterHealthSummary {
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

    pub async fn cluster_self(&self) -> anyhow::Result<SelfMemberResponse> {
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

        let response: ApiResponse<NodeSelfResponse> =
            self.http_client.get(admin_api_path::CLUSTER_SELF).await?;

        let node = response.data;
        let version = node
            .extend_info
            .as_ref()
            .and_then(|info| info.get("version"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let is_standalone = self
            .cluster_member_count()
            .await
            .map(|c| c <= 1)
            .unwrap_or(true);

        Ok(SelfMemberResponse {
            ip: node.ip,
            port: node.port,
            address: node.address,
            state: node.state,
            is_standalone,
            version,
        })
    }

    pub async fn cluster_member(&self, address: &str) -> anyhow::Result<Option<Member>> {
        #[derive(Serialize)]
        struct Query<'a> {
            keyword: &'a str,
        }

        let response: ApiResponse<Vec<Member>> = self
            .http_client
            .get_with_query(
                admin_api_path::CLUSTER_NODE_LIST,
                &Query { keyword: address },
            )
            .await?;
        Ok(response.data.into_iter().find(|m| m.address == address))
    }

    pub async fn cluster_member_count(&self) -> anyhow::Result<usize> {
        let members = self.cluster_members().await?;
        Ok(members.len())
    }

    pub async fn update_lookup_mode(&self, mode: &str) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        struct Form<'a> {
            r#type: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .put_form(admin_api_path::CLUSTER_LOOKUP, &Form { r#type: mode })
            .await?;
        Ok(response.data)
    }

    pub async fn get_current_clients(&self) -> anyhow::Result<HashMap<String, ConnectionInfo>> {
        let response: ApiResponse<HashMap<String, ConnectionInfo>> = self
            .http_client
            .get(admin_api_path::CORE_LOADER_CURRENT)
            .await?;
        Ok(response.data)
    }

    pub async fn reload_connection_count(
        &self,
        count: Option<i32>,
        redirect_address: Option<&str>,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            count: Option<i32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            redirect_address: Option<&'a str>,
        }

        let response: ApiResponse<String> = self
            .http_client
            .get_with_query(
                admin_api_path::CORE_LOADER_RELOAD,
                &Query {
                    count,
                    redirect_address,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn smart_reload_cluster(
        &self,
        loader_factor: Option<&str>,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            loader_factor: Option<&'a str>,
        }

        let response: ApiResponse<String> = self
            .http_client
            .get_with_query(
                admin_api_path::CORE_LOADER_SMART_RELOAD,
                &Query { loader_factor },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn reload_single_client(
        &self,
        connection_id: &str,
        redirect_address: &str,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            connection_id: &'a str,
            redirect_address: &'a str,
        }

        let response: ApiResponse<String> = self
            .http_client
            .get_with_query(
                admin_api_path::CORE_LOADER_RELOAD_CLIENT,
                &Query {
                    connection_id,
                    redirect_address,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn get_cluster_loader_metrics(&self) -> anyhow::Result<ServerLoaderMetrics> {
        let response: ApiResponse<ServerLoaderMetrics> = self
            .http_client
            .get(admin_api_path::CORE_LOADER_METRICS)
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // Namespace APIs
    // ============================================================================

    pub async fn namespace_list(&self) -> anyhow::Result<Vec<Namespace>> {
        let response: ApiResponse<Vec<Namespace>> =
            self.http_client.get(admin_api_path::NAMESPACE_LIST).await?;
        Ok(response.data)
    }

    pub async fn namespace_get(&self, namespace_id: &str) -> anyhow::Result<Namespace> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
        }

        let response: ApiResponse<Namespace> = self
            .http_client
            .get_with_query(admin_api_path::NAMESPACE, &Query { namespace_id })
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
                admin_api_path::NAMESPACE,
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
                admin_api_path::NAMESPACE,
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
            .delete_with_query(admin_api_path::NAMESPACE, &Query { namespace_id })
            .await?;
        Ok(response.data)
    }

    pub async fn namespace_exists(&self, namespace_id: &str) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            custom_namespace_id: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .get_with_query(
                admin_api_path::NAMESPACE_EXIST,
                &Query {
                    custom_namespace_id: namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // Config CRUD APIs
    // ============================================================================

    pub async fn config_get(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigDetailInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<Option<ConfigDetailInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::CONFIG,
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
    pub async fn config_search(
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
                admin_api_path::CONFIG_LIST,
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
    pub async fn config_search_by_detail(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        search: &str,
        config_detail: &str,
        config_type: &str,
        config_tags: &str,
        app_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigBasicInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
            search: &'a str,
            config_detail: &'a str,
            r#type: &'a str,
            config_tags: &'a str,
            app_name: &'a str,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<Page<ConfigBasicInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::CONFIG_SEARCH,
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                    search,
                    config_detail,
                    r#type: config_type,
                    config_tags,
                    app_name,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn config_publish(
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
                admin_api_path::CONFIG,
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

    pub async fn config_update_metadata(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        description: &str,
        config_tags: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
            desc: &'a str,
            config_tags: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .put_form(
                admin_api_path::CONFIG_METADATA,
                &Form {
                    data_id,
                    group_name,
                    namespace_id,
                    desc: description,
                    config_tags,
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
                admin_api_path::CONFIG,
                &Query {
                    data_id,
                    group_name,
                    tenant: namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn config_batch_delete(&self, ids: &[i64]) -> anyhow::Result<bool> {
        let response: ApiResponse<bool> = self
            .http_client
            .delete_with_query(
                admin_api_path::CONFIG_BATCH_DELETE,
                &[("ids", &serde_json::to_string(ids)?)],
            )
            .await?;
        Ok(response.data)
    }

    pub async fn config_clone(
        &self,
        namespace_id: &str,
        clone_infos: &[ConfigCloneInfo],
        src_user: &str,
        policy: SameConfigPolicy,
    ) -> anyhow::Result<HashMap<String, serde_json::Value>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            clone_infos: &'a str,
            src_user: &'a str,
            policy: &'a str,
        }

        let clone_infos_json = serde_json::to_string(clone_infos)?;
        let policy_str = format!("{}", policy);

        let response: ApiResponse<HashMap<String, serde_json::Value>> = self
            .http_client
            .post_form(
                admin_api_path::CONFIG_CLONE,
                &Form {
                    namespace_id,
                    clone_infos: &clone_infos_json,
                    src_user,
                    policy: &policy_str,
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

        let query = Query {
            namespace_id,
            group,
            data_ids,
            app_name,
        };
        let query_string = serde_urlencoded::to_string(&query)?;
        let path = format!("{}?{}", admin_api_path::CONFIG_EXPORT, query_string);

        self.http_client.get_bytes(&path).await
    }

    pub async fn config_import(
        &self,
        file_data: Vec<u8>,
        namespace_id: &str,
        policy: SameConfigPolicy,
    ) -> anyhow::Result<ImportResult> {
        let query_string = serde_urlencoded::to_string([
            ("namespace_id", namespace_id),
            ("policy", &format!("{}", policy)),
        ])?;
        let path = format!("{}?{}", admin_api_path::CONFIG_IMPORT, query_string);

        let part = reqwest::multipart::Part::bytes(file_data)
            .file_name("import.zip")
            .mime_str("application/zip")?;
        let form = reqwest::multipart::Form::new().part("file", part);

        let response: ApiResponse<ImportResult> =
            self.http_client.post_multipart(&path, form).await?;
        Ok(response.data)
    }

    // ============================================================================
    // Config Beta APIs
    // ============================================================================

    pub async fn config_get_beta(
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
                admin_api_path::CONFIG_BETA,
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
    pub async fn config_publish_beta(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        content: &str,
        app_name: &str,
        src_user: &str,
        config_tags: &str,
        desc: &str,
        r#type: &str,
        beta_ips: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
            content: &'a str,
            app_name: &'a str,
            src_user: &'a str,
            config_tags: &'a str,
            desc: &'a str,
            r#type: &'a str,
            beta_ips: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .post_form(
                admin_api_path::CONFIG_BETA_PUBLISH,
                &Form {
                    data_id,
                    group_name,
                    namespace_id,
                    content,
                    app_name,
                    src_user,
                    config_tags,
                    desc,
                    r#type,
                    beta_ips,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn config_stop_beta(
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
            namespace_id: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .delete_with_query(
                admin_api_path::CONFIG_BETA_STOP,
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // Config History APIs
    // ============================================================================

    pub async fn history_get(
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
                admin_api_path::CONFIG_HISTORY,
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

    pub async fn history_list(
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
                admin_api_path::CONFIG_HISTORY_LIST,
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

    pub async fn history_configs_by_namespace(
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
            .get_with_query(
                admin_api_path::CONFIG_HISTORY_CONFIGS,
                &Query { namespace_id },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn history_get_previous(
        &self,
        id: i64,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigHistoryDetailInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            id: i64,
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<Option<ConfigHistoryDetailInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::CONFIG_HISTORY_PREVIOUS,
                &Query {
                    id,
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // Config Listener APIs
    // ============================================================================

    pub async fn config_listeners(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<ConfigListenerInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<ConfigListenerInfo> = self
            .http_client
            .get_with_query(
                admin_api_path::CONFIG_LISTENER,
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn config_listeners_with_aggregation(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        aggregation: bool,
    ) -> anyhow::Result<ConfigListenerInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
            aggregation: bool,
        }

        let response: ApiResponse<ConfigListenerInfo> = self
            .http_client
            .get_with_query(
                admin_api_path::CONFIG_LISTENER,
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                    aggregation,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn config_listeners_by_ip(
        &self,
        ip: &str,
        all: bool,
        namespace_id: &str,
        aggregation: bool,
    ) -> anyhow::Result<ConfigListenerInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            ip: &'a str,
            all: bool,
            namespace_id: &'a str,
            aggregation: bool,
        }

        let response: ApiResponse<ConfigListenerInfo> = self
            .http_client
            .get_with_query(
                admin_api_path::CONFIG_LISTENER_IP,
                &Query {
                    ip,
                    all,
                    namespace_id,
                    aggregation,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // Config Ops APIs
    // ============================================================================

    pub async fn config_update_local_cache(&self) -> anyhow::Result<String> {
        let response: ApiResponse<String> = self
            .http_client
            .post_form(admin_api_path::CONFIG_OPS_LOCAL_CACHE, &[("", "")])
            .await?;
        Ok(response.data)
    }

    pub async fn config_set_log_level(
        &self,
        log_name: &str,
        log_level: &str,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            log_name: &'a str,
            log_level: &'a str,
        }

        let response: ApiResponse<String> = self
            .http_client
            .put_form(
                admin_api_path::CONFIG_OPS_LOG,
                &Form {
                    log_name,
                    log_level,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // Service APIs
    // ============================================================================

    #[allow(clippy::too_many_arguments)]
    pub async fn service_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ServiceView>> {
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

        let response: ApiResponse<Page<ServiceView>> = self
            .http_client
            .get_with_query(
                admin_api_path::SERVICE_LIST,
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

    #[allow(clippy::too_many_arguments)]
    pub async fn service_list_with_options(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ignore_empty_service: bool,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ServiceView>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            service_name: &'a str,
            ignore_empty_service: bool,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<Page<ServiceView>> = self
            .http_client
            .get_with_query(
                admin_api_path::SERVICE_LIST,
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    ignore_empty_service,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn service_list_with_detail(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ServiceDetailInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            group_name: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            service_name: &'a str,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<Page<ServiceDetailInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::SERVICE_LIST_DETAIL,
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
    ) -> anyhow::Result<ServiceDetailInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
        }

        let response: ApiResponse<ServiceDetailInfo> = self
            .http_client
            .get_with_query(
                admin_api_path::SERVICE_DETAIL,
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
    ) -> anyhow::Result<String> {
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

        let response: ApiResponse<String> = self
            .http_client
            .post_json(
                admin_api_path::SERVICE,
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
    ) -> anyhow::Result<String> {
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

        let response: ApiResponse<String> = self
            .http_client
            .put_form(
                admin_api_path::SERVICE,
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
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
        }

        let response: ApiResponse<String> = self
            .http_client
            .delete_with_query(
                admin_api_path::SERVICE,
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn service_subscribers(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<SubscriberInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<Page<SubscriberInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::SUBSCRIBER_LIST,
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

    pub async fn service_subscribers_with_aggregation(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u64,
        page_size: u64,
        aggregation: bool,
    ) -> anyhow::Result<Page<SubscriberInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            page_no: u64,
            page_size: u64,
            aggregation: bool,
        }

        let response: ApiResponse<Page<SubscriberInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::SUBSCRIBER_LIST,
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    page_no,
                    page_size,
                    aggregation,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn service_selector_types(&self) -> anyhow::Result<Vec<String>> {
        let response: ApiResponse<Vec<String>> = self
            .http_client
            .get(admin_api_path::SERVICE_SELECTOR_TYPES)
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // Instance APIs
    // ============================================================================

    pub async fn instance_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> anyhow::Result<Vec<Instance>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            cluster_name: &'a str,
        }

        let response: ApiResponse<Vec<Instance>> = self
            .http_client
            .get_with_query(
                admin_api_path::INSTANCE_LIST,
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

    pub async fn instance_list_with_healthy_filter(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        healthy_only: bool,
    ) -> anyhow::Result<Vec<Instance>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            cluster_name: &'a str,
            healthy_only: bool,
        }

        let response: ApiResponse<Vec<Instance>> = self
            .http_client
            .get_with_query(
                admin_api_path::INSTANCE_LIST,
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    cluster_name,
                    healthy_only,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn instance_detail(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) -> anyhow::Result<Instance> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            ip: &'a str,
            port: i32,
            #[serde(skip_serializing_if = "str::is_empty")]
            cluster_name: &'a str,
        }

        let response: ApiResponse<Instance> = self
            .http_client
            .get_with_query(
                admin_api_path::INSTANCE_DETAIL,
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    ip,
                    port,
                    cluster_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn instance_register(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            ip: &'a str,
            port: i32,
            weight: f64,
            enabled: bool,
            healthy: bool,
            ephemeral: bool,
            #[serde(skip_serializing_if = "str::is_empty")]
            cluster_name: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            metadata: &'a str,
        }

        let metadata_str = if instance.metadata.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&instance.metadata)?
        };

        let response: ApiResponse<String> = self
            .http_client
            .post_form(
                admin_api_path::INSTANCE,
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    ip: &instance.ip,
                    port: instance.port,
                    weight: instance.weight,
                    enabled: instance.enabled,
                    healthy: instance.healthy,
                    ephemeral: instance.ephemeral,
                    cluster_name: &instance.cluster_name,
                    metadata: &metadata_str,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn instance_deregister(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            ip: &'a str,
            port: i32,
            #[serde(skip_serializing_if = "str::is_empty")]
            cluster_name: &'a str,
            ephemeral: bool,
        }

        let response: ApiResponse<String> = self
            .http_client
            .delete_with_query(
                admin_api_path::INSTANCE,
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    ip: &instance.ip,
                    port: instance.port,
                    cluster_name: &instance.cluster_name,
                    ephemeral: instance.ephemeral,
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
    ) -> anyhow::Result<String> {
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

        let response: ApiResponse<String> = self
            .http_client
            .put_form(
                admin_api_path::INSTANCE,
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

    pub async fn instance_partial_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            ip: &'a str,
            port: i32,
            #[serde(skip_serializing_if = "str::is_empty")]
            cluster_name: &'a str,
            weight: f64,
            enabled: bool,
            #[serde(skip_serializing_if = "str::is_empty")]
            metadata: &'a str,
        }

        let metadata_str = if instance.metadata.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&instance.metadata)?
        };

        let response: ApiResponse<String> = self
            .http_client
            .put_form(
                &format!("{}/partial", admin_api_path::INSTANCE),
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    ip: &instance.ip,
                    port: instance.port,
                    cluster_name: &instance.cluster_name,
                    weight: instance.weight,
                    enabled: instance.enabled,
                    metadata: &metadata_str,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn instance_batch_update_metadata(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instances_json: &str,
        metadata: &HashMap<String, String>,
    ) -> anyhow::Result<InstanceMetadataBatchResult> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            instances: &'a str,
            metadata: &'a str,
        }

        let metadata_str = serde_json::to_string(metadata)?;

        let response: ApiResponse<InstanceMetadataBatchResult> = self
            .http_client
            .put_form(
                admin_api_path::INSTANCE_METADATA_BATCH,
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    instances: instances_json,
                    metadata: &metadata_str,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn instance_batch_delete_metadata(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instances_json: &str,
        metadata: &HashMap<String, String>,
    ) -> anyhow::Result<InstanceMetadataBatchResult> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            instances: &'a str,
            metadata: &'a str,
        }

        let metadata_str = serde_json::to_string(metadata)?;

        let response: ApiResponse<InstanceMetadataBatchResult> = self
            .http_client
            .delete_with_query(
                admin_api_path::INSTANCE_METADATA_BATCH,
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    instances: instances_json,
                    metadata: &metadata_str,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // Naming Health APIs
    // ============================================================================

    pub async fn update_instance_health_status(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        healthy: bool,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            ip: &'a str,
            port: i32,
            healthy: bool,
        }

        let response: ApiResponse<String> = self
            .http_client
            .put_form(
                admin_api_path::NAMING_HEALTH_INSTANCE,
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    ip,
                    port,
                    healthy,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn get_health_checkers(&self) -> anyhow::Result<HashMap<String, serde_json::Value>> {
        let response: ApiResponse<HashMap<String, serde_json::Value>> = self
            .http_client
            .get(admin_api_path::NAMING_HEALTH_CHECKERS)
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // Naming Cluster APIs
    // ============================================================================

    pub async fn update_naming_cluster(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster: &ClusterInfo,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            cluster_name: &'a str,
            health_checker: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            metadata: &'a str,
        }

        let health_checker_str = cluster
            .health_checker
            .as_ref()
            .map(|h| serde_json::to_string(h).unwrap_or_default())
            .unwrap_or_default();

        let metadata_str = if cluster.metadata.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&cluster.metadata)?
        };

        let response: ApiResponse<String> = self
            .http_client
            .put_form(
                admin_api_path::NAMING_CLUSTER,
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    cluster_name: &cluster.cluster_name,
                    health_checker: &health_checker_str,
                    metadata: &metadata_str,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // Naming Ops APIs
    // ============================================================================

    pub async fn naming_metrics(&self, only_status: bool) -> anyhow::Result<MetricsInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query {
            only_status: bool,
        }

        let response: ApiResponse<MetricsInfo> = self
            .http_client
            .get_with_query(admin_api_path::NAMING_OPS_METRICS, &Query { only_status })
            .await?;
        Ok(response.data)
    }

    pub async fn naming_set_log_level(
        &self,
        log_name: &str,
        log_level: &str,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            log_name: &'a str,
            log_level: &'a str,
        }

        let response: ApiResponse<String> = self
            .http_client
            .put_form(
                admin_api_path::NAMING_OPS_LOG,
                &Form {
                    log_name,
                    log_level,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // Naming Client APIs
    // ============================================================================

    pub async fn client_list(&self) -> anyhow::Result<Vec<String>> {
        let response: ApiResponse<Vec<String>> = self
            .http_client
            .get(admin_api_path::NAMING_CLIENT_LIST)
            .await?;
        Ok(response.data)
    }

    pub async fn client_detail(&self, client_id: &str) -> anyhow::Result<ClientSummaryInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            client_id: &'a str,
        }

        let response: ApiResponse<ClientSummaryInfo> = self
            .http_client
            .get_with_query(admin_api_path::NAMING_CLIENT, &Query { client_id })
            .await?;
        Ok(response.data)
    }

    pub async fn client_published_services(
        &self,
        client_id: &str,
    ) -> anyhow::Result<Vec<ClientServiceInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            client_id: &'a str,
        }

        let response: ApiResponse<Vec<ClientServiceInfo>> = self
            .http_client
            .get_with_query(admin_api_path::NAMING_CLIENT_PUBLISH, &Query { client_id })
            .await?;
        Ok(response.data)
    }

    pub async fn client_subscribed_services(
        &self,
        client_id: &str,
    ) -> anyhow::Result<Vec<ClientServiceInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            client_id: &'a str,
        }

        let response: ApiResponse<Vec<ClientServiceInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::NAMING_CLIENT_SUBSCRIBE,
                &Query { client_id },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn service_published_clients(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: Option<&str>,
        port: Option<i32>,
    ) -> anyhow::Result<Vec<ClientPublisherInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            ip: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            port: Option<i32>,
        }

        let response: ApiResponse<Vec<ClientPublisherInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::NAMING_CLIENT_SERVICE_PUBLISH,
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    ip,
                    port,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn service_subscribed_clients(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: Option<&str>,
        port: Option<i32>,
    ) -> anyhow::Result<Vec<ClientSubscriberInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            ip: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            port: Option<i32>,
        }

        let response: ApiResponse<Vec<ClientSubscriberInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::NAMING_CLIENT_SERVICE_SUBSCRIBE,
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    ip,
                    port,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // AI MCP APIs
    // ============================================================================

    pub async fn mcp_server_list(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<McpServerBasicInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            mcp_name: &'a str,
            search: &'a str,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<Page<McpServerBasicInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_MCP_LIST,
                &Query {
                    namespace_id,
                    mcp_name,
                    search: "accurate",
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn mcp_server_search(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<McpServerBasicInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            mcp_name: &'a str,
            search: &'a str,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<Page<McpServerBasicInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_MCP_LIST,
                &Query {
                    namespace_id,
                    mcp_name,
                    search: "blur",
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn mcp_server_detail(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        mcp_id: &str,
        version: &str,
    ) -> anyhow::Result<McpServerDetailInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            mcp_name: &'a str,
            mcp_id: &'a str,
            version: &'a str,
        }

        let response: ApiResponse<McpServerDetailInfo> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_MCP,
                &Query {
                    namespace_id,
                    mcp_name,
                    mcp_id,
                    version,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn mcp_server_create(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        server_spec: &McpServerBasicInfo,
        tool_spec: Option<&McpToolSpecification>,
        endpoint_spec: Option<&McpEndpointSpec>,
    ) -> anyhow::Result<String> {
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("namespaceId".to_string(), namespace_id.to_string());
        params.insert("mcpName".to_string(), mcp_name.to_string());
        params.insert(
            "serverSpecification".to_string(),
            serde_json::to_string(server_spec)?,
        );
        if let Some(tool) = tool_spec {
            params.insert(
                "toolSpecification".to_string(),
                serde_json::to_string(tool)?,
            );
        }
        if let Some(endpoint) = endpoint_spec {
            params.insert(
                "endpointSpecification".to_string(),
                serde_json::to_string(endpoint)?,
            );
        }

        let response: ApiResponse<String> = self
            .http_client
            .post_form(admin_api_path::AI_MCP, &params)
            .await?;
        Ok(response.data)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn mcp_server_update(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        is_latest: bool,
        server_spec: &McpServerBasicInfo,
        tool_spec: Option<&McpToolSpecification>,
        endpoint_spec: Option<&McpEndpointSpec>,
        override_existing: bool,
    ) -> anyhow::Result<bool> {
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("namespaceId".to_string(), namespace_id.to_string());
        params.insert("mcpName".to_string(), mcp_name.to_string());
        params.insert("latest".to_string(), is_latest.to_string());
        params.insert(
            "overrideExisting".to_string(),
            override_existing.to_string(),
        );
        params.insert(
            "serverSpecification".to_string(),
            serde_json::to_string(server_spec)?,
        );
        if let Some(tool) = tool_spec {
            params.insert(
                "toolSpecification".to_string(),
                serde_json::to_string(tool)?,
            );
        }
        if let Some(endpoint) = endpoint_spec {
            params.insert(
                "endpointSpecification".to_string(),
                serde_json::to_string(endpoint)?,
            );
        }

        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .put_form(admin_api_path::AI_MCP, &params)
            .await?;
        Ok(response.code == 0 || response.code == 200)
    }

    pub async fn mcp_server_delete(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        mcp_id: &str,
        version: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            mcp_name: &'a str,
            mcp_id: &'a str,
            version: &'a str,
        }

        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .delete_with_query(
                admin_api_path::AI_MCP,
                &Query {
                    namespace_id,
                    mcp_name,
                    mcp_id,
                    version,
                },
            )
            .await?;
        Ok(response.code == 0 || response.code == 200)
    }

    // ============================================================================
    // AI Agent (A2A) APIs
    // ============================================================================

    pub async fn agent_register(
        &self,
        agent_card: &AgentCard,
        namespace_id: &str,
        registration_type: &str,
    ) -> anyhow::Result<bool> {
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("agentCard".to_string(), serde_json::to_string(agent_card)?);
        params.insert("namespaceId".to_string(), namespace_id.to_string());
        params.insert("agentName".to_string(), agent_card.basic_info.name.clone());
        params.insert(
            "registrationType".to_string(),
            registration_type.to_string(),
        );

        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .post_form(admin_api_path::AI_AGENT, &params)
            .await?;
        Ok(response.code == 0 || response.code == 200)
    }

    pub async fn agent_get(
        &self,
        agent_name: &str,
        namespace_id: &str,
        registration_type: &str,
    ) -> anyhow::Result<AgentCardDetailInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            agent_name: &'a str,
            namespace_id: &'a str,
            registration_type: &'a str,
        }

        let response: ApiResponse<AgentCardDetailInfo> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENT,
                &Query {
                    agent_name,
                    namespace_id,
                    registration_type,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn agent_update(
        &self,
        agent_card: &AgentCard,
        namespace_id: &str,
        set_as_latest: bool,
        registration_type: &str,
    ) -> anyhow::Result<bool> {
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("agentCard".to_string(), serde_json::to_string(agent_card)?);
        params.insert("namespaceId".to_string(), namespace_id.to_string());
        params.insert("agentName".to_string(), agent_card.basic_info.name.clone());
        params.insert("setAsLatest".to_string(), set_as_latest.to_string());
        params.insert(
            "registrationType".to_string(),
            registration_type.to_string(),
        );

        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .put_form(admin_api_path::AI_AGENT, &params)
            .await?;
        Ok(response.code == 0 || response.code == 200)
    }

    pub async fn agent_delete(
        &self,
        agent_name: &str,
        namespace_id: &str,
        version: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            agent_name: &'a str,
            namespace_id: &'a str,
            version: &'a str,
        }

        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .delete_with_query(
                admin_api_path::AI_AGENT,
                &Query {
                    agent_name,
                    namespace_id,
                    version,
                },
            )
            .await?;
        Ok(response.code == 0 || response.code == 200)
    }

    pub async fn agent_list_versions(
        &self,
        agent_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<AgentVersionDetail>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            agent_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<Vec<AgentVersionDetail>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENT_VERSION_LIST,
                &Query {
                    agent_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn agent_list(
        &self,
        namespace_id: &str,
        agent_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentCardVersionInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            agent_name: &'a str,
            search: &'a str,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<Page<AgentCardVersionInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENT_LIST,
                &Query {
                    namespace_id,
                    agent_name,
                    search: "accurate",
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    pub async fn agent_search(
        &self,
        namespace_id: &str,
        agent_name_pattern: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentCardVersionInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            agent_name: &'a str,
            search: &'a str,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<Page<AgentCardVersionInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENT_LIST,
                &Query {
                    namespace_id,
                    agent_name: agent_name_pattern,
                    search: "blur",
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }
}
