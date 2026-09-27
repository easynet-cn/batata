//! AI service traits: SkillService, AgentSpecService, McpServerService,
//! A2aAgentService, PipelineService.

use crate::model::Page;
use crate::model::ai::VersionDetail;
use crate::model::ai::a2a::{
    AgentCard, AgentCardVersionInfo, AgentRegistryStats, BatchAgentRegistrationRequest,
    BatchRegistrationResponse, RegisteredAgent,
};
use crate::model::ai::agentspec::{AgentSpec, AgentSpecBasicInfo, AgentSpecMeta, AgentSpecSummary};
use crate::model::ai::mcp::McpRegistryStats;
use crate::model::ai::mcp::{McpServer, McpServerBasicInfo, McpServerRegistration};
use crate::model::ai::pipeline::PipelineExecution;
use crate::model::ai::skill::{Skill, SkillBasicInfo, SkillMeta, SkillSummary};

/// Trait for skill lifecycle operations (CRUD, draft, publish, etc.)
#[async_trait::async_trait]
pub trait SkillService: Send + Sync {
    /// The `get_skill_detail` method.
    async fn get_skill_detail(
        &self,
        namespace_id: &str,
        name: &str,
        user: Option<&str>,
    ) -> anyhow::Result<Option<SkillMeta>>;

    /// The `get_skill_version_detail` method.
    async fn get_skill_version_detail(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        user: Option<&str>,
    ) -> anyhow::Result<Option<Skill>>;

    /// The `download_skill_version` method.
    async fn download_skill_version(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        user: Option<&str>,
    ) -> anyhow::Result<Option<Skill>>;

    /// The `delete_skill` method.
    async fn delete_skill(
        &self,
        namespace_id: &str,
        name: &str,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `list_skills` method.
    async fn list_skills(
        &self,
        namespace_id: &str,
        skill_name: Option<&str>,
        search: Option<&str>,
        order_by: Option<&str>,
        page_no: u64,
        page_size: u64,
        user: Option<&str>,
    ) -> anyhow::Result<Page<SkillSummary>>;

    /// The `upload_skill` method.
    async fn upload_skill(
        &self,
        namespace_id: &str,
        name: &str,
        skill: &Skill,
        author: &str,
        overwrite: bool,
    ) -> anyhow::Result<String>;

    /// The `create_draft` method.
    async fn create_draft(
        &self,
        namespace_id: &str,
        name: &str,
        based_on_version: Option<&str>,
        target_version: Option<&str>,
        initial_content: Option<&Skill>,
        author: &str,
    ) -> anyhow::Result<String>;

    /// The `update_draft` method.
    async fn update_draft(
        &self,
        namespace_id: &str,
        name: &str,
        skill: &Skill,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `delete_draft` method.
    async fn delete_draft(
        &self,
        namespace_id: &str,
        name: &str,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `submit` method.
    async fn submit(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        user: Option<&str>,
    ) -> anyhow::Result<String>;

    /// The `publish` method.
    async fn publish(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `update_labels` method.
    async fn update_labels(
        &self,
        namespace_id: &str,
        name: &str,
        labels: std::collections::HashMap<String, String>,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `update_biz_tags` method.
    async fn update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        biz_tags: &str,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `change_online_status` method.
    async fn change_online_status(
        &self,
        namespace_id: &str,
        name: &str,
        scope: Option<&str>,
        version: Option<&str>,
        online: bool,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `update_scope` method.
    async fn update_scope(
        &self,
        namespace_id: &str,
        name: &str,
        scope: &str,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `query_skill` method.
    async fn query_skill(
        &self,
        namespace_id: &str,
        name: &str,
        version: Option<&str>,
        label: Option<&str>,
        user: Option<&str>,
    ) -> anyhow::Result<Option<Skill>>;

    /// The `search_skills` method.
    async fn search_skills(
        &self,
        namespace_id: &str,
        keyword: Option<&str>,
        page_no: u64,
        page_size: u64,
        user: Option<&str>,
    ) -> anyhow::Result<Page<SkillBasicInfo>>;
}

/// Trait for agentspec lifecycle operations (CRUD, draft, publish, etc.)
#[async_trait::async_trait]
pub trait AgentSpecService: Send + Sync {
    /// The `get_detail` method.
    async fn get_detail(
        &self,
        namespace_id: &str,
        name: &str,
        user: Option<&str>,
    ) -> anyhow::Result<Option<AgentSpecMeta>>;

    /// The `get_version_detail` method.
    async fn get_version_detail(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        user: Option<&str>,
    ) -> anyhow::Result<Option<AgentSpec>>;

    /// The `delete` method.
    async fn delete(
        &self,
        namespace_id: &str,
        name: &str,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `list` method.
    async fn list(
        &self,
        namespace_id: &str,
        name_filter: Option<&str>,
        search: Option<&str>,
        page_no: u64,
        page_size: u64,
        user: Option<&str>,
    ) -> anyhow::Result<Page<AgentSpecSummary>>;

    /// The `upload` method.
    async fn upload(
        &self,
        namespace_id: &str,
        name: &str,
        spec: &AgentSpec,
        author: &str,
        overwrite: bool,
    ) -> anyhow::Result<String>;

    /// The `create_draft` method.
    async fn create_draft(
        &self,
        namespace_id: &str,
        name: &str,
        based_on_version: Option<&str>,
        target_version: Option<&str>,
        initial_content: Option<&AgentSpec>,
        author: &str,
    ) -> anyhow::Result<String>;

    /// The `update_draft` method.
    async fn update_draft(
        &self,
        namespace_id: &str,
        name: &str,
        spec: &AgentSpec,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `delete_draft` method.
    async fn delete_draft(
        &self,
        namespace_id: &str,
        name: &str,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `submit` method.
    async fn submit(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        user: Option<&str>,
    ) -> anyhow::Result<String>;

    /// The `publish` method.
    async fn publish(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `update_labels` method.
    async fn update_labels(
        &self,
        namespace_id: &str,
        name: &str,
        labels: std::collections::HashMap<String, String>,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `update_biz_tags` method.
    async fn update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        biz_tags: &str,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `change_online_status` method.
    async fn change_online_status(
        &self,
        namespace_id: &str,
        name: &str,
        scope: Option<&str>,
        version: Option<&str>,
        online: bool,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `update_scope` method.
    async fn update_scope(
        &self,
        namespace_id: &str,
        name: &str,
        scope: &str,
        user: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `query` method.
    async fn query(
        &self,
        namespace_id: &str,
        name: &str,
        version: Option<&str>,
        label: Option<&str>,
        user: Option<&str>,
    ) -> anyhow::Result<Option<AgentSpec>>;

    /// The `search` method.
    async fn search(
        &self,
        namespace_id: &str,
        keyword: Option<&str>,
        page_no: u64,
        page_size: u64,
        user: Option<&str>,
    ) -> anyhow::Result<Page<AgentSpecBasicInfo>>;
}

/// Trait for MCP server CRUD operations (config-backed persistence)
#[async_trait::async_trait]
pub trait McpServerService: Send + Sync {
    /// The `create_mcp_server` method.
    async fn create_mcp_server(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<String>;

    /// The `get_mcp_server_detail` method.
    async fn get_mcp_server_detail(
        &self,
        namespace: &str,
        id: Option<&str>,
        name: Option<&str>,
        version: Option<&str>,
    ) -> anyhow::Result<Option<McpServer>>;

    /// The `update_mcp_server` method.
    async fn update_mcp_server(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<()>;

    /// The `delete_mcp_server` method.
    async fn delete_mcp_server(
        &self,
        namespace: &str,
        name: Option<&str>,
        id: Option<&str>,
        version: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `list_mcp_servers` method.
    fn list_mcp_servers(
        &self,
        namespace: &str,
        name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
    ) -> Page<McpServerBasicInfo>;

    /// Import tools from a running MCP server via SSE transport
    async fn import_tools_from_mcp(
        &self,
        base_url: &str,
        endpoint: &str,
        auth_token: Option<&str>,
        timeout: std::time::Duration,
    ) -> anyhow::Result<Vec<crate::model::ai::mcp::McpTool>>;

    /// Get registry statistics
    async fn mcp_stats(&self) -> anyhow::Result<McpRegistryStats>;
}

/// Trait for A2A agent CRUD operations (config-backed persistence)
#[async_trait::async_trait]
pub trait A2aAgentService: Send + Sync {
    /// The `register_agent` method.
    async fn register_agent(
        &self,
        card: &AgentCard,
        namespace: &str,
        registration_type: &str,
    ) -> anyhow::Result<String>;

    /// The `get_agent_card` method.
    async fn get_agent_card(
        &self,
        namespace: &str,
        agent_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<Option<RegisteredAgent>>;

    /// The `update_agent_card` method.
    async fn update_agent_card(
        &self,
        card: &AgentCard,
        namespace: &str,
        registration_type: &str,
    ) -> anyhow::Result<()>;

    /// The `delete_agent` method.
    async fn delete_agent(
        &self,
        namespace: &str,
        agent_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()>;

    /// The `list_agents` method.
    async fn list_agents(
        &self,
        namespace: &str,
        agent_name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<Page<AgentCardVersionInfo>>;

    /// The `list_versions` method.
    async fn list_versions(
        &self,
        namespace: &str,
        agent_name: &str,
    ) -> anyhow::Result<Vec<VersionDetail>>;

    /// Find agents that provide a specific skill
    async fn find_by_skill(&self, skill: &str) -> anyhow::Result<Vec<RegisteredAgent>>;

    /// Batch register multiple agents
    async fn batch_register(
        &self,
        request: BatchAgentRegistrationRequest,
    ) -> anyhow::Result<BatchRegistrationResponse>;

    /// Get registry statistics
    async fn stats(&self) -> anyhow::Result<AgentRegistryStats>;
}

/// Trait for pipeline query operations
#[async_trait::async_trait]
pub trait PipelineService: Send + Sync {
    /// The `get_pipeline` method.
    async fn get_pipeline(&self, execution_id: &str) -> anyhow::Result<Option<PipelineExecution>>;

    /// The `list_pipelines` method.
    async fn list_pipelines(
        &self,
        resource_type: &str,
        resource_name: Option<&str>,
        namespace_id: Option<&str>,
        version: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PipelineExecution>>;
}
