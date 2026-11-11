//! MaintainerClient - facade for all Admin API operations

use std::collections::HashMap;

use batata_client::model::OkOrBool;
use batata_client::{BatataHttpClient, HttpClientConfig, TlsOptions};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    config::MaintainerClientConfig,
    constants::admin_api_path,
    model::common::ApiResponse,
    model::{
        AgentCard, AgentCardDetailInfo, AgentCardVersionInfo, AgentDraftCreateForm,
        AgentDraftUpdateForm, AgentLabelsUpdateForm, AgentSpec, AgentSpecBasicInfo, AgentSpecMeta,
        AgentSpecSummary, AgentSummary, AgentUpdateForm, AgentVersionDetail, AgentVersionInfo,
        BatchUploadResult, ClientPublisherInfo, ClientServiceInfo, ClientSubscriberInfo,
        ClientSummaryInfo, ClusterHealthResponse, ClusterHealthSummary, ClusterInfo,
        ConfigBasicInfo, ConfigCloneInfo, ConfigDetailInfo, ConfigGrayInfo, ConfigHistoryBasicInfo,
        ConfigHistoryDetailInfo, ConfigListenerInfo, ConnectionInfo, IdGeneratorInfo, ImportResult,
        Instance, InstanceMetadataBatchResult, McpEndpointSpec, McpServerBasicInfo,
        McpServerDetailInfo, McpServerVersionInfo, McpToolSpecification, Member, MetricsInfo,
        Namespace, Page, PipelineExecution, PluginAvailability, PluginDetail, PluginInfo,
        PromptMetaInfo, PromptMetaSummary, PromptVersionInfo, PromptVersionSummary,
        RuntimeEndpointSnapshot, SameConfigPolicy, SelfMemberResponse, ServerLoaderMetrics,
        ServiceDetailInfo, ServiceView, Skill, SkillMeta, SkillSummary, SkillUploadPrecheckResult,
        SubscriberInfo,
    },
};

/// Admin/maintainer HTTP client for Batata
pub struct MaintainerClient {
    http_client: BatataHttpClient,
}

/// Render a service-CRD answer the way the maintainer API reports it.
///
/// Upstream replies with the string `"ok"` while Batata's own admin API replies
/// with the boolean `true`, and both mean the same thing to a caller. Reading
/// the payload as `OkOrBool` and collapsing it here keeps the published
/// `Result<String>` signature tolerant of either.
fn ok_or_bool_to_string(value: OkOrBool) -> String {
    if value.0 {
        "ok".to_string()
    } else {
        String::new()
    }
}

/// Optional query values are omitted when blank, like upstream `putIfNotBlank`.
fn non_empty(value: &str) -> Option<&str> {
    Some(value).filter(|v| !v.is_empty())
}

/// Identity parameters shared by the protocol-neutral agent endpoints.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentIdentityQuery<'a> {
    namespace_id: &'a str,
    agent_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
}

/// Target parameters shared by the agent version endpoints.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentVersionQuery<'a> {
    namespace_id: &'a str,
    agent_name: &'a str,
    version: &'a str,
}

/// Query parameters for the agent list endpoint.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentListQuery<'a> {
    namespace_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    biz_tag: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order_by: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_no: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_size: Option<u64>,
}

// Query shapes for the AI resource facades (`F-NAC-MAINT-007`). All field
// names are camelCase on the wire, matching both the GA maintainer client and
// batata's `web::Query` extractors.

/// Identity of a skill or one of its versions.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillIdentityQuery<'a> {
    namespace_id: &'a str,
    skill_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
}

/// Filters accepted by the GA `listSkills` overloads. Batata currently applies
/// only `namespaceId`, `skillName`, `search`, `orderBy` and pagination; the
/// remaining filters are forwarded but ignored by the server.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillListQuery<'a> {
    namespace_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    skill_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order_by: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    biz_tag: Option<&'a str>,
    page_no: u64,
    page_size: u64,
}

/// Query parameters shared by skill and agentspec ZIP uploads.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiUploadQuery<'a> {
    namespace_id: &'a str,
    overwrite: bool,
    /// GA `targetVersion`: explicit version override for the uploaded draft.
    #[serde(skip_serializing_if = "Option::is_none")]
    target_version: Option<&'a str>,
    /// GA `uploadAction`: `CREATE_DRAFT` / `OVERWRITE_DRAFT` / `DELETE_DRAFT_AND_CREATE`.
    #[serde(skip_serializing_if = "Option::is_none")]
    upload_action: Option<&'a str>,
    /// GA `autoPublishIfNew`: force-publish the first upload automatically.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    auto_publish_if_new: bool,
    /// GA `commitMsg`: stored as the version-level description.
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_msg: Option<&'a str>,
}

/// Identity of a prompt, optionally narrowed to a version, label or MD5.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptIdentityQuery<'a> {
    namespace_id: &'a str,
    prompt_key: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    md5: Option<&'a str>,
}

/// Filters for the prompt list endpoint.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptListQuery<'a> {
    namespace_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_key: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    biz_tags: Option<&'a str>,
    page_no: u64,
    page_size: u64,
}

/// Pagination for the prompt version history endpoint.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptHistoryQuery<'a> {
    namespace_id: &'a str,
    prompt_key: &'a str,
    page_no: u64,
    page_size: u64,
}

/// Identity of an agentspec or one of its versions.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentSpecIdentityQuery<'a> {
    namespace_id: &'a str,
    agent_spec_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
}

/// Filters for the agentspec list endpoint.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentSpecListQuery<'a> {
    namespace_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_spec_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search: Option<&'a str>,
    page_no: u64,
    page_size: u64,
}

/// Query parameters for a single pipeline execution.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PipelineDetailQuery<'a> {
    pipeline_id: &'a str,
}

/// Filters for the pipeline execution list endpoint.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PipelineListQuery<'a> {
    resource_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    page_no: u64,
    page_size: u64,
}

// Write forms for the AI resource facades (`F-NAC-MAINT-007`, batch 7). GA's
// `ClientHttpProxy.executeSync` sends POST/PUT parameters as an urlencoded form
// body, and batata's skill/prompt/agentspec handlers read `web::Form` while MCP
// reads `AdminForm`/`AdminParams` (query or form). Optional values are omitted
// rather than sent blank, mirroring GA's `putIfNotBlank`. Keeping one struct
// per resource puts every wire name in a single place.

/// Every parameter accepted by the skill lifecycle endpoints.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct SkillWriteForm<'a> {
    namespace_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    skill_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    based_on_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skill_card: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_msg: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    set_as_latest: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    update_latest_label: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    biz_tags: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<&'a str>,
}

/// Every parameter accepted by the prompt lifecycle and legacy endpoints.
///
/// `version` is batata's spelling for the first draft version; GA calls it
/// `targetVersion`. Both are sent so the same call works against either server.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct PromptWriteForm<'a> {
    namespace_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_key: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    based_on_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    template: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_msg: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    biz_tags: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    update_latest_label: Option<bool>,
}

/// Every parameter accepted by the agentspec lifecycle endpoints.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct AgentSpecWriteForm<'a> {
    namespace_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_spec_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    based_on_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_spec_card: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_msg: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    set_as_latest: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    update_latest_label: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    biz_tags: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<&'a str>,
}

/// Parameters accepted by the MCP resource updates not covered by batch 1.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct McpWriteForm<'a> {
    namespace_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    mcp_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<&'a str>,
}

/// GA `listAgentSpecAdminItems` adds three filters to the plain list endpoint.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentSpecAdminListQuery<'a> {
    namespace_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_spec_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order_by: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<&'a str>,
    page_no: u64,
    page_size: u64,
}

/// HTTP verb used by an AI resource write. DELETE endpoints carry their
/// identity in the query string, while POST/PUT carry it in the form body.
#[derive(Clone, Copy)]
enum AiWriteVerb {
    Post,
    Put,
    Delete,
}

/// Build the multipart body used by every AI ADMIN ZIP endpoint.
///
/// Both GA (`ClientHttpProxy.executeMultipartPost`) and batata
/// (`read_zip_field`) look for a part literally named `file`.
fn ai_zip_form(zip_bytes: &[u8]) -> reqwest::multipart::Form {
    reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(zip_bytes.to_vec()).file_name("resource.zip"),
    )
}

impl MaintainerClient {
    /// Create a new MaintainerClient with the given configuration
    pub async fn new(config: MaintainerClientConfig) -> anyhow::Result<Self> {
        let mut http_config = HttpClientConfig::with_servers(config.server_addrs)
            .with_auth(&config.username, &config.password)
            .with_timeouts(config.connect_timeout_ms, config.read_timeout_ms)
            .with_context_path(&config.context_path)
            .with_auth_endpoint(admin_api_path::AUTH_LOGIN);

        if !config.server_identity_key.is_empty() {
            http_config = http_config
                .with_server_identity(&config.server_identity_key, &config.server_identity_value);
        }

        // HTTPS admin endpoints may be terminated with a private CA or require
        // mutual TLS. The maintainer client is HTTP-only, so TLS is not enabled
        // for a gRPC channel here — only the reqwest trust/identity settings.
        http_config = http_config.with_tls(TlsOptions {
            ca_cert_path: config.tls_ca_cert_path.clone(),
            client_cert_path: config.tls_client_cert_path.clone(),
            client_key_path: config.tls_client_key_path.clone(),
            ..TlsOptions::default()
        });

        let http_client = BatataHttpClient::new(http_config).await?;
        Ok(Self { http_client })
    }

    /// Create a MaintainerClient from an existing HTTP client (for testing)
    #[doc(hidden)]
    pub fn from_http_client(http_client: BatataHttpClient) -> Self {
        Self { http_client }
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

    /// Server state.
    ///
    /// # Returns
    /// The `anyhow::Result<HashMap<String, Option<String>>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Server readiness.
    ///
    /// # Returns
    /// The `bool` result.
    pub async fn server_readiness(&self) -> bool {
        self.cluster_health().await.is_ok()
    }

    /// Liveness.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn liveness(&self) -> anyhow::Result<bool> {
        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .get(admin_api_path::SERVER_LIVENESS)
            .await?;
        Ok(response.code == 0 || response.code == 200)
    }

    /// Readiness.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Raft ops.
    ///
    /// # Arguments
    /// * `command` - The `command` parameter.
    /// * `value` - The `value` parameter.
    /// * `group_id` - The `group_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Get id generators.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<IdGeneratorInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn get_id_generators(&self) -> anyhow::Result<Vec<IdGeneratorInfo>> {
        let response: ApiResponse<Vec<IdGeneratorInfo>> = self
            .http_client
            .get(admin_api_path::CORE_OPS_ID_GENERATOR)
            .await?;
        Ok(response.data)
    }

    /// Update core log level.
    ///
    /// # Arguments
    /// * `log_name` - The `log_name` parameter.
    /// * `log_level` - The `log_level` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<()>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Cluster members.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<Member>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn cluster_members(&self) -> anyhow::Result<Vec<Member>> {
        let response: ApiResponse<Vec<Member>> = self
            .http_client
            .get(admin_api_path::CLUSTER_NODE_LIST)
            .await?;
        Ok(response.data)
    }

    /// Cluster members filtered.
    ///
    /// # Arguments
    /// * `address` - The `address` parameter.
    /// * `state` - The `state` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<Member>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Cluster health.
    ///
    /// # Returns
    /// The `anyhow::Result<ClusterHealthResponse>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Cluster self.
    ///
    /// # Returns
    /// The `anyhow::Result<SelfMemberResponse>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Cluster member.
    ///
    /// # Arguments
    /// * `address` - The `address` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Option<Member>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Cluster member count.
    ///
    /// # Returns
    /// The `anyhow::Result<usize>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn cluster_member_count(&self) -> anyhow::Result<usize> {
        let members = self.cluster_members().await?;
        Ok(members.len())
    }

    /// Update lookup mode.
    ///
    /// # Arguments
    /// * `mode` - The `mode` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Get current clients.
    ///
    /// # Returns
    /// The `anyhow::Result<HashMap<String, ConnectionInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn get_current_clients(&self) -> anyhow::Result<HashMap<String, ConnectionInfo>> {
        let response: ApiResponse<HashMap<String, ConnectionInfo>> = self
            .http_client
            .get(admin_api_path::CORE_LOADER_CURRENT)
            .await?;
        Ok(response.data)
    }

    /// Reload connection count.
    ///
    /// # Arguments
    /// * `count` - The `count` parameter.
    /// * `redirect_address` - The `redirect_address` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Smart reload cluster.
    ///
    /// # Arguments
    /// * `loader_factor` - The `loader_factor` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Reload single client.
    ///
    /// # Arguments
    /// * `connection_id` - The `connection_id` parameter.
    /// * `redirect_address` - The `redirect_address` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Get cluster loader metrics.
    ///
    /// # Returns
    /// The `anyhow::Result<ServerLoaderMetrics>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Namespace list.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<Namespace>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn namespace_list(&self) -> anyhow::Result<Vec<Namespace>> {
        let response: ApiResponse<Vec<Namespace>> =
            self.http_client.get(admin_api_path::NAMESPACE_LIST).await?;
        Ok(response.data)
    }

    /// Namespace get.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Namespace>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Namespace create.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `namespace_name` - The `namespace_name` parameter.
    /// * `namespace_desc` - The `namespace_desc` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Namespace update.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `namespace_name` - The `namespace_name` parameter.
    /// * `namespace_desc` - The `namespace_desc` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Namespace delete.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Namespace exists.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn namespace_exists(&self, namespace_id: &str) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
        }

        // Upstream `AbstractCoreMaintainerService#checkNamespaceIdExist` reads
        // `Result<Integer>` (the `tenant_info` row count) and returns `data > 0`.
        let response: ApiResponse<i64> = self
            .http_client
            .get_with_query(admin_api_path::NAMESPACE_EXIST, &Query { namespace_id })
            .await?;
        Ok(response.data > 0)
    }

    // ============================================================================
    // Config CRUD APIs
    // ============================================================================

    /// Config get.
    ///
    /// # Arguments
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Option<ConfigDetailInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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
    /// Config search.
    ///
    /// # Arguments
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `app_name` - The `app_name` parameter.
    /// * `tags` - The `tags` parameter.
    /// * `types` - The `types` parameter.
    /// * `content` - The `content` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<ConfigBasicInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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
    /// Config search by detail.
    ///
    /// # Arguments
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `search` - The `search` parameter.
    /// * `config_detail` - The `config_detail` parameter.
    /// * `config_type` - The `config_type` parameter.
    /// * `config_tags` - The `config_tags` parameter.
    /// * `app_name` - The `app_name` parameter.
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<ConfigBasicInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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
    /// Config publish.
    ///
    /// # Arguments
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `content` - The `content` parameter.
    /// * `app_name` - The `app_name` parameter.
    /// * `config_tags` - The `config_tags` parameter.
    /// * `desc` - The `desc` parameter.
    /// * `r#use` - The `r#use` parameter.
    /// * `effect` - The `effect` parameter.
    /// * `r#type` - The `r#type` parameter.
    /// * `schema` - The `schema` parameter.
    /// * `encrypted_data_key` - The `encrypted_data_key` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Config update metadata.
    ///
    /// # Arguments
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `description` - The `description` parameter.
    /// * `config_tags` - The `config_tags` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Config delete.
    ///
    /// # Arguments
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn config_delete(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<bool> {
        // Upstream `NacosConfigMaintainerServiceImpl#deleteConfig` sends
        // `dataId`, `groupName` and `namespaceId` — not `tenant`. Sending
        // `tenant` made the admin handler fall back to the public namespace.
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

    /// Config batch delete.
    ///
    /// # Arguments
    /// * `ids` - The `ids` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Config clone.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `clone_infos` - The `clone_infos` parameter.
    /// * `src_user` - The `src_user` parameter.
    /// * `policy` - The `policy` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<HashMap<String, serde_json::Value>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn config_clone(
        &self,
        namespace_id: &str,
        clone_infos: &[ConfigCloneInfo],
        src_user: &str,
        policy: SameConfigPolicy,
    ) -> anyhow::Result<HashMap<String, serde_json::Value>> {
        // Upstream `POST /v3/admin/cs/config/clone` reads the descriptors from
        // an `@RequestBody List<ConfigCloneInfo>` while `namespaceId`, `policy`
        // and `srcUser` stay query parameters.
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            policy: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            src_user: &'a str,
        }

        let policy_str = format!("{}", policy);

        let response: ApiResponse<HashMap<String, serde_json::Value>> = self
            .http_client
            .post_json_with_query(
                admin_api_path::CONFIG_CLONE,
                &Query {
                    namespace_id,
                    policy: &policy_str,
                    src_user,
                },
                clone_infos,
            )
            .await?;
        Ok(response.data)
    }

    /// Config export.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group` - The `group` parameter.
    /// * `data_ids` - The `data_ids` parameter.
    /// * `app_name` - The `app_name` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<u8>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Config import.
    ///
    /// # Arguments
    /// * `file_data` - The `file_data` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `policy` - The `policy` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<ImportResult>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Config get beta.
    ///
    /// # Arguments
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Option<ConfigGrayInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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
    /// Config publish beta.
    ///
    /// # Arguments
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `content` - The `content` parameter.
    /// * `app_name` - The `app_name` parameter.
    /// * `src_user` - The `src_user` parameter.
    /// * `config_tags` - The `config_tags` parameter.
    /// * `desc` - The `desc` parameter.
    /// * `r#type` - The `r#type` parameter.
    /// * `beta_ips` - The `beta_ips` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Config stop beta.
    ///
    /// # Arguments
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// History get.
    ///
    /// # Arguments
    /// * `nid` - The `nid` parameter.
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Option<ConfigHistoryDetailInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn history_get(
        &self,
        nid: i64,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigHistoryDetailInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            nid: i64,
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

    /// History list.
    ///
    /// # Arguments
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<ConfigHistoryBasicInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// History configs by namespace.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<ConfigBasicInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// History get previous.
    ///
    /// # Arguments
    /// * `id` - The `id` parameter.
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Option<ConfigHistoryDetailInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Config listeners.
    ///
    /// # Arguments
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<ConfigListenerInfo>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Config listeners with aggregation.
    ///
    /// # Arguments
    /// * `data_id` - The `data_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `aggregation` - The `aggregation` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<ConfigListenerInfo>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Config listeners by ip.
    ///
    /// # Arguments
    /// * `ip` - The `ip` parameter.
    /// * `all` - The `all` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `aggregation` - The `aggregation` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<ConfigListenerInfo>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Config update local cache.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn config_update_local_cache(&self) -> anyhow::Result<String> {
        let response: ApiResponse<String> = self
            .http_client
            .post_form(admin_api_path::CONFIG_OPS_LOCAL_CACHE, &[("", "")])
            .await?;
        Ok(response.data)
    }

    /// Config set log level.
    ///
    /// # Arguments
    /// * `log_name` - The `log_name` parameter.
    /// * `log_level` - The `log_level` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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
    /// Service list.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<ServiceView>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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
    /// Service list with options.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `ignore_empty_service` - The `ignore_empty_service` parameter.
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<ServiceView>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Service list with detail.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<ServiceDetailInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Service get.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<ServiceDetailInfo>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Service create.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `protect_threshold` - The `protect_threshold` parameter.
    /// * `metadata` - The `metadata` parameter.
    /// * `selector` - The `selector` parameter.
    /// * `ephemeral` - The `ephemeral` parameter, mirroring upstream
    ///   `ServiceForm.ephemeral`. Only meaningful on create: upstream
    ///   `ConsoleServiceController.updateService` does not apply it.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn service_create(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f64,
        metadata: &str,
        selector: &str,
        ephemeral: bool,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            protect_threshold: f64,
            #[serde(skip_serializing_if = "str::is_empty")]
            metadata: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            selector: &'a str,
            ephemeral: bool,
        }

        // The admin naming endpoints are form-encoded (a JSON body is rejected
        // with 415), so this mirrors `service_update` below rather than posting
        // JSON. Upstream reaches the same place by appending the parameters to
        // the query string.
        //
        // Upstream answers with the string "ok", Batata's own admin API with
        // `true`; `OkOrBool` reads both.
        let response: ApiResponse<OkOrBool> = self
            .http_client
            .post_form(
                admin_api_path::SERVICE,
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    protect_threshold,
                    metadata,
                    selector,
                    ephemeral,
                },
            )
            .await?;
        Ok(ok_or_bool_to_string(response.data))
    }

    /// Service update.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `protect_threshold` - The `protect_threshold` parameter.
    /// * `metadata` - The `metadata` parameter.
    /// * `selector` - The `selector` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn service_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f64,
        metadata: &str,
        selector: &str,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            protect_threshold: f64,
            #[serde(skip_serializing_if = "str::is_empty")]
            metadata: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            selector: &'a str,
        }

        // See `service_create`: the payload is form-encoded and the answer is
        // either "ok" (upstream) or `true` (Batata).
        let response: ApiResponse<OkOrBool> = self
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
        Ok(ok_or_bool_to_string(response.data))
    }

    /// Service delete.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

        // See `service_create`: "ok" upstream, `true` on Batata.
        let response: ApiResponse<OkOrBool> = self
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
        Ok(ok_or_bool_to_string(response.data))
    }

    /// Service subscribers.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<SubscriberInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Service subscribers with aggregation.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    /// * `aggregation` - The `aggregation` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<SubscriberInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Service selector types.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<String>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Instance list.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `cluster_name` - The `cluster_name` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<Instance>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Instance list with healthy filter.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `cluster_name` - The `cluster_name` parameter.
    /// * `healthy_only` - The `healthy_only` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<Instance>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Instance detail.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `ip` - The `ip` parameter.
    /// * `port` - The `port` parameter.
    /// * `cluster_name` - The `cluster_name` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Instance>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Instance register.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `instance` - The `instance` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Instance deregister.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `instance` - The `instance` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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
    /// Instance update.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `ip` - The `ip` parameter.
    /// * `port` - The `port` parameter.
    /// * `cluster_name` - The `cluster_name` parameter.
    /// * `weight` - The `weight` parameter.
    /// * `enabled` - The `enabled` parameter.
    /// * `metadata` - The `metadata` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Instance partial update.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `instance` - The `instance` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Instance batch update metadata.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `instances_json` - The `instances_json` parameter.
    /// * `metadata` - The `metadata` parameter.
    /// * `String>` - The `String>` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<InstanceMetadataBatchResult>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Instance batch delete metadata.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `instances_json` - The `instances_json` parameter.
    /// * `metadata` - The `metadata` parameter.
    /// * `String>` - The `String>` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<InstanceMetadataBatchResult>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Update instance health status.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `ip` - The `ip` parameter.
    /// * `port` - The `port` parameter.
    /// * `healthy` - The `healthy` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Get health checkers.
    ///
    /// # Returns
    /// The `anyhow::Result<HashMap<String, serde_json::Value>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Update naming cluster.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `cluster` - The `cluster` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Naming metrics.
    ///
    /// # Arguments
    /// * `only_status` - The `only_status` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<MetricsInfo>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Naming set log level.
    ///
    /// # Arguments
    /// * `log_name` - The `log_name` parameter.
    /// * `log_level` - The `log_level` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Client list.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<String>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn client_list(&self) -> anyhow::Result<Vec<String>> {
        let response: ApiResponse<Vec<String>> = self
            .http_client
            .get(admin_api_path::NAMING_CLIENT_LIST)
            .await?;
        Ok(response.data)
    }

    /// Client detail.
    ///
    /// # Arguments
    /// * `client_id` - The `client_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<ClientSummaryInfo>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Client published services.
    ///
    /// # Arguments
    /// * `client_id` - The `client_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<ClientServiceInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Client subscribed services.
    ///
    /// # Arguments
    /// * `client_id` - The `client_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<ClientServiceInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Service published clients.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `ip` - The `ip` parameter.
    /// * `port` - The `port` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<ClientPublisherInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Service subscribed clients.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `group_name` - The `group_name` parameter.
    /// * `service_name` - The `service_name` parameter.
    /// * `ip` - The `ip` parameter.
    /// * `port` - The `port` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<ClientSubscriberInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Mcp server list.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `mcp_name` - The `mcp_name` parameter.
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<McpServerBasicInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Mcp server search.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `mcp_name` - The `mcp_name` parameter.
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<McpServerBasicInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Mcp server detail.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `mcp_name` - The `mcp_name` parameter.
    /// * `mcp_id` - The `mcp_id` parameter.
    /// * `version` - The `version` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<McpServerDetailInfo>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Mcp server create.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `mcp_name` - The `mcp_name` parameter.
    /// * `server_spec` - The `server_spec` parameter.
    /// * `tool_spec` - The `tool_spec` parameter.
    /// * `endpoint_spec` - The `endpoint_spec` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<String>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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
    /// Mcp server update.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `mcp_name` - The `mcp_name` parameter.
    /// * `is_latest` - The `is_latest` parameter.
    /// * `server_spec` - The `server_spec` parameter.
    /// * `tool_spec` - The `tool_spec` parameter.
    /// * `endpoint_spec` - The `endpoint_spec` parameter.
    /// * `override_existing` - The `override_existing` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Mcp server delete.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `mcp_name` - The `mcp_name` parameter.
    /// * `mcp_id` - The `mcp_id` parameter.
    /// * `version` - The `version` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// List the version history of one MCP server.
    ///
    /// Wraps `GET /v3/admin/ai/mcp/versions`. `status` is forwarded verbatim and
    /// the server validates it against its five lifecycle states.
    pub async fn mcp_server_list_versions(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        status: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<McpServerVersionInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            mcp_name: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            status: Option<&'a str>,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<Page<McpServerVersionInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_MCP_VERSIONS,
                &Query {
                    namespace_id,
                    mcp_name,
                    status,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Fetch one exact MCP server version.
    ///
    /// Wraps `GET /v3/admin/ai/mcp/version`.
    pub async fn mcp_server_get_version(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            mcp_name: &'a str,
            version: &'a str,
        }

        let response: ApiResponse<McpServerVersionInfo> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_MCP_VERSION,
                &Query {
                    namespace_id,
                    mcp_name,
                    version,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Shared implementation of the six ADMIN MCP version transitions.
    ///
    /// All six endpoints take `namespaceId`, `mcpName` and `version` as query
    /// parameters and no body; `post_with_query` supplies the content type the
    /// actix handler needs to resolve its payload extractor.
    async fn mcp_server_transition(
        &self,
        path: &str,
        namespace_id: &str,
        mcp_name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            mcp_name: &'a str,
            version: &'a str,
        }

        let response: ApiResponse<McpServerVersionInfo> = self
            .http_client
            .post_with_query(
                path,
                &Query {
                    namespace_id,
                    mcp_name,
                    version,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Submit a draft version for review (`draft` -> `reviewing`).
    pub async fn mcp_server_submit_version(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionInfo> {
        self.mcp_server_transition(
            admin_api_path::AI_MCP_SUBMIT,
            namespace_id,
            mcp_name,
            version,
        )
        .await
    }

    /// Publish a reviewed version (`reviewing`/`reviewed` -> `online`).
    pub async fn mcp_server_publish_version(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionInfo> {
        self.mcp_server_transition(
            admin_api_path::AI_MCP_PUBLISH,
            namespace_id,
            mcp_name,
            version,
        )
        .await
    }

    /// Publish a version bypassing the review gate.
    pub async fn mcp_server_force_publish_version(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionInfo> {
        self.mcp_server_transition(
            admin_api_path::AI_MCP_FORCE_PUBLISH,
            namespace_id,
            mcp_name,
            version,
        )
        .await
    }

    /// Move a version back to draft so it can be edited again.
    pub async fn mcp_server_redraft_version(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionInfo> {
        self.mcp_server_transition(
            admin_api_path::AI_MCP_REDRAFT,
            namespace_id,
            mcp_name,
            version,
        )
        .await
    }

    /// Bring an offline version back online.
    pub async fn mcp_server_online_version(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionInfo> {
        self.mcp_server_transition(
            admin_api_path::AI_MCP_ONLINE,
            namespace_id,
            mcp_name,
            version,
        )
        .await
    }

    /// Take a version offline without deleting it.
    pub async fn mcp_server_offline_version(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionInfo> {
        self.mcp_server_transition(
            admin_api_path::AI_MCP_OFFLINE,
            namespace_id,
            mcp_name,
            version,
        )
        .await
    }

    // ============================================================================
    // AI Agent (A2A) APIs
    // ============================================================================

    /// Agent register.
    ///
    /// # Arguments
    /// * `agent_card` - The `agent_card` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `registration_type` - The `registration_type` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Agent get (legacy `/v3/admin/ai/a2a` facade).
    ///
    /// GA `getAgentCard` binds AND consumes a `version` query parameter, but
    /// the legacy 3-argument signature cannot express it. This method keeps
    /// that signature for backward compatibility and delegates to
    /// [`Self::agent_get_with_version`] with `version: None`; use the latter
    /// to actually send the GA `version` query.
    ///
    /// # Arguments
    /// * `agent_name` - The `agent_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `registration_type` - The `registration_type` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<AgentCardDetailInfo>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
    pub async fn agent_get(
        &self,
        agent_name: &str,
        namespace_id: &str,
        registration_type: &str,
    ) -> anyhow::Result<AgentCardDetailInfo> {
        self.agent_get_with_version(agent_name, namespace_id, registration_type, None)
            .await
    }

    /// Agent get with an optional GA `version` query parameter.
    ///
    /// Wraps `GET /v3/admin/ai/a2a`. When `version` is `Some`, the request
    /// carries `version=<..>` (GA `A2aAdminController` binds and consumes it);
    /// when `None` the field is omitted entirely through
    /// `skip_serializing_if`, preserving the legacy query shape.
    pub async fn agent_get_with_version(
        &self,
        agent_name: &str,
        namespace_id: &str,
        registration_type: &str,
        version: Option<&str>,
    ) -> anyhow::Result<AgentCardDetailInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            agent_name: &'a str,
            namespace_id: &'a str,
            registration_type: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            version: Option<&'a str>,
        }

        let response: ApiResponse<AgentCardDetailInfo> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENT,
                &Query {
                    agent_name,
                    namespace_id,
                    registration_type,
                    version,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Agent update.
    ///
    /// # Arguments
    /// * `agent_card` - The `agent_card` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `set_as_latest` - The `set_as_latest` parameter.
    /// * `registration_type` - The `registration_type` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Agent delete.
    ///
    /// # Arguments
    /// * `agent_name` - The `agent_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `version` - The `version` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<bool>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Agent list versions.
    ///
    /// # Arguments
    /// * `agent_name` - The `agent_name` parameter.
    /// * `namespace_id` - The `namespace_id` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Vec<AgentVersionDetail>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Agent list.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `agent_name` - The `agent_name` parameter.
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<AgentCardVersionInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    /// Agent search.
    ///
    /// # Arguments
    /// * `namespace_id` - The `namespace_id` parameter.
    /// * `agent_name_pattern` - The `agent_name_pattern` parameter.
    /// * `page_no` - The `page_no` parameter.
    /// * `page_size` - The `page_size` parameter.
    ///
    /// # Returns
    /// The `anyhow::Result<Page<AgentCardVersionInfo>>` result.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP request fails.
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

    // ============================================================================
    // Protocol-neutral Agent APIs (Nacos 3.3.0 `AgentMaintainerService`)
    // ============================================================================

    /// Fetch an agent, optionally pinned to one exact version.
    ///
    /// Wraps `GET /v3/admin/ai/agents`. The optional `version` is the
    /// `agent_get(version)` capability that the legacy `/v3/admin/ai/a2a`
    /// facade cannot express; upstream GA exposes the version separately as
    /// `getAgentVersion`, which maps to [`Self::agents_get_version`].
    pub async fn agents_get(
        &self,
        namespace_id: &str,
        agent_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<AgentVersionInfo> {
        let response: ApiResponse<AgentVersionInfo> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENTS,
                &AgentIdentityQuery {
                    namespace_id,
                    agent_name,
                    version,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// List agents in the protocol-neutral GA agent view.
    ///
    /// Wraps `GET /v3/admin/ai/agents/list`. Blank filters are omitted, like
    /// upstream `putIfNotBlank`. `bizTag`, `scope`, `owner` and `orderBy` are
    /// forwarded for upstream parity; batata's admin handler ignores them.
    pub async fn agents_list(
        &self,
        namespace_id: &str,
        agent_name: &str,
        biz_tag: &str,
        scope: &str,
        owner: &str,
        order_by: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentVersionInfo>> {
        let response: ApiResponse<Page<AgentVersionInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENTS_LIST,
                &AgentListQuery {
                    namespace_id,
                    agent_name: non_empty(agent_name),
                    biz_tag: non_empty(biz_tag),
                    scope: non_empty(scope),
                    owner: non_empty(owner),
                    order_by: non_empty(order_by),
                    status: None,
                    page_no: Some(page_no),
                    page_size: Some(page_size),
                },
            )
            .await?;
        Ok(response.data)
    }

    /// List the versions of one agent.
    ///
    /// Wraps `GET /v3/admin/ai/agents/versions`. Upstream GA returns a
    /// `Page<AgentVersionSummary>` and batata's admin handler (batch 12) now
    /// returns the same paged shape, so the client decodes `Page` rather than
    /// the old plain array. `status`/`pageNo`/`pageSize` are forwarded as GA
    /// query parameters.
    pub async fn agents_list_versions(
        &self,
        namespace_id: &str,
        agent_name: &str,
        status: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentVersionInfo>> {
        let response: ApiResponse<Page<AgentVersionInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENTS_VERSIONS,
                &AgentListQuery {
                    namespace_id,
                    agent_name: non_empty(agent_name),
                    biz_tag: None,
                    scope: None,
                    owner: None,
                    order_by: None,
                    status: non_empty(status),
                    page_no: Some(page_no),
                    page_size: Some(page_size),
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Fetch one exact agent version definition.
    ///
    /// Wraps `GET /v3/admin/ai/agents/version`.
    pub async fn agents_get_version(
        &self,
        namespace_id: &str,
        agent_name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionInfo> {
        let response: ApiResponse<AgentVersionInfo> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENTS_VERSION,
                &AgentVersionQuery {
                    namespace_id,
                    agent_name,
                    version,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Create a draft version using the GA urlencoded form wire.
    ///
    /// Wraps `POST /v3/admin/ai/agents/draft` (GA `createDraft`). GA sends
    /// every field as an urlencoded form value, with nested objects collapsed
    /// into a single JSON string; batata's `resolve_agent_write` accepts this
    /// same flat form. Use [`Self::agents_create_draft_json`] for the legacy
    /// batata JSON-body channel.
    pub async fn agents_create_draft(
        &self,
        form: &AgentDraftCreateForm,
    ) -> anyhow::Result<AgentVersionInfo> {
        let response: ApiResponse<AgentVersionInfo> = self
            .http_client
            .post_form(admin_api_path::AI_AGENTS_DRAFT, form)
            .await?;
        Ok(response.data)
    }

    /// Legacy batata JSON-body variant of [`Self::agents_create_draft`].
    ///
    /// Keeps the previous `post_json_with_query` wire (`AgentCard` body plus
    /// `namespaceId`/`overwrite` query) so the batata dual-channel admin
    /// handler and existing callers keep working.
    pub async fn agents_create_draft_json(
        &self,
        namespace_id: &str,
        card: &AgentCard,
        overwrite: bool,
    ) -> anyhow::Result<AgentVersionInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct DraftQuery<'a> {
            namespace_id: &'a str,
            overwrite: bool,
        }

        let response: ApiResponse<AgentVersionInfo> = self
            .http_client
            .post_json_with_query(
                admin_api_path::AI_AGENTS_DRAFT,
                &DraftQuery {
                    namespace_id,
                    overwrite,
                },
                card,
            )
            .await?;
        Ok(response.data)
    }

    /// Replace the draft version using the GA urlencoded form wire.
    ///
    /// Wraps `PUT /v3/admin/ai/agents/draft` (GA `updateDraft`). GA sends
    /// identity plus `callInterfaces` (JSON string) and `changeDescription`
    /// as form values. Use [`Self::agents_update_draft_json`] for the legacy
    /// batata JSON-body channel.
    pub async fn agents_update_draft(
        &self,
        form: &AgentDraftUpdateForm,
    ) -> anyhow::Result<AgentVersionInfo> {
        let response: ApiResponse<AgentVersionInfo> = self
            .http_client
            .put_form(admin_api_path::AI_AGENTS_DRAFT, form)
            .await?;
        Ok(response.data)
    }

    /// Legacy batata JSON-body variant of [`Self::agents_update_draft`].
    pub async fn agents_update_draft_json(
        &self,
        namespace_id: &str,
        card: &AgentCard,
    ) -> anyhow::Result<AgentVersionInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct DraftQuery<'a> {
            namespace_id: &'a str,
        }

        let response: ApiResponse<AgentVersionInfo> = self
            .http_client
            .put_json_with_query(
                admin_api_path::AI_AGENTS_DRAFT,
                &DraftQuery { namespace_id },
                card,
            )
            .await?;
        Ok(response.data)
    }

    /// Delete a draft version.
    ///
    /// Wraps `DELETE /v3/admin/ai/agents/draft`.
    pub async fn agents_delete_draft(
        &self,
        namespace_id: &str,
        agent_name: &str,
        version: &str,
    ) -> anyhow::Result<bool> {
        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .delete_with_query(
                admin_api_path::AI_AGENTS_DRAFT,
                &AgentVersionQuery {
                    namespace_id,
                    agent_name,
                    version,
                },
            )
            .await?;
        Ok(response.code == 0 || response.code == 200)
    }

    /// Shared implementation of the six ADMIN agent version transitions.
    ///
    /// All six endpoints take `namespaceId`, `agentName` and `version` as
    /// query parameters and no body, which is identical on upstream GA and on
    /// the batata admin handlers.
    async fn agent_version_transition(
        &self,
        path: &str,
        namespace_id: &str,
        agent_name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionInfo> {
        let response: ApiResponse<AgentVersionInfo> = self
            .http_client
            .post_with_query(
                path,
                &AgentVersionQuery {
                    namespace_id,
                    agent_name,
                    version,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Submit a draft version for review.
    pub async fn agents_submit_version(
        &self,
        namespace_id: &str,
        agent_name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionInfo> {
        self.agent_version_transition(
            admin_api_path::AI_AGENTS_SUBMIT,
            namespace_id,
            agent_name,
            version,
        )
        .await
    }

    /// Publish a reviewed version.
    pub async fn agents_publish_version(
        &self,
        namespace_id: &str,
        agent_name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionInfo> {
        self.agent_version_transition(
            admin_api_path::AI_AGENTS_PUBLISH,
            namespace_id,
            agent_name,
            version,
        )
        .await
    }

    /// Publish a version bypassing the review gate.
    pub async fn agents_force_publish_version(
        &self,
        namespace_id: &str,
        agent_name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionInfo> {
        self.agent_version_transition(
            admin_api_path::AI_AGENTS_FORCE_PUBLISH,
            namespace_id,
            agent_name,
            version,
        )
        .await
    }

    /// Move a version back to draft so it can be edited again.
    pub async fn agents_redraft_version(
        &self,
        namespace_id: &str,
        agent_name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionInfo> {
        self.agent_version_transition(
            admin_api_path::AI_AGENTS_REDRAFT,
            namespace_id,
            agent_name,
            version,
        )
        .await
    }

    /// Bring an offline version back online.
    pub async fn agents_online_version(
        &self,
        namespace_id: &str,
        agent_name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionInfo> {
        self.agent_version_transition(
            admin_api_path::AI_AGENTS_ONLINE,
            namespace_id,
            agent_name,
            version,
        )
        .await
    }

    /// Take an online version offline without deleting it.
    pub async fn agents_offline_version(
        &self,
        namespace_id: &str,
        agent_name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionInfo> {
        self.agent_version_transition(
            admin_api_path::AI_AGENTS_OFFLINE,
            namespace_id,
            agent_name,
            version,
        )
        .await
    }

    /// Replace the custom version labels using the GA urlencoded form wire.
    ///
    /// Wraps `PUT /v3/admin/ai/agents/labels` (GA `updateLabels`). GA sends
    /// `labels` as a SINGLE JSON string form value (`putJsonIfNotNull`), so
    /// the map is serialized to one JSON string here. Returns GA
    /// `AgentSummary`; use [`Self::agents_update_labels_json`] for the legacy
    /// batata JSON-body channel (which returns the labels map).
    pub async fn agents_update_labels(
        &self,
        namespace_id: &str,
        agent_name: &str,
        labels: &HashMap<String, String>,
    ) -> anyhow::Result<AgentSummary> {
        let labels_json = serde_json::to_string(labels)?;
        let form = AgentLabelsUpdateForm {
            namespace_id: namespace_id.to_string(),
            agent_name: agent_name.to_string(),
            labels: Some(labels_json),
        };
        let response: ApiResponse<AgentSummary> = self
            .http_client
            .put_form(admin_api_path::AI_AGENTS_LABELS, &form)
            .await?;
        Ok(response.data)
    }

    /// Legacy batata JSON-body variant of [`Self::agents_update_labels`].
    ///
    /// Keeps the `{"labels": {...}}` JSON body plus identity query and returns
    /// the batata labels map unchanged.
    pub async fn agents_update_labels_json(
        &self,
        namespace_id: &str,
        agent_name: &str,
        labels: &HashMap<String, String>,
    ) -> anyhow::Result<HashMap<String, String>> {
        #[derive(Serialize)]
        struct LabelsBody<'a> {
            labels: &'a HashMap<String, String>,
        }

        let response: ApiResponse<HashMap<String, String>> = self
            .http_client
            .put_json_with_query(
                admin_api_path::AI_AGENTS_LABELS,
                &AgentIdentityQuery {
                    namespace_id,
                    agent_name,
                    version: None,
                },
                &LabelsBody { labels },
            )
            .await?;
        Ok(response.data)
    }

    /// Change the visibility scope of an agent.
    ///
    /// Wraps `PUT /v3/admin/ai/agents/scope`. The handler reads every field
    /// from the query; the empty JSON body keeps the request well-formed.
    pub async fn agents_update_scope(
        &self,
        namespace_id: &str,
        agent_name: &str,
        scope: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct ScopeQuery<'a> {
            namespace_id: &'a str,
            agent_name: &'a str,
            scope: &'a str,
        }

        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .put_json_with_query(
                admin_api_path::AI_AGENTS_SCOPE,
                &ScopeQuery {
                    namespace_id,
                    agent_name,
                    scope,
                },
                &serde_json::json!({}),
            )
            .await?;
        Ok(response.code == 0 || response.code == 200)
    }

    /// Update the agent's current metadata using the GA urlencoded form wire.
    ///
    /// Wraps `PUT /v3/admin/ai/agents` (GA `updateAgent`). The form carries
    /// `namespaceId`/`agentName` plus `displayName`/`description`/`iconUrl`/
    /// `provider`/`tags`/`extensions`/`status` — and deliberately NO `version`.
    /// Returns GA `AgentSummary`.
    pub async fn agents_update_agent(
        &self,
        form: &AgentUpdateForm,
    ) -> anyhow::Result<AgentSummary> {
        let response: ApiResponse<AgentSummary> = self
            .http_client
            .put_form(admin_api_path::AI_AGENTS, form)
            .await?;
        Ok(response.data)
    }

    /// Delete an agent (GA `deleteAgent`).
    ///
    /// Wraps `DELETE /v3/admin/ai/agents` with ONLY `namespaceId` and
    /// `agentName` in the query (no `version`). GA returns void; success is
    /// reported through the envelope code.
    pub async fn agents_delete_agent(
        &self,
        namespace_id: &str,
        agent_name: &str,
    ) -> anyhow::Result<bool> {
        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .delete_with_query(
                admin_api_path::AI_AGENTS,
                &AgentIdentityQuery {
                    namespace_id,
                    agent_name,
                    version: None,
                },
            )
            .await?;
        Ok(response.code == 0 || response.code == 200)
    }

    /// Fetch the runtime endpoint snapshot for an agent version.
    ///
    /// Wraps `GET /v3/admin/ai/agents/runtime-endpoints` (GA
    /// `getRuntimeEndpoints`). `protocol` and `version` are optional query
    /// fields. The response is decoded into a tolerant DTO because batata's
    /// snapshot shape is known to differ from GA's (`F-NAC-AISRV-001`).
    pub async fn agents_get_runtime_endpoints(
        &self,
        namespace_id: &str,
        agent_name: &str,
        protocol: Option<&str>,
        version: Option<&str>,
    ) -> anyhow::Result<RuntimeEndpointSnapshot> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            agent_name: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            protocol: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            version: Option<&'a str>,
        }

        let response: ApiResponse<RuntimeEndpointSnapshot> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENTS_RUNTIME_ENDPOINTS,
                &Query {
                    namespace_id,
                    agent_name,
                    protocol,
                    version,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // AI resource facades (Nacos `AiMaintainerService` delegates)
    // ============================================================================
    //
    // The Java SDK exposes these capabilities through `skill()`, `prompt()`,
    // `agentSpec()` and `pipeline()` sub-service objects. Rust uses flat
    // `skill_*`/`prompt_*`/`agentspec_*`/`pipeline_*` methods instead. Read and
    // DELETE endpoints carry identity in the query string; POST/PUT endpoints
    // carry it in an urlencoded form body; ZIP endpoints use a `file` multipart
    // part plus query parameters. The protocol-neutral `agents_*` writes above
    // now use the same GA form wire; the explicit `*_json` aliases are the only
    // remaining JSON bodies, kept for batata's dual-channel admin handlers.

    /// Send an AI resource write that reports success through its result code.
    ///
    /// GA checks `ErrorCode.SUCCESS` on the envelope, while batata uses
    /// `code: 0`. Decoding `data` as a JSON value keeps the helper usable for
    /// endpoints that answer with `true`, `"ok"` or an object.
    async fn ai_write_ok<F: Serialize + ?Sized>(
        &self,
        verb: AiWriteVerb,
        path: &str,
        form: &F,
    ) -> anyhow::Result<bool> {
        let response: ApiResponse<serde_json::Value> = match verb {
            AiWriteVerb::Post => self.http_client.post_form(path, form).await?,
            AiWriteVerb::Put => self.http_client.put_form(path, form).await?,
            AiWriteVerb::Delete => self.http_client.delete_with_query(path, form).await?,
        };
        Ok(response.code == 0 || response.code == 200)
    }

    /// Send a POST that returns a version string (draft creation or submission).
    async fn ai_write_version<F: Serialize + ?Sized>(
        &self,
        path: &str,
        form: &F,
    ) -> anyhow::Result<String> {
        let response: ApiResponse<String> = self.http_client.post_form(path, form).await?;
        Ok(response.data)
    }

    /// Get skill metadata and governance information.
    ///
    /// Wraps `GET /v3/admin/ai/skills`, the batata equivalent of GA
    /// `SkillMaintainerService.getSkillMeta`. GA `SkillMeta` fields are typed;
    /// unmodelled fields stay reachable through `extra`.
    pub async fn skill_get_meta(
        &self,
        namespace_id: &str,
        skill_name: &str,
    ) -> anyhow::Result<SkillMeta> {
        let response: ApiResponse<SkillMeta> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_SKILLS,
                &SkillIdentityQuery {
                    namespace_id,
                    skill_name,
                    version: None,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Get the detail of one published skill version.
    pub async fn skill_get_version_detail(
        &self,
        namespace_id: &str,
        skill_name: &str,
        version: &str,
    ) -> anyhow::Result<Skill> {
        let response: ApiResponse<Skill> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_SKILLS_VERSION,
                &SkillIdentityQuery {
                    namespace_id,
                    skill_name,
                    version: Some(version),
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Download one skill version as a raw ZIP archive.
    pub async fn skill_download_version(
        &self,
        namespace_id: &str,
        skill_name: &str,
        version: &str,
    ) -> anyhow::Result<Vec<u8>> {
        self.http_client
            .get_bytes_with_query(
                admin_api_path::AI_SKILLS_VERSION_DOWNLOAD,
                &SkillIdentityQuery {
                    namespace_id,
                    skill_name,
                    version: Some(version),
                },
            )
            .await
    }

    /// List skills with GA's full filter set.
    ///
    /// `owner`, `scope` and `biz_tag` are forwarded for GA parity; batata's
    /// `SkillListForm` currently ignores them.
    pub async fn skill_list(
        &self,
        namespace_id: &str,
        skill_name: Option<&str>,
        search: Option<&str>,
        order_by: Option<&str>,
        owner: Option<&str>,
        scope: Option<&str>,
        biz_tag: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<SkillSummary>> {
        let response: ApiResponse<Page<SkillSummary>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_SKILLS_LIST,
                &SkillListQuery {
                    namespace_id,
                    skill_name,
                    search,
                    order_by,
                    owner,
                    scope,
                    biz_tag,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Delete a skill and all of its versions.
    pub async fn skill_delete(&self, namespace_id: &str, skill_name: &str) -> anyhow::Result<bool> {
        let response: ApiResponse<OkOrBool> = self
            .http_client
            .delete_with_query(
                admin_api_path::AI_SKILLS,
                &SkillIdentityQuery {
                    namespace_id,
                    skill_name,
                    version: None,
                },
            )
            .await?;
        Ok(response.data.0)
    }

    /// Upload a skill ZIP archive and return the stored skill name.
    ///
    /// Mirrors the GA `POST /v3/admin/ai/skills/upload` contract: `target_version`,
    /// `upload_action`, `auto_publish_if_new` and `commit_msg` are sent as query
    /// parameters alongside the multipart `file` body.
    pub async fn skill_upload_from_zip(
        &self,
        namespace_id: &str,
        zip_bytes: &[u8],
        overwrite: bool,
        target_version: Option<&str>,
        upload_action: Option<&str>,
        auto_publish_if_new: bool,
        commit_msg: Option<&str>,
    ) -> anyhow::Result<String> {
        let response: ApiResponse<String> = self
            .http_client
            .post_multipart_with_query(
                admin_api_path::AI_SKILLS_UPLOAD,
                &AiUploadQuery {
                    namespace_id,
                    overwrite,
                    target_version,
                    upload_action,
                    auto_publish_if_new,
                    commit_msg,
                },
                ai_zip_form(zip_bytes),
            )
            .await?;
        Ok(response.data)
    }

    /// Report what a skill ZIP upload would do, without applying it.
    pub async fn skill_precheck_upload(
        &self,
        namespace_id: &str,
        zip_bytes: &[u8],
    ) -> anyhow::Result<Vec<SkillUploadPrecheckResult>> {
        let response: ApiResponse<Vec<SkillUploadPrecheckResult>> = self
            .http_client
            .post_multipart_with_query(
                admin_api_path::AI_SKILLS_UPLOAD_PRECHECK,
                &AiUploadQuery {
                    namespace_id,
                    overwrite: false,
                    target_version: None,
                    upload_action: None,
                    auto_publish_if_new: false,
                    commit_msg: None,
                },
                ai_zip_form(zip_bytes),
            )
            .await?;
        Ok(response.data)
    }

    /// Upload every skill contained in a ZIP archive.
    ///
    /// GA `/upload/batch` only binds `namespaceId` / `overwrite` / `file`, so the
    /// optional upload-action parameters are omitted from the wire here.
    pub async fn skill_batch_upload(
        &self,
        namespace_id: &str,
        zip_bytes: &[u8],
        overwrite: bool,
    ) -> anyhow::Result<BatchUploadResult> {
        let response: ApiResponse<BatchUploadResult> = self
            .http_client
            .post_multipart_with_query(
                admin_api_path::AI_SKILLS_UPLOAD_BATCH,
                &AiUploadQuery {
                    namespace_id,
                    overwrite,
                    target_version: None,
                    upload_action: None,
                    auto_publish_if_new: false,
                    commit_msg: None,
                },
                ai_zip_form(zip_bytes),
            )
            .await?;
        Ok(response.data)
    }

    /// List prompts with pagination and optional filters.
    pub async fn prompt_list(
        &self,
        namespace_id: &str,
        prompt_key: Option<&str>,
        search: Option<&str>,
        biz_tags: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PromptMetaSummary>> {
        let response: ApiResponse<Page<PromptMetaSummary>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_PROMPT_LIST,
                &PromptListQuery {
                    namespace_id,
                    prompt_key,
                    search,
                    biz_tags,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// List the version history of a prompt.
    pub async fn prompt_list_versions(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PromptVersionSummary>> {
        let response: ApiResponse<Page<PromptVersionSummary>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_PROMPT_VERSIONS,
                &PromptHistoryQuery {
                    namespace_id,
                    prompt_key,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Get prompt metadata without version content.
    pub async fn prompt_get_meta(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> anyhow::Result<PromptMetaInfo> {
        self.prompt_get::<PromptMetaInfo>(
            admin_api_path::AI_PROMPT_METADATA,
            namespace_id,
            prompt_key,
            None,
            None,
            None,
        )
        .await
    }

    /// Get prompt governance detail, including all version summaries.
    pub async fn prompt_get_governance_detail(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> anyhow::Result<PromptMetaInfo> {
        self.prompt_get::<PromptMetaInfo>(
            admin_api_path::AI_PROMPT_GOVERNANCE,
            namespace_id,
            prompt_key,
            None,
            None,
            None,
        )
        .await
    }

    /// Get one prompt version by its version string.
    pub async fn prompt_get_version_detail(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<PromptVersionInfo> {
        self.prompt_get::<PromptVersionInfo>(
            admin_api_path::AI_PROMPT_VERSION,
            namespace_id,
            prompt_key,
            Some(version),
            None,
            None,
        )
        .await
    }

    /// Query prompt detail, resolving an optional version, label or MD5.
    pub async fn prompt_query_detail(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: Option<&str>,
        label: Option<&str>,
        md5: Option<&str>,
    ) -> anyhow::Result<PromptVersionInfo> {
        self.prompt_get::<PromptVersionInfo>(
            admin_api_path::AI_PROMPT_DETAIL,
            namespace_id,
            prompt_key,
            version,
            label,
            md5,
        )
        .await
    }

    /// Shared implementation of the prompt read endpoints.
    async fn prompt_get<T: DeserializeOwned>(
        &self,
        path: &str,
        namespace_id: &str,
        prompt_key: &str,
        version: Option<&str>,
        label: Option<&str>,
        md5: Option<&str>,
    ) -> anyhow::Result<T> {
        let response: ApiResponse<T> = self
            .http_client
            .get_with_query(
                path,
                &PromptIdentityQuery {
                    namespace_id,
                    prompt_key,
                    version,
                    label,
                    md5,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Download one prompt version as raw bytes.
    pub async fn prompt_download_version(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<Vec<u8>> {
        self.http_client
            .get_bytes_with_query(
                admin_api_path::AI_PROMPT_VERSION_DOWNLOAD,
                &PromptIdentityQuery {
                    namespace_id,
                    prompt_key,
                    version: Some(version),
                    label: None,
                    md5: None,
                },
            )
            .await
    }

    /// Delete a prompt and all of its versions.
    pub async fn prompt_delete(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> anyhow::Result<bool> {
        let response: ApiResponse<OkOrBool> = self
            .http_client
            .delete_with_query(
                admin_api_path::AI_PROMPT,
                &PromptIdentityQuery {
                    namespace_id,
                    prompt_key,
                    version: None,
                    label: None,
                    md5: None,
                },
            )
            .await?;
        Ok(response.data.0)
    }

    /// Get agentspec detail, including governance and version information.
    pub async fn agentspec_get_detail(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
    ) -> anyhow::Result<AgentSpecMeta> {
        self.agentspec_get::<AgentSpecMeta>(
            admin_api_path::AI_AGENTSPECS,
            namespace_id,
            agent_spec_name,
            None,
        )
        .await
    }

    /// Get the full detail of one agentspec version.
    pub async fn agentspec_get_version_detail(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        version: &str,
    ) -> anyhow::Result<AgentSpec> {
        self.agentspec_get::<AgentSpec>(
            admin_api_path::AI_AGENTSPECS_VERSION,
            namespace_id,
            agent_spec_name,
            Some(version),
        )
        .await
    }

    /// Get one agentspec version without loading resource file contents.
    pub async fn agentspec_get_version_meta(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        version: &str,
    ) -> anyhow::Result<AgentSpec> {
        self.agentspec_get::<AgentSpec>(
            admin_api_path::AI_AGENTSPECS_VERSION_META,
            namespace_id,
            agent_spec_name,
            Some(version),
        )
        .await
    }

    /// Shared implementation of the agentspec read endpoints.
    async fn agentspec_get<T: DeserializeOwned>(
        &self,
        path: &str,
        namespace_id: &str,
        agent_spec_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<T> {
        let response: ApiResponse<T> = self
            .http_client
            .get_with_query(
                path,
                &AgentSpecIdentityQuery {
                    namespace_id,
                    agent_spec_name,
                    version,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// List agentspecs with pagination and optional filters.
    pub async fn agentspec_list(
        &self,
        namespace_id: &str,
        agent_spec_name: Option<&str>,
        search: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentSpecBasicInfo>> {
        let response: ApiResponse<Page<AgentSpecBasicInfo>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENTSPECS_LIST,
                &AgentSpecListQuery {
                    namespace_id,
                    agent_spec_name,
                    search,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Download one agentspec version as a raw ZIP archive.
    pub async fn agentspec_download_version(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        version: &str,
    ) -> anyhow::Result<Vec<u8>> {
        self.http_client
            .get_bytes_with_query(
                admin_api_path::AI_AGENTSPECS_VERSION_DOWNLOAD,
                &AgentSpecIdentityQuery {
                    namespace_id,
                    agent_spec_name,
                    version: Some(version),
                },
            )
            .await
    }

    /// Upload an agentspec ZIP archive and return the stored spec name.
    pub async fn agentspec_upload_from_zip(
        &self,
        namespace_id: &str,
        zip_bytes: &[u8],
        overwrite: bool,
    ) -> anyhow::Result<String> {
        let response: ApiResponse<String> = self
            .http_client
            .post_multipart_with_query(
                admin_api_path::AI_AGENTSPECS_UPLOAD,
                &AiUploadQuery {
                    namespace_id,
                    overwrite,
                    target_version: None,
                    upload_action: None,
                    auto_publish_if_new: false,
                    commit_msg: None,
                },
                ai_zip_form(zip_bytes),
            )
            .await?;
        Ok(response.data)
    }

    /// Delete an agentspec and all of its versions.
    pub async fn agentspec_delete(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
    ) -> anyhow::Result<bool> {
        let response: ApiResponse<OkOrBool> = self
            .http_client
            .delete_with_query(
                admin_api_path::AI_AGENTSPECS,
                &AgentSpecIdentityQuery {
                    namespace_id,
                    agent_spec_name,
                    version: None,
                },
            )
            .await?;
        Ok(response.data.0)
    }

    /// Get one pipeline execution by ID.
    pub async fn pipeline_get_detail(
        &self,
        pipeline_id: &str,
    ) -> anyhow::Result<PipelineExecution> {
        let response: ApiResponse<PipelineExecution> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_PIPELINES_DETAIL,
                &PipelineDetailQuery { pipeline_id },
            )
            .await?;
        Ok(response.data)
    }

    /// List pipeline executions for a resource type with optional filters.
    pub async fn pipeline_list_executions(
        &self,
        resource_type: &str,
        resource_name: Option<&str>,
        namespace_id: Option<&str>,
        version: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PipelineExecution>> {
        let response: ApiResponse<Page<PipelineExecution>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_PIPELINES_LIST,
                &PipelineListQuery {
                    resource_type,
                    resource_name,
                    namespace_id,
                    version,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============================================================================
    // AI resource write facade (Nacos `AiMaintainerService` delegates, batch 7)
    // ============================================================================

    /// Create a new skill draft or fork an existing version.
    ///
    /// Wraps GA `SkillMaintainerService.createDraft` (`POST
    /// /v3/admin/ai/skills/draft`). `skill_card` is required for a brand-new
    /// skill and ignored when `based_on_version` is set.
    pub async fn skill_create_draft(
        &self,
        namespace_id: &str,
        skill_name: Option<&str>,
        based_on_version: Option<&str>,
        target_version: Option<&str>,
        skill_card: Option<&str>,
        commit_msg: Option<&str>,
    ) -> anyhow::Result<String> {
        self.ai_write_version(
            admin_api_path::AI_SKILLS_DRAFT,
            &SkillWriteForm {
                namespace_id,
                skill_name,
                based_on_version,
                target_version,
                skill_card,
                commit_msg,
                ..Default::default()
            },
        )
        .await
    }

    /// Replace the current skill draft content.
    pub async fn skill_update_draft(
        &self,
        namespace_id: &str,
        skill_name: Option<&str>,
        version: Option<&str>,
        skill_card: &str,
        set_as_latest: Option<bool>,
        commit_msg: Option<&str>,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_SKILLS_DRAFT,
            &SkillWriteForm {
                namespace_id,
                skill_name,
                version,
                skill_card: Some(skill_card),
                set_as_latest,
                commit_msg,
                ..Default::default()
            },
        )
        .await
    }

    /// Discard the current skill draft.
    pub async fn skill_delete_draft(
        &self,
        namespace_id: &str,
        skill_name: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Delete,
            admin_api_path::AI_SKILLS_DRAFT,
            &SkillWriteForm {
                namespace_id,
                skill_name: Some(skill_name),
                ..Default::default()
            },
        )
        .await
    }

    /// Submit a skill version for pipeline review; returns the submitted version.
    pub async fn skill_submit(
        &self,
        namespace_id: &str,
        skill_name: &str,
        version: &str,
    ) -> anyhow::Result<String> {
        self.ai_write_version(
            admin_api_path::AI_SKILLS_SUBMIT,
            &SkillWriteForm {
                namespace_id,
                skill_name: Some(skill_name),
                version: Some(version),
                ..Default::default()
            },
        )
        .await
    }

    /// Publish an approved skill version.
    pub async fn skill_publish(
        &self,
        namespace_id: &str,
        skill_name: &str,
        version: &str,
        update_latest_label: Option<bool>,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Post,
            admin_api_path::AI_SKILLS_PUBLISH,
            &SkillWriteForm {
                namespace_id,
                skill_name: Some(skill_name),
                version: Some(version),
                update_latest_label,
                ..Default::default()
            },
        )
        .await
    }

    /// Publish a skill version without the pipeline review gate.
    pub async fn skill_force_publish(
        &self,
        namespace_id: &str,
        skill_name: &str,
        version: &str,
        update_latest_label: Option<bool>,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Post,
            admin_api_path::AI_SKILLS_FORCE_PUBLISH,
            &SkillWriteForm {
                namespace_id,
                skill_name: Some(skill_name),
                version: Some(version),
                update_latest_label,
                ..Default::default()
            },
        )
        .await
    }

    /// Move a reviewed skill version back to draft.
    pub async fn skill_redraft(
        &self,
        namespace_id: &str,
        skill_name: &str,
        version: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Post,
            admin_api_path::AI_SKILLS_REDRAFT,
            &SkillWriteForm {
                namespace_id,
                skill_name: Some(skill_name),
                version: Some(version),
                ..Default::default()
            },
        )
        .await
    }

    /// Replace the skill's runtime label-to-version mapping (JSON string).
    pub async fn skill_update_labels(
        &self,
        namespace_id: &str,
        skill_name: &str,
        labels: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_SKILLS_LABELS,
            &SkillWriteForm {
                namespace_id,
                skill_name: Some(skill_name),
                labels: Some(labels),
                ..Default::default()
            },
        )
        .await
    }

    /// Replace the skill's business tags (JSON string).
    pub async fn skill_update_biz_tags(
        &self,
        namespace_id: &str,
        skill_name: &str,
        biz_tags: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_SKILLS_BIZ_TAGS,
            &SkillWriteForm {
                namespace_id,
                skill_name: Some(skill_name),
                biz_tags: Some(biz_tags),
                ..Default::default()
            },
        )
        .await
    }

    /// Enable or disable a skill (`scope = "skill"`) or one of its versions.
    pub async fn skill_change_online_status(
        &self,
        namespace_id: &str,
        skill_name: &str,
        scope: Option<&str>,
        version: Option<&str>,
        online: bool,
    ) -> anyhow::Result<bool> {
        let path = if online {
            admin_api_path::AI_SKILLS_ONLINE
        } else {
            admin_api_path::AI_SKILLS_OFFLINE
        };
        self.ai_write_ok(
            AiWriteVerb::Post,
            path,
            &SkillWriteForm {
                namespace_id,
                skill_name: Some(skill_name),
                scope,
                version,
                ..Default::default()
            },
        )
        .await
    }

    /// Change the skill's visibility scope (`PUBLIC` / `PRIVATE`).
    pub async fn skill_update_scope(
        &self,
        namespace_id: &str,
        skill_name: &str,
        scope: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_SKILLS_SCOPE,
            &SkillWriteForm {
                namespace_id,
                skill_name: Some(skill_name),
                scope: Some(scope),
                ..Default::default()
            },
        )
        .await
    }

    /// Create a prompt draft, optionally forking an existing version.
    ///
    /// GA calls the first version `targetVersion`; batata's
    /// `PromptDraftCreateForm` calls it `version`. Both are sent.
    pub async fn prompt_create_draft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        target_version: Option<&str>,
        based_on_version: Option<&str>,
        template: &str,
        variables: Option<&str>,
        commit_msg: Option<&str>,
        description: Option<&str>,
        biz_tags: Option<&str>,
    ) -> anyhow::Result<String> {
        self.ai_write_version(
            admin_api_path::AI_PROMPT_DRAFT,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                version: target_version,
                target_version,
                based_on_version,
                template: Some(template),
                variables,
                commit_msg,
                description,
                biz_tags,
                ..Default::default()
            },
        )
        .await
    }

    /// Replace the current prompt draft content.
    pub async fn prompt_update_draft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        template: &str,
        variables: Option<&str>,
        commit_msg: Option<&str>,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_PROMPT_DRAFT,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                template: Some(template),
                variables,
                commit_msg,
                ..Default::default()
            },
        )
        .await
    }

    /// Discard the current prompt draft.
    pub async fn prompt_delete_draft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Delete,
            admin_api_path::AI_PROMPT_DRAFT,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                ..Default::default()
            },
        )
        .await
    }

    /// Submit a prompt version for pipeline review.
    pub async fn prompt_submit(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<String> {
        self.ai_write_version(
            admin_api_path::AI_PROMPT_SUBMIT,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                version: Some(version),
                ..Default::default()
            },
        )
        .await
    }

    /// Publish a reviewed prompt version.
    pub async fn prompt_publish(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
        update_latest_label: Option<bool>,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Post,
            admin_api_path::AI_PROMPT_PUBLISH,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                version: Some(version),
                update_latest_label,
                ..Default::default()
            },
        )
        .await
    }

    /// Publish a prompt version without the pipeline review gate.
    pub async fn prompt_force_publish(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
        update_latest_label: Option<bool>,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Post,
            admin_api_path::AI_PROMPT_FORCE_PUBLISH,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                version: Some(version),
                update_latest_label,
                ..Default::default()
            },
        )
        .await
    }

    /// Move a reviewed prompt version back to draft.
    pub async fn prompt_redraft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Post,
            admin_api_path::AI_PROMPT_REDRAFT,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                version: Some(version),
                ..Default::default()
            },
        )
        .await
    }

    /// Enable or disable one prompt version.
    pub async fn prompt_change_online_status(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
        online: bool,
    ) -> anyhow::Result<bool> {
        let path = if online {
            admin_api_path::AI_PROMPT_ONLINE
        } else {
            admin_api_path::AI_PROMPT_OFFLINE
        };
        self.ai_write_ok(
            AiWriteVerb::Post,
            path,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                version: Some(version),
                ..Default::default()
            },
        )
        .await
    }

    /// Replace the prompt's label-to-version mapping (JSON string).
    pub async fn prompt_update_labels(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        labels: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_PROMPT_LABELS,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                labels: Some(labels),
                ..Default::default()
            },
        )
        .await
    }

    /// Update the prompt description.
    pub async fn prompt_update_description(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        description: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_PROMPT_DESCRIPTION,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                description: Some(description),
                ..Default::default()
            },
        )
        .await
    }

    /// Replace the prompt's business tags.
    pub async fn prompt_update_biz_tags(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        biz_tags: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_PROMPT_BIZ_TAGS,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                biz_tags: Some(biz_tags),
                ..Default::default()
            },
        )
        .await
    }

    /// Legacy single-label bind (deprecated upstream, still mounted).
    pub async fn prompt_bind_label(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        label: &str,
        version: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_PROMPT_LABEL,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                label: Some(label),
                version: Some(version),
                ..Default::default()
            },
        )
        .await
    }

    /// Legacy single-label unbind (deprecated upstream, still mounted).
    pub async fn prompt_unbind_label(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        label: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Delete,
            admin_api_path::AI_PROMPT_LABEL,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                label: Some(label),
                ..Default::default()
            },
        )
        .await
    }

    /// Legacy one-shot publish of a prompt version.
    pub async fn prompt_publish_prompt(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
        template: &str,
        commit_msg: Option<&str>,
        description: Option<&str>,
        biz_tags: Option<&str>,
        variables: Option<&str>,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Post,
            admin_api_path::AI_PROMPT,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                version: Some(version),
                template: Some(template),
                commit_msg,
                description,
                biz_tags,
                variables,
                ..Default::default()
            },
        )
        .await
    }

    /// Legacy prompt metadata update (deprecated upstream, still mounted).
    pub async fn prompt_update_metadata(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        description: Option<&str>,
        biz_tags: Option<&str>,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_PROMPT_METADATA,
            &PromptWriteForm {
                namespace_id,
                prompt_key: Some(prompt_key),
                description,
                biz_tags,
                ..Default::default()
            },
        )
        .await
    }

    /// List agentspecs with GA's admin filter set (`listAgentSpecAdminItems`).
    pub async fn agentspec_list_admin_items(
        &self,
        namespace_id: &str,
        agent_spec_name: Option<&str>,
        search: Option<&str>,
        order_by: Option<&str>,
        owner: Option<&str>,
        scope: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentSpecSummary>> {
        let response: ApiResponse<Page<AgentSpecSummary>> = self
            .http_client
            .get_with_query(
                admin_api_path::AI_AGENTSPECS_LIST,
                &AgentSpecAdminListQuery {
                    namespace_id,
                    agent_spec_name,
                    search,
                    order_by,
                    owner,
                    scope,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Create a new agentspec draft or fork an existing version.
    pub async fn agentspec_create_draft(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        based_on_version: Option<&str>,
        target_version: Option<&str>,
        agent_spec_card: Option<&str>,
        commit_msg: Option<&str>,
    ) -> anyhow::Result<String> {
        self.ai_write_version(
            admin_api_path::AI_AGENTSPECS_DRAFT,
            &AgentSpecWriteForm {
                namespace_id,
                agent_spec_name: Some(agent_spec_name),
                based_on_version,
                target_version,
                agent_spec_card,
                commit_msg,
                ..Default::default()
            },
        )
        .await
    }

    /// Replace the current agentspec draft content.
    pub async fn agentspec_update_draft(
        &self,
        namespace_id: &str,
        agent_spec_name: Option<&str>,
        version: Option<&str>,
        agent_spec_card: &str,
        set_as_latest: Option<bool>,
        commit_msg: Option<&str>,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_AGENTSPECS_DRAFT,
            &AgentSpecWriteForm {
                namespace_id,
                agent_spec_name,
                version,
                agent_spec_card: Some(agent_spec_card),
                set_as_latest,
                commit_msg,
                ..Default::default()
            },
        )
        .await
    }

    /// Discard the current agentspec draft.
    pub async fn agentspec_delete_draft(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Delete,
            admin_api_path::AI_AGENTSPECS_DRAFT,
            &AgentSpecWriteForm {
                namespace_id,
                agent_spec_name: Some(agent_spec_name),
                ..Default::default()
            },
        )
        .await
    }

    /// Submit an agentspec version for pipeline review.
    pub async fn agentspec_submit(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        version: &str,
    ) -> anyhow::Result<String> {
        self.ai_write_version(
            admin_api_path::AI_AGENTSPECS_SUBMIT,
            &AgentSpecWriteForm {
                namespace_id,
                agent_spec_name: Some(agent_spec_name),
                version: Some(version),
                ..Default::default()
            },
        )
        .await
    }

    /// Publish an approved agentspec version.
    pub async fn agentspec_publish(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        version: &str,
        update_latest_label: Option<bool>,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Post,
            admin_api_path::AI_AGENTSPECS_PUBLISH,
            &AgentSpecWriteForm {
                namespace_id,
                agent_spec_name: Some(agent_spec_name),
                version: Some(version),
                update_latest_label,
                ..Default::default()
            },
        )
        .await
    }

    /// Publish an agentspec version without the pipeline review gate.
    pub async fn agentspec_force_publish(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        version: &str,
        update_latest_label: Option<bool>,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Post,
            admin_api_path::AI_AGENTSPECS_FORCE_PUBLISH,
            &AgentSpecWriteForm {
                namespace_id,
                agent_spec_name: Some(agent_spec_name),
                version: Some(version),
                update_latest_label,
                ..Default::default()
            },
        )
        .await
    }

    /// Move a reviewed agentspec version back to draft.
    pub async fn agentspec_redraft(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        version: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Post,
            admin_api_path::AI_AGENTSPECS_REDRAFT,
            &AgentSpecWriteForm {
                namespace_id,
                agent_spec_name: Some(agent_spec_name),
                version: Some(version),
                ..Default::default()
            },
        )
        .await
    }

    /// Replace the agentspec's runtime labels (JSON string).
    pub async fn agentspec_update_labels(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        labels: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_AGENTSPECS_LABELS,
            &AgentSpecWriteForm {
                namespace_id,
                agent_spec_name: Some(agent_spec_name),
                labels: Some(labels),
                ..Default::default()
            },
        )
        .await
    }

    /// Replace the agentspec's business tags (JSON string).
    pub async fn agentspec_update_biz_tags(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        biz_tags: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_AGENTSPECS_BIZ_TAGS,
            &AgentSpecWriteForm {
                namespace_id,
                agent_spec_name: Some(agent_spec_name),
                biz_tags: Some(biz_tags),
                ..Default::default()
            },
        )
        .await
    }

    /// Enable or disable an agentspec or one of its versions.
    pub async fn agentspec_change_online_status(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        scope: Option<&str>,
        version: Option<&str>,
        online: bool,
    ) -> anyhow::Result<bool> {
        let path = if online {
            admin_api_path::AI_AGENTSPECS_ONLINE
        } else {
            admin_api_path::AI_AGENTSPECS_OFFLINE
        };
        self.ai_write_ok(
            AiWriteVerb::Post,
            path,
            &AgentSpecWriteForm {
                namespace_id,
                agent_spec_name: Some(agent_spec_name),
                scope,
                version,
                ..Default::default()
            },
        )
        .await
    }

    /// Change the agentspec's visibility scope.
    pub async fn agentspec_update_scope(
        &self,
        namespace_id: &str,
        agent_spec_name: &str,
        scope: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_AGENTSPECS_SCOPE,
            &AgentSpecWriteForm {
                namespace_id,
                agent_spec_name: Some(agent_spec_name),
                scope: Some(scope),
                ..Default::default()
            },
        )
        .await
    }

    /// Delete an MCP server draft (`deleteMcpServerDraft`).
    pub async fn mcp_server_delete_draft(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        version: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Delete,
            admin_api_path::AI_MCP_DRAFT,
            &McpWriteForm {
                namespace_id,
                mcp_name: Some(mcp_name),
                version: Some(version),
                ..Default::default()
            },
        )
        .await
    }

    /// Replace an MCP server's label-to-version mapping (JSON string).
    pub async fn mcp_server_update_labels(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        labels: &str,
    ) -> anyhow::Result<HashMap<String, String>> {
        let response: ApiResponse<HashMap<String, String>> = self
            .http_client
            .put_form(
                admin_api_path::AI_MCP_LABELS,
                &McpWriteForm {
                    namespace_id,
                    mcp_name: Some(mcp_name),
                    labels: Some(labels),
                    ..Default::default()
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Enable or disable an MCP server.
    pub async fn mcp_server_update_status(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        enabled: bool,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_MCP_STATUS,
            &McpWriteForm {
                namespace_id,
                mcp_name: Some(mcp_name),
                enabled: Some(enabled),
                ..Default::default()
            },
        )
        .await
    }

    /// Change an MCP server's visibility scope.
    pub async fn mcp_server_update_scope(
        &self,
        namespace_id: &str,
        mcp_name: &str,
        scope: &str,
    ) -> anyhow::Result<bool> {
        self.ai_write_ok(
            AiWriteVerb::Put,
            admin_api_path::AI_MCP_SCOPE,
            &McpWriteForm {
                namespace_id,
                mcp_name: Some(mcp_name),
                scope: Some(scope),
                ..Default::default()
            },
        )
        .await
    }

    // ============================================================================
    // Plugin Management APIs
    // ============================================================================

    /// List all plugins, optionally filtered by type.
    ///
    /// Matches Nacos `CoreMaintainerService.listPlugins()`.
    pub async fn plugin_list(&self, plugin_type: Option<&str>) -> anyhow::Result<Vec<PluginInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            plugin_type: Option<&'a str>,
        }

        let response: ApiResponse<Vec<PluginInfo>> = self
            .http_client
            .get_with_query(admin_api_path::PLUGIN_LIST, &Query { plugin_type })
            .await?;
        Ok(response.data)
    }

    /// Get detailed information for a specific plugin.
    ///
    /// Matches Nacos `CoreMaintainerService.getPluginDetail()`.
    pub async fn plugin_detail(
        &self,
        plugin_type: &str,
        plugin_name: &str,
    ) -> anyhow::Result<PluginDetail> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            plugin_type: &'a str,
            plugin_name: &'a str,
        }

        let response: ApiResponse<PluginDetail> = self
            .http_client
            .get_with_query(
                admin_api_path::PLUGIN_DETAIL,
                &Query {
                    plugin_type,
                    plugin_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Enable or disable a plugin.
    ///
    /// When `local_only` is true, only the current node is affected.
    /// Matches Nacos `CoreMaintainerService.updatePluginStatus()`.
    pub async fn plugin_update_status(
        &self,
        plugin_type: &str,
        plugin_name: &str,
        enabled: bool,
        local_only: bool,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            plugin_type: &'a str,
            plugin_name: &'a str,
            enabled: bool,
            local_only: bool,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .put_form(
                admin_api_path::PLUGIN_STATUS,
                &Form {
                    plugin_type,
                    plugin_name,
                    enabled,
                    local_only,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Update plugin configuration.
    ///
    /// When `local_only` is true, only the current node is affected.
    /// Matches Nacos `CoreMaintainerService.updatePluginConfig()`.
    pub async fn plugin_update_config(
        &self,
        plugin_type: &str,
        plugin_name: &str,
        config: &HashMap<String, String>,
        local_only: bool,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            plugin_type: &'a str,
            plugin_name: &'a str,
            config: &'a str,
            local_only: bool,
        }

        let config_json = serde_json::to_string(config)?;
        let response: ApiResponse<bool> = self
            .http_client
            .put_form(
                admin_api_path::PLUGIN_CONFIG,
                &Form {
                    plugin_type,
                    plugin_name,
                    config: &config_json,
                    local_only,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Check plugin availability across cluster nodes.
    ///
    /// Matches Nacos `CoreMaintainerService.getPluginAvailability()`.
    pub async fn plugin_availability(
        &self,
        plugin_type: &str,
        plugin_name: &str,
    ) -> anyhow::Result<PluginAvailability> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            plugin_type: &'a str,
            plugin_name: &'a str,
        }

        let response: ApiResponse<PluginAvailability> = self
            .http_client
            .get_with_query(
                admin_api_path::PLUGIN_AVAILABILITY,
                &Query {
                    plugin_type,
                    plugin_name,
                },
            )
            .await?;
        Ok(response.data)
    }
}
