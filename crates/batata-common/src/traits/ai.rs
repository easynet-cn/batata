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
    async fn get_skill_detail(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Option<SkillMeta>>;

    async fn get_skill_version_detail(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<Skill>>;

    async fn download_skill_version(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<Skill>>;

    async fn delete_skill(&self, namespace_id: &str, name: &str) -> anyhow::Result<()>;

    async fn list_skills(
        &self,
        namespace_id: &str,
        skill_name: Option<&str>,
        search: Option<&str>,
        order_by: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<SkillSummary>>;

    async fn upload_skill(
        &self,
        namespace_id: &str,
        name: &str,
        skill: &Skill,
        author: &str,
        overwrite: bool,
    ) -> anyhow::Result<String>;

    async fn create_draft(
        &self,
        namespace_id: &str,
        name: &str,
        based_on_version: Option<&str>,
        target_version: Option<&str>,
        initial_content: Option<&Skill>,
        author: &str,
    ) -> anyhow::Result<String>;

    async fn update_draft(
        &self,
        namespace_id: &str,
        name: &str,
        skill: &Skill,
    ) -> anyhow::Result<()>;

    async fn delete_draft(&self, namespace_id: &str, name: &str) -> anyhow::Result<()>;

    async fn submit(&self, namespace_id: &str, name: &str, version: &str)
    -> anyhow::Result<String>;

    async fn publish(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        update_latest_label: bool,
    ) -> anyhow::Result<()>;

    async fn update_labels(
        &self,
        namespace_id: &str,
        name: &str,
        labels: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<()>;

    async fn update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        biz_tags: &str,
    ) -> anyhow::Result<()>;

    async fn change_online_status(
        &self,
        namespace_id: &str,
        name: &str,
        scope: Option<&str>,
        version: Option<&str>,
        online: bool,
    ) -> anyhow::Result<()>;

    async fn update_scope(&self, namespace_id: &str, name: &str, scope: &str)
    -> anyhow::Result<()>;

    async fn query_skill(
        &self,
        namespace_id: &str,
        name: &str,
        version: Option<&str>,
        label: Option<&str>,
    ) -> anyhow::Result<Option<Skill>>;

    async fn search_skills(
        &self,
        namespace_id: &str,
        keyword: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<SkillBasicInfo>>;
}

/// Trait for agentspec lifecycle operations (CRUD, draft, publish, etc.)
#[async_trait::async_trait]
pub trait AgentSpecService: Send + Sync {
    async fn get_detail(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Option<AgentSpecMeta>>;

    async fn get_version_detail(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<AgentSpec>>;

    async fn delete(&self, namespace_id: &str, name: &str) -> anyhow::Result<()>;

    async fn list(
        &self,
        namespace_id: &str,
        name_filter: Option<&str>,
        search: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentSpecSummary>>;

    async fn upload(
        &self,
        namespace_id: &str,
        name: &str,
        spec: &AgentSpec,
        author: &str,
        overwrite: bool,
    ) -> anyhow::Result<String>;

    async fn create_draft(
        &self,
        namespace_id: &str,
        name: &str,
        based_on_version: Option<&str>,
        target_version: Option<&str>,
        initial_content: Option<&AgentSpec>,
        author: &str,
    ) -> anyhow::Result<String>;

    async fn update_draft(
        &self,
        namespace_id: &str,
        name: &str,
        spec: &AgentSpec,
    ) -> anyhow::Result<()>;

    async fn delete_draft(&self, namespace_id: &str, name: &str) -> anyhow::Result<()>;

    async fn submit(&self, namespace_id: &str, name: &str, version: &str)
    -> anyhow::Result<String>;

    async fn publish(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        update_latest_label: bool,
    ) -> anyhow::Result<()>;

    async fn update_labels(
        &self,
        namespace_id: &str,
        name: &str,
        labels: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<()>;

    async fn update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        biz_tags: &str,
    ) -> anyhow::Result<()>;

    async fn change_online_status(
        &self,
        namespace_id: &str,
        name: &str,
        scope: Option<&str>,
        version: Option<&str>,
        online: bool,
    ) -> anyhow::Result<()>;

    async fn update_scope(&self, namespace_id: &str, name: &str, scope: &str)
    -> anyhow::Result<()>;

    async fn query(
        &self,
        namespace_id: &str,
        name: &str,
        version: Option<&str>,
        label: Option<&str>,
    ) -> anyhow::Result<Option<AgentSpec>>;

    async fn search(
        &self,
        namespace_id: &str,
        keyword: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentSpecBasicInfo>>;
}

/// Trait for MCP server CRUD operations (config-backed persistence)
#[async_trait::async_trait]
pub trait McpServerService: Send + Sync {
    async fn create_mcp_server(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<String>;

    async fn get_mcp_server_detail(
        &self,
        namespace: &str,
        id: Option<&str>,
        name: Option<&str>,
        version: Option<&str>,
    ) -> anyhow::Result<Option<McpServer>>;

    async fn update_mcp_server(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<()>;

    async fn delete_mcp_server(
        &self,
        namespace: &str,
        name: Option<&str>,
        id: Option<&str>,
        version: Option<&str>,
    ) -> anyhow::Result<()>;

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

    /// Import MCP servers from a config (e.g., claude_desktop_config.json)
    async fn import_mcp_servers(
        &self,
        request: crate::model::ai::mcp::McpServerImportRequest,
    ) -> anyhow::Result<crate::model::ai::a2a::BatchRegistrationResponse>;

    /// Get registry statistics
    async fn mcp_stats(&self) -> anyhow::Result<McpRegistryStats>;
}

/// Trait for A2A agent CRUD operations (config-backed persistence)
#[async_trait::async_trait]
pub trait A2aAgentService: Send + Sync {
    async fn register_agent(
        &self,
        card: &AgentCard,
        namespace: &str,
        registration_type: &str,
    ) -> anyhow::Result<String>;

    async fn get_agent_card(
        &self,
        namespace: &str,
        agent_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<Option<RegisteredAgent>>;

    async fn update_agent_card(
        &self,
        card: &AgentCard,
        namespace: &str,
        registration_type: &str,
    ) -> anyhow::Result<()>;

    async fn delete_agent(
        &self,
        namespace: &str,
        agent_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()>;

    async fn list_agents(
        &self,
        namespace: &str,
        agent_name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<Page<AgentCardVersionInfo>>;

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
    async fn get_pipeline(&self, execution_id: &str) -> anyhow::Result<Option<PipelineExecution>>;

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
