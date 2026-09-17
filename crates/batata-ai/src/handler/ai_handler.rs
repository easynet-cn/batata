// AI module gRPC handlers for MCP and A2A
// Implements handlers for MCP server and A2A agent management via gRPC
// Uses config-backed operation services when available, falls back to in-memory registries

use std::sync::Arc;

use tonic::Status;
use tracing::{debug, warn};

use batata_core::{GrpcResource, PermissionAction, ResourceType, model::Connection};

use batata_api::{
    grpc::Payload,
    remote::model::{
        AgentCatalogEntry, AgentCatalogVersion, AgentDiscoveryCallInterface,
        AgentDiscoveryResponse, AgentDiscoveryResult, AgentDiscoveryRpcRequest, AgentEndpointInfo,
        AgentEndpointOperationResponse, AgentEndpointRegisterRpcRequest,
        AgentEndpointDeregisterRpcRequest, AgentSearchPage, AgentSearchResponse,
        AgentSearchRpcRequest, EndpointSet, AgentEndpointRequest, AgentEndpointResponse,
        McpServerEndpointRequest, McpServerEndpointResponse, QueryAgentCardRequest,
        QueryAgentCardResponse, QueryMcpServerRequest, QueryMcpServerResponse,
        ReleaseAgentCardRequest, ReleaseAgentCardResponse, ReleaseMcpServerRequest,
        ReleaseMcpServerResponse, RequestTrait, ResponseTrait,
    },
};

use batata_core::handler::rpc::{AuthRequirement, PayloadHandler};

use crate::{
    model::{AgentCard, AgentRegistrationRequest, McpServerRegistration},
    registry::a2a::AgentRegistry,
    registry::mcp::McpServerRegistry,
    service::{
        AiEndpointService,
        traits::{A2aAgentService, McpServerService},
    },
};

// =============================================================================
// MCP Server Handlers
// =============================================================================

/// Handler for McpServerEndpointRequest - register/deregister MCP server endpoint
#[derive(Clone)]
pub struct McpServerEndpointHandler {
    /// In-memory MCP server registry used as a fallback target.
    pub mcp_registry: Arc<McpServerRegistry>,
    /// Optional config-backed MCP server operation service.
    pub mcp_service: Option<Arc<dyn McpServerService>>,
    /// Optional endpoint registration service.
    pub endpoint_service: Option<Arc<AiEndpointService>>,
}

#[tonic::async_trait]
impl PayloadHandler for McpServerEndpointHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = McpServerEndpointRequest::from(payload);
        let request_id = request.request_id();
        let operation = &request.operation_type;

        debug!(
            operation = %operation,
            namespace = %request.namespace_id,
            name = %request.mcp_name,
            "Processing MCP server endpoint request"
        );

        match operation.as_str() {
            "registerEndpoint" | "register" => {
                // Register endpoint via endpoint service if available
                if let Some(ref ep_svc) = self.endpoint_service {
                    ep_svc.create_mcp_endpoint(
                        &request.namespace_id,
                        &request.mcp_name,
                        &request.version,
                        &request.address,
                        request.port,
                    );
                }

                // Also register in in-memory registry
                let endpoint = format!("{}:{}", request.address, request.port);
                let registration = McpServerRegistration {
                    name: request.mcp_name.clone(),
                    namespace: request.namespace_id.clone(),
                    version: request.version.clone(),
                    endpoint,
                    ..default_mcp_registration()
                };

                let _ = self.mcp_registry.register(registration);

                let mut response = McpServerEndpointResponse::new();
                response.response.request_id = request_id;
                response.operation_type = "register".to_string();
                Ok(response.build_payload())
            }
            "deregisterEndpoint" | "deregister" => {
                // Deregister endpoint via endpoint service if available
                if let Some(ref ep_svc) = self.endpoint_service {
                    ep_svc.delete_mcp_endpoint(
                        &request.namespace_id,
                        &request.mcp_name,
                        &request.version,
                        &request.address,
                        request.port,
                    );
                }

                let _ = self
                    .mcp_registry
                    .deregister(&request.namespace_id, &request.mcp_name);

                let mut response = McpServerEndpointResponse::new();
                response.response.request_id = request_id;
                response.operation_type = "deregister".to_string();
                Ok(response.build_payload())
            }
            _ => {
                warn!(operation = %operation, "Unknown MCP endpoint operation type");
                let response = batata_core::error_response!(
                    McpServerEndpointResponse,
                    request_id,
                    format!("Unknown operation type: {}", operation)
                );
                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "McpServerEndpointRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "ai"
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Ai
    }

    fn resource_from_payload(&self, payload: &Payload) -> Option<(GrpcResource, PermissionAction)> {
        let request = McpServerEndpointRequest::from(payload);
        Some((
            GrpcResource::ai(&request.namespace_id, &request.mcp_name),
            PermissionAction::Write,
        ))
    }
}

/// Handler for QueryMcpServerRequest - query MCP server details
#[derive(Clone)]
pub struct QueryMcpServerHandler {
    /// In-memory MCP server registry used as a fallback source.
    pub mcp_registry: Arc<McpServerRegistry>,
    /// Optional config-backed MCP server operation service.
    pub mcp_service: Option<Arc<dyn McpServerService>>,
}

#[tonic::async_trait]
impl PayloadHandler for QueryMcpServerHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = QueryMcpServerRequest::from(payload);
        let request_id = request.request_id();

        debug!(
            namespace = %request.namespace_id,
            name = %request.mcp_name,
            "Querying MCP server"
        );

        // Try operation service first
        if let Some(ref svc) = self.mcp_service {
            match svc
                .get_mcp_server_detail(&request.namespace_id, None, Some(&request.mcp_name), None)
                .await
            {
                Ok(Some(server)) => {
                    let detail = serde_json::to_value(&server).unwrap_or_default();
                    let mut response = QueryMcpServerResponse::new();
                    response.response.request_id = request_id;
                    response.mcp_server_detail_info = detail;
                    return Ok(response.build_payload());
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }

        // Fall back to in-memory registry
        match self
            .mcp_registry
            .get(&request.namespace_id, &request.mcp_name)
        {
            Some(server) => {
                let detail = serde_json::to_value(&server).unwrap_or_default();
                let mut response = QueryMcpServerResponse::new();
                response.response.request_id = request_id;
                response.mcp_server_detail_info = detail;
                Ok(response.build_payload())
            }
            None => {
                let response = batata_core::error_response!(
                    QueryMcpServerResponse,
                    request_id,
                    format!(
                        "MCP server '{}' not found in namespace '{}'",
                        request.mcp_name, request.namespace_id
                    )
                );
                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "QueryMcpServerRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Read
    }

    fn sign_type(&self) -> &'static str {
        "ai"
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Ai
    }

    fn resource_from_payload(&self, payload: &Payload) -> Option<(GrpcResource, PermissionAction)> {
        let request = QueryMcpServerRequest::from(payload);
        Some((
            GrpcResource::ai(&request.namespace_id, &request.mcp_name),
            PermissionAction::Read,
        ))
    }
}

/// Handler for ReleaseMcpServerRequest - publish/release an MCP server
#[derive(Clone)]
pub struct ReleaseMcpServerHandler {
    /// In-memory MCP server registry used as a fallback target.
    pub mcp_registry: Arc<McpServerRegistry>,
    /// Optional config-backed MCP server operation service.
    pub mcp_service: Option<Arc<dyn McpServerService>>,
}

#[tonic::async_trait]
impl PayloadHandler for ReleaseMcpServerHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = ReleaseMcpServerRequest::from(payload);
        let request_id = request.request_id();

        debug!(
            namespace = %request.namespace_id,
            name = %request.mcp_name,
            "Releasing MCP server"
        );

        // Parse server_specification into a McpServerRegistration
        let mut registration: McpServerRegistration =
            serde_json::from_value(request.server_specification.clone()).unwrap_or_else(|_| {
                McpServerRegistration {
                    name: request.mcp_name.clone(),
                    namespace: request.namespace_id.clone(),
                    ..default_mcp_registration()
                }
            });
        registration.name = request.mcp_name.clone();
        registration.namespace = request.namespace_id.clone();

        // Try operation service first
        if let Some(ref svc) = self.mcp_service {
            // Try create, then update on conflict
            match svc
                .create_mcp_server(&request.namespace_id, &registration)
                .await
            {
                Ok(id) => {
                    let _ = self.mcp_registry.register(registration);
                    let mut response = ReleaseMcpServerResponse::new();
                    response.response.request_id = request_id;
                    response.mcp_id = id;
                    return Ok(response.build_payload());
                }
                Err(_) => {
                    // Already exists, try update
                    match svc
                        .update_mcp_server(&request.namespace_id, &registration)
                        .await
                    {
                        Ok(()) => {
                            let _ = self.mcp_registry.update(
                                &request.namespace_id,
                                &request.mcp_name,
                                registration,
                            );
                            let mut response = ReleaseMcpServerResponse::new();
                            response.response.request_id = request_id;
                            return Ok(response.build_payload());
                        }
                        Err(e) => {
                            let response = batata_core::error_response!(
                                ReleaseMcpServerResponse,
                                request_id,
                                format!("Failed to release MCP server: {}", e)
                            );
                            return Ok(response.build_payload());
                        }
                    }
                }
            }
        }

        // Fall back to in-memory registry
        match self.mcp_registry.register(registration.clone()) {
            Ok(server) => {
                let mut response = ReleaseMcpServerResponse::new();
                response.response.request_id = request_id;
                response.mcp_id = server.id;
                Ok(response.build_payload())
            }
            Err(_) => {
                // Try update
                match self.mcp_registry.update(
                    &request.namespace_id,
                    &request.mcp_name,
                    registration,
                ) {
                    Ok(server) => {
                        let mut response = ReleaseMcpServerResponse::new();
                        response.response.request_id = request_id;
                        response.mcp_id = server.id;
                        Ok(response.build_payload())
                    }
                    Err(e) => {
                        let response = batata_core::error_response!(
                            ReleaseMcpServerResponse,
                            request_id,
                            format!("Failed to release MCP server: {}", e)
                        );
                        Ok(response.build_payload())
                    }
                }
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "ReleaseMcpServerRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "ai"
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Ai
    }

    fn resource_from_payload(&self, payload: &Payload) -> Option<(GrpcResource, PermissionAction)> {
        let request = ReleaseMcpServerRequest::from(payload);
        Some((
            GrpcResource::ai(&request.namespace_id, &request.mcp_name),
            PermissionAction::Write,
        ))
    }
}

// =============================================================================
// A2A Agent Handlers
// =============================================================================

/// Handler for AgentEndpointRequest - register/deregister agent endpoint
#[derive(Clone)]
pub struct AgentEndpointHandler {
    /// In-memory A2A agent registry used as a fallback target.
    pub agent_registry: Arc<AgentRegistry>,
    /// Optional config-backed A2A agent service.
    pub a2a_service: Option<Arc<dyn A2aAgentService>>,
    /// Optional endpoint registration service.
    pub endpoint_service: Option<Arc<AiEndpointService>>,
}

#[tonic::async_trait]
impl PayloadHandler for AgentEndpointHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = AgentEndpointRequest::from(payload);
        let request_id = request.request_id();
        let operation = &request.operation_type;

        debug!(
            operation = %operation,
            namespace = %request.namespace_id,
            name = %request.agent_name,
            "Processing agent endpoint request"
        );

        match operation.as_str() {
            "registerEndpoint" | "register" => {
                let endpoint_info = request.endpoint.as_ref();
                let endpoint_url = endpoint_info
                    .map(|ep| {
                        let scheme = if ep.support_tls { "https" } else { "http" };
                        if ep.path.is_empty() {
                            format!("{}://{}:{}", scheme, ep.address, ep.port)
                        } else {
                            format!("{}://{}:{}{}", scheme, ep.address, ep.port, ep.path)
                        }
                    })
                    .unwrap_or_default();

                let version = endpoint_info
                    .map(|ep| ep.version.clone())
                    .unwrap_or_default();

                // Register endpoint via endpoint service if available
                if let (Some(ep_svc), Some(ep_info)) = (&self.endpoint_service, &request.endpoint) {
                    ep_svc.create_agent_endpoint(
                        &request.namespace_id,
                        &request.agent_name,
                        &ep_info.version,
                        &ep_info.address,
                        ep_info.port,
                    );
                }

                let card = AgentCard {
                    name: request.agent_name.clone(),
                    url: endpoint_url,
                    version,
                    ..default_agent_card()
                };

                let reg_request = AgentRegistrationRequest {
                    card,
                    namespace: request.namespace_id.clone(),
                };

                let _ = self.agent_registry.register(reg_request);

                let mut response = AgentEndpointResponse::new();
                response.response.request_id = request_id;
                response.operation_type = "register".to_string();
                Ok(response.build_payload())
            }
            "deregisterEndpoint" | "deregister" => {
                // Deregister endpoint via endpoint service if available
                if let (Some(ep_svc), Some(ep_info)) = (&self.endpoint_service, &request.endpoint) {
                    ep_svc.delete_agent_endpoint(
                        &request.namespace_id,
                        &request.agent_name,
                        &ep_info.version,
                        &ep_info.address,
                        ep_info.port,
                    );
                }

                let _ = self
                    .agent_registry
                    .deregister(&request.namespace_id, &request.agent_name);

                let mut response = AgentEndpointResponse::new();
                response.response.request_id = request_id;
                response.operation_type = "deregister".to_string();
                Ok(response.build_payload())
            }
            _ => {
                warn!(operation = %operation, "Unknown agent endpoint operation type");
                let response = batata_core::error_response!(
                    AgentEndpointResponse,
                    request_id,
                    format!("Unknown operation type: {}", operation)
                );
                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "AgentEndpointRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "ai"
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Ai
    }

    fn resource_from_payload(&self, payload: &Payload) -> Option<(GrpcResource, PermissionAction)> {
        let request = AgentEndpointRequest::from(payload);
        Some((
            GrpcResource::ai(&request.namespace_id, &request.agent_name),
            PermissionAction::Write,
        ))
    }
}

/// Handler for QueryAgentCardRequest - query agent card details
#[derive(Clone)]
pub struct QueryAgentCardHandler {
    /// In-memory A2A agent registry used as a fallback source.
    pub agent_registry: Arc<AgentRegistry>,
    /// Optional config-backed A2A agent service.
    pub a2a_service: Option<Arc<dyn A2aAgentService>>,
}

#[tonic::async_trait]
impl PayloadHandler for QueryAgentCardHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = QueryAgentCardRequest::from(payload);
        let request_id = request.request_id();

        debug!(
            namespace = %request.namespace_id,
            name = %request.agent_name,
            "Querying agent card"
        );

        // Try operation service first
        if let Some(ref svc) = self.a2a_service {
            match svc
                .get_agent_card(&request.namespace_id, &request.agent_name, None)
                .await
            {
                Ok(Some(agent)) => {
                    let detail = serde_json::to_value(&agent).unwrap_or_default();
                    let mut response = QueryAgentCardResponse::new();
                    response.response.request_id = request_id;
                    response.agent_card_detail_info = detail;
                    return Ok(response.build_payload());
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }

        // Fall back to in-memory registry
        match self
            .agent_registry
            .get(&request.namespace_id, &request.agent_name)
        {
            Some(agent) => {
                let detail = serde_json::to_value(&agent).unwrap_or_default();
                let mut response = QueryAgentCardResponse::new();
                response.response.request_id = request_id;
                response.agent_card_detail_info = detail;
                Ok(response.build_payload())
            }
            None => {
                let response = batata_core::error_response!(
                    QueryAgentCardResponse,
                    request_id,
                    format!(
                        "Agent '{}' not found in namespace '{}'",
                        request.agent_name, request.namespace_id
                    )
                );
                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "QueryAgentCardRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Read
    }

    fn sign_type(&self) -> &'static str {
        "ai"
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Ai
    }

    fn resource_from_payload(&self, payload: &Payload) -> Option<(GrpcResource, PermissionAction)> {
        let request = QueryAgentCardRequest::from(payload);
        Some((
            GrpcResource::ai(&request.namespace_id, &request.agent_name),
            PermissionAction::Read,
        ))
    }
}

/// Handler for ReleaseAgentCardRequest - publish/release an agent card
#[derive(Clone)]
pub struct ReleaseAgentCardHandler {
    /// In-memory A2A agent registry used as a fallback target.
    pub agent_registry: Arc<AgentRegistry>,
    /// Optional config-backed A2A agent service.
    pub a2a_service: Option<Arc<dyn A2aAgentService>>,
}

#[tonic::async_trait]
impl PayloadHandler for ReleaseAgentCardHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = ReleaseAgentCardRequest::from(payload);
        let request_id = request.request_id();

        debug!(
            namespace = %request.namespace_id,
            name = %request.agent_name,
            "Releasing agent card"
        );

        // Parse agent_card JSON into AgentCard
        let mut card: AgentCard = serde_json::from_value(request.agent_card.clone())
            .unwrap_or_else(|_| AgentCard {
                name: request.agent_name.clone(),
                ..default_agent_card()
            });
        card.name = request.agent_name.clone();

        // Try operation service first
        if let Some(ref svc) = self.a2a_service {
            // Try register, then update on conflict
            match svc
                .register_agent(&card, &request.namespace_id, "sdk")
                .await
            {
                Ok(_) => {
                    let reg = AgentRegistrationRequest {
                        card,
                        namespace: request.namespace_id.clone(),
                    };
                    let _ = self.agent_registry.register(reg);
                    let mut response = ReleaseAgentCardResponse::new();
                    response.response.request_id = request_id;
                    return Ok(response.build_payload());
                }
                Err(_) => {
                    match svc
                        .update_agent_card(&card, &request.namespace_id, "sdk")
                        .await
                    {
                        Ok(()) => {
                            let reg = AgentRegistrationRequest {
                                card,
                                namespace: request.namespace_id.clone(),
                            };
                            let _ = self.agent_registry.update(
                                &request.namespace_id,
                                &request.agent_name,
                                reg,
                            );
                            let mut response = ReleaseAgentCardResponse::new();
                            response.response.request_id = request_id;
                            return Ok(response.build_payload());
                        }
                        Err(e) => {
                            let response = batata_core::error_response!(
                                ReleaseAgentCardResponse,
                                request_id,
                                format!("Failed to release agent card: {}", e)
                            );
                            return Ok(response.build_payload());
                        }
                    }
                }
            }
        }

        // Fall back to in-memory registry
        let reg_request = AgentRegistrationRequest {
            card,
            namespace: request.namespace_id.clone(),
        };

        match self.agent_registry.register(reg_request.clone()) {
            Ok(_agent) => {
                let mut response = ReleaseAgentCardResponse::new();
                response.response.request_id = request_id;
                Ok(response.build_payload())
            }
            Err(_) => {
                match self.agent_registry.update(
                    &request.namespace_id,
                    &request.agent_name,
                    reg_request,
                ) {
                    Ok(_agent) => {
                        let mut response = ReleaseAgentCardResponse::new();
                        response.response.request_id = request_id;
                        Ok(response.build_payload())
                    }
                    Err(e) => {
                        let response = batata_core::error_response!(
                            ReleaseAgentCardResponse,
                            request_id,
                            format!("Failed to release agent card: {}", e)
                        );
                        Ok(response.build_payload())
                    }
                }
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "ReleaseAgentCardRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "ai"
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Ai
    }

    fn resource_from_payload(&self, payload: &Payload) -> Option<(GrpcResource, PermissionAction)> {
        let request = ReleaseAgentCardRequest::from(payload);
        Some((
            GrpcResource::ai(&request.namespace_id, &request.agent_name),
            PermissionAction::Write,
        ))
    }
}

// =============================================================================
// AI-RAD: Agent Search, Discovery, Endpoint Register/Deregister
// (Nacos 3.x Remote Agent Discovery protocol via gRPC)
// =============================================================================

/// Handler for AgentSearchRpcRequest — search visible agent catalog entries.
///
/// Mirrors Nacos `AgentSearchRpcRequestHandler`. Calls `A2aAgentService.list_agents()`
/// and converts results to `AgentCatalogEntry` page.
#[derive(Clone)]
pub struct AgentSearchRpcHandler {
    /// In-memory A2A agent registry used as a fallback source.
    pub agent_registry: Arc<AgentRegistry>,
    /// Optional config-backed A2A agent service.
    pub a2a_service: Option<Arc<dyn A2aAgentService>>,
}

#[tonic::async_trait]
impl PayloadHandler for AgentSearchRpcHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = AgentSearchRpcRequest::from(payload);
        let request_id = request.request_id();

        // Extract inner search request (fallback to empty default)
        let search = request.search_request.unwrap_or_default();
        let namespace_id = if search.namespace_id.is_empty() {
            "public"
        } else {
            &search.namespace_id
        };
        let page_no = if search.page_no == 0 { 1 } else { search.page_no };
        let page_size = if search.page_size == 0 {
            20
        } else {
            search.page_size
        };

        debug!(
            namespace = %namespace_id,
            name_contains = %search.agent_name_contains,
            page_no,
            page_size,
            "Processing AgentSearchRpcRequest"
        );

        // Try operation service first
        let agent_name_filter = if search.agent_name_contains.is_empty() {
            None
        } else {
            Some(search.agent_name_contains.as_str())
        };

        if let Some(ref svc) = self.a2a_service {
            match svc
                .list_agents(namespace_id, agent_name_filter, "blur", page_no, page_size)
                .await
            {
                Ok(page) => {
                    let entries: Vec<AgentCatalogEntry> = page
                        .page_items
                        .iter()
                        .map(|v| AgentCatalogEntry {
                            agent_name: v.name.clone(),
                            display_name: v.name.clone(),
                            description: String::new(),
                            icon_url: String::new(),
                            provider: None,
                            tags: vec![],
                            latest_version: v.latest_published_version.clone(),
                            versions: v
                                .version_details
                                .iter()
                                .map(|vd| AgentCatalogVersion {
                                    version: vd.version.clone(),
                                    labels: if vd.is_latest {
                                        vec!["latest".to_string()]
                                    } else {
                                        vec![]
                                    },
                                    protocols: vec!["a2a".to_string()],
                                })
                                .collect(),
                        })
                        .collect();

                    let total = page.total_count;
                    let pages_available = if page_size > 0 {
                        total.div_ceil(page_size as u64)
                    } else {
                        1
                    };

                    let mut response = AgentSearchResponse::new();
                    response.response.request_id = request_id;
                    response.page = Some(AgentSearchPage {
                        total_count: total,
                        page_number: page_no as u64,
                        pages_available,
                        page_items: entries,
                    });
                    return Ok(response.build_payload());
                }
                Err(e) => {
                    warn!("AgentSearchRpcRequest error: {}", e);
                    let response = batata_core::error_response!(
                        AgentSearchResponse,
                        request_id,
                        &e.to_string()
                    );
                    return Ok(response.build_payload());
                }
            }
        }

        // Fall back to in-memory registry
        let query = batata_common::model::ai::a2a::AgentQuery {
            namespace: Some(namespace_id.to_string()),
            name_pattern: if search.agent_name_contains.is_empty() {
                None
            } else {
                Some(search.agent_name_contains.clone())
            },
            page: page_no,
            page_size,
            ..Default::default()
        };
        let result = self.agent_registry.list(&query);

        let entries: Vec<AgentCatalogEntry> = result
            .agents
            .iter()
            .map(|a| AgentCatalogEntry {
                agent_name: a.card.name.clone(),
                display_name: a.card.display_name.clone(),
                description: a.card.description.clone(),
                icon_url: a.card.icon_url.clone().unwrap_or_default(),
                provider: None,
                tags: a.card.tags.clone(),
                latest_version: a.card.version.clone(),
                versions: vec![AgentCatalogVersion {
                    version: a.card.version.clone(),
                    labels: vec!["latest".to_string()],
                    protocols: vec!["a2a".to_string()],
                }],
            })
            .collect();

        let total = result.total;
        let pages_available = if page_size > 0 {
            total.div_ceil(page_size as u64)
        } else {
            1
        };

        let mut response = AgentSearchResponse::new();
        response.response.request_id = request_id;
        response.page = Some(AgentSearchPage {
            total_count: total,
            page_number: page_no as u64,
            pages_available,
            page_items: entries,
        });
        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "AgentSearchRpcRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Read
    }

    fn sign_type(&self) -> &'static str {
        "ai"
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Ai
    }
}

/// Handler for AgentDiscoveryRpcRequest — discover one exact agent version and its endpoints.
///
/// Mirrors Nacos `AgentDiscoveryRpcRequestHandler`. Resolves `AgentReference`
/// (version or label), queries endpoints via `AiEndpointService`, and assembles
/// `AgentDiscoveryResult` with `callInterfaces` + `endpointSets`.
#[derive(Clone)]
pub struct AgentDiscoveryRpcHandler {
    /// In-memory A2A agent registry used as a fallback source.
    pub agent_registry: Arc<AgentRegistry>,
    /// Optional config-backed A2A agent service.
    pub a2a_service: Option<Arc<dyn A2aAgentService>>,
    /// Optional endpoint registration service.
    pub endpoint_service: Option<Arc<AiEndpointService>>,
}

#[tonic::async_trait]
impl PayloadHandler for AgentDiscoveryRpcHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = AgentDiscoveryRpcRequest::from(payload);
        let request_id = request.request_id();

        // Extract inner discovery request
        let discovery = request.discovery_request.unwrap_or_default();
        let reference = discovery.reference;
        let namespace_id = if discovery.namespace_id.is_empty() {
            "public"
        } else {
            &discovery.namespace_id
        };

        if reference.agent_name.is_empty() {
            let response = batata_core::error_response!(
                AgentDiscoveryResponse,
                request_id,
                "agentName is required in reference"
            );
            return Ok(response.build_payload());
        }

        debug!(
            namespace = %namespace_id,
            agent_name = %reference.agent_name,
            version = %reference.version,
            label = %reference.label,
            "Processing AgentDiscoveryRpcRequest"
        );

        // Resolve version: explicit > label "latest" > query service
        let resolved_version = if !reference.version.is_empty() {
            reference.version.clone()
        } else {
            // Try to resolve via service
            let mut version = String::new();
            if let Some(ref svc) = self.a2a_service
                && let Ok(versions) = svc.list_versions(namespace_id, &reference.agent_name).await {
                    // If label is "latest" or empty, find the latest version
                    let label = if reference.label.is_empty() {
                        "latest"
                    } else {
                        &reference.label
                    };
                    if label == "latest" {
                        if let Some(latest) = versions.iter().find(|v| v.is_latest) {
                            version = latest.version.clone();
                        } else if let Some(first) = versions.first() {
                            version = first.version.clone();
                        }
                    } else {
                        // Try to match by label (not yet supported; fallback to latest)
                        if let Some(latest) = versions.iter().find(|v| v.is_latest) {
                            version = latest.version.clone();
                        } else if let Some(first) = versions.first() {
                            version = first.version.clone();
                        }
                    }
                }
            // Fall back to in-memory registry
            if version.is_empty()
                && let Some(agent) = self
                    .agent_registry
                    .get(namespace_id, &reference.agent_name)
                {
                    version = agent.card.version.clone();
                }
            version
        };

        if resolved_version.is_empty() {
            let response = batata_core::error_response!(
                AgentDiscoveryResponse,
                request_id,
                format!(
                    "Cannot resolve version for agent '{}' in namespace '{}'",
                    reference.agent_name, namespace_id
                )
            );
            return Ok(response.build_payload());
        }

        // Gather endpoints from NamingService
        let mut endpoint_list: Vec<AgentEndpointInfo> = Vec::new();
        if let Some(ref ep_svc) = self.endpoint_service {
            let endpoints = ep_svc.get_agent_endpoints(
                namespace_id,
                &reference.agent_name,
                &resolved_version,
            );
            endpoint_list = endpoints
                .iter()
                .map(|ep| AgentEndpointInfo {
                    address: ep.address.clone(),
                    port: ep.port,
                    transport: String::new(),
                    path: String::new(),
                    healthy: Some(ep.healthy),
                    metadata: ep.metadata.clone(),
                })
                .collect();
        }

        // Compute a simple content digest for change detection
        let digest_input = format!("{}:{}:{}", reference.agent_name, resolved_version, endpoint_list.len());
        let content_digest = format!("{:x}", md5_hash(digest_input.as_bytes()));

        // Build call interface with endpoint set (DECLARED source)
        let call_interface = AgentDiscoveryCallInterface {
            protocol: "a2a".to_string(),
            protocol_version: "1.0".to_string(),
            descriptor_media_type: "application/json".to_string(),
            native_descriptor: None,
            endpoint_sets: vec![EndpointSet {
                source: "DECLARED".to_string(),
                source_revision: resolved_version.clone(),
                endpoints: endpoint_list,
            }],
        };

        let result = AgentDiscoveryResult {
            namespace_id: namespace_id.to_string(),
            agent_name: reference.agent_name.clone(),
            version: resolved_version,
            content_digest,
            call_interfaces: vec![call_interface],
        };

        let mut response = AgentDiscoveryResponse::new();
        response.response.request_id = request_id;
        response.discovery_result = Some(result);
        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "AgentDiscoveryRpcRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Read
    }

    fn sign_type(&self) -> &'static str {
        "ai"
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Ai
    }
}

/// Handler for AgentEndpointRegisterRpcRequest — register agent endpoints (batch replace).
///
/// Mirrors Nacos `AgentEndpointRegisterRpcRequestHandler`. Parses the
/// `AgentEndpointRegistrationBatch` and registers each endpoint via
/// `AiEndpointService.create_agent_endpoint()`.
#[derive(Clone)]
pub struct AgentEndpointRegisterRpcHandler {
    /// In-memory A2A agent registry used as a fallback target.
    pub agent_registry: Arc<AgentRegistry>,
    /// Optional endpoint registration service.
    pub endpoint_service: Option<Arc<AiEndpointService>>,
}

#[tonic::async_trait]
impl PayloadHandler for AgentEndpointRegisterRpcHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = AgentEndpointRegisterRpcRequest::from(payload);
        let request_id = request.request_id();

        let batch = match request.registration_batch {
            Some(b) => b,
            None => {
                let response = batata_core::error_response!(
                    AgentEndpointOperationResponse,
                    request_id,
                    "registrationBatch is required"
                );
                return Ok(response.build_payload());
            }
        };

        let namespace_id = if batch.namespace_id.is_empty() {
            "public"
        } else {
            &batch.namespace_id
        };

        debug!(
            namespace = %namespace_id,
            agent_name = %batch.agent_name,
            version = %batch.runtime_version,
            protocol = %batch.protocol,
            endpoint_count = batch.endpoints.len(),
            "Processing AgentEndpointRegisterRpcRequest"
        );

        // Register each endpoint via endpoint service
        if let Some(ref ep_svc) = self.endpoint_service {
            for endpoint in &batch.endpoints {
                ep_svc.create_agent_endpoint(
                    namespace_id,
                    &batch.agent_name,
                    &batch.runtime_version,
                    &endpoint.address,
                    endpoint.port,
                );
            }
        }

        // Also update in-memory registry for fallback queries
        for endpoint in &batch.endpoints {
            let scheme = if endpoint.path.starts_with("https") || false {
                "https"
            } else {
                "http"
            };
            let url = if endpoint.path.is_empty() {
                format!("{}://{}:{}", scheme, endpoint.address, endpoint.port)
            } else {
                format!("{}://{}:{}{}", scheme, endpoint.address, endpoint.port, endpoint.path)
            };
            let card = AgentCard {
                name: batch.agent_name.clone(),
                url,
                version: batch.runtime_version.clone(),
                ..default_agent_card()
            };
            let reg = AgentRegistrationRequest {
                card,
                namespace: namespace_id.to_string(),
            };
            let _ = self.agent_registry.register(reg);
        }

        let mut response = AgentEndpointOperationResponse::new();
        response.response.request_id = request_id;
        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "AgentEndpointRegisterRpcRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "ai"
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Ai
    }

    fn resource_from_payload(&self, payload: &Payload) -> Option<(GrpcResource, PermissionAction)> {
        let request = AgentEndpointRegisterRpcRequest::from(payload);
        request.registration_batch.as_ref().map(|batch| (
                GrpcResource::ai(&batch.namespace_id, &batch.agent_name),
                PermissionAction::Write,
            ))
    }
}

/// Handler for AgentEndpointDeregisterRpcRequest — deregister agent endpoints.
///
/// Mirrors Nacos `AgentEndpointDeregisterRpcRequestHandler`. Lists all versions
/// for the agent, then removes all registered endpoints for each version via
/// `AiEndpointService.delete_agent_endpoint()`.
#[derive(Clone)]
pub struct AgentEndpointDeregisterRpcHandler {
    /// In-memory A2A agent registry used as a fallback target.
    pub agent_registry: Arc<AgentRegistry>,
    /// Optional config-backed A2A agent service.
    pub a2a_service: Option<Arc<dyn A2aAgentService>>,
    /// Optional endpoint registration service.
    pub endpoint_service: Option<Arc<AiEndpointService>>,
}

#[tonic::async_trait]
impl PayloadHandler for AgentEndpointDeregisterRpcHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = AgentEndpointDeregisterRpcRequest::from(payload);
        let request_id = request.request_id();

        let namespace_id = if request.namespace_id.is_empty() {
            "public"
        } else {
            &request.namespace_id
        };

        debug!(
            namespace = %namespace_id,
            agent_name = %request.agent_name,
            protocol = %request.protocol,
            "Processing AgentEndpointDeregisterRpcRequest"
        );

        // List all versions and delete endpoints for each
        if let Some(ref svc) = self.a2a_service
            && let Ok(versions) = svc.list_versions(namespace_id, &request.agent_name).await
                && let Some(ref ep_svc) = self.endpoint_service {
                    for vd in &versions {
                        let endpoints = ep_svc.get_agent_endpoints(
                            namespace_id,
                            &request.agent_name,
                            &vd.version,
                        );
                        for ep in &endpoints {
                            ep_svc.delete_agent_endpoint(
                                namespace_id,
                                &request.agent_name,
                                &vd.version,
                                &ep.address,
                                ep.port,
                            );
                        }
                    }
                }

        // Also deregister from in-memory registry
        let _ = self
            .agent_registry
            .deregister(namespace_id, &request.agent_name);

        let mut response = AgentEndpointOperationResponse::new();
        response.response.request_id = request_id;
        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "AgentEndpointDeregisterRpcRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "ai"
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Ai
    }

    fn resource_from_payload(&self, payload: &Payload) -> Option<(GrpcResource, PermissionAction)> {
        let request = AgentEndpointDeregisterRpcRequest::from(payload);
        Some((
            GrpcResource::ai(&request.namespace_id, &request.agent_name),
            PermissionAction::Write,
        ))
    }
}

/// Simple MD5 hash for content digest (used for change detection).
fn md5_hash(data: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

// =============================================================================
// Helper functions
// =============================================================================

fn default_mcp_registration() -> McpServerRegistration {
    McpServerRegistration {
        name: String::new(),
        display_name: String::new(),
        description: String::new(),
        namespace: "default".to_string(),
        version: "1.0.0".to_string(),
        endpoint: String::new(),
        server_type: Default::default(),
        transport: Default::default(),
        capabilities: Default::default(),
        tools: vec![],
        resources: vec![],
        prompts: vec![],
        metadata: Default::default(),
        tags: vec![],
        auto_fetch_tools: false,
        health_check: None,
    }
}

fn default_agent_card() -> AgentCard {
    AgentCard {
        name: String::new(),
        display_name: String::new(),
        description: String::new(),
        version: "1.0.0".to_string(),
        url: String::new(),
        protocol_version: "1.0".to_string(),
        capabilities: Default::default(),
        skills: vec![],
        default_input_modes: vec![],
        default_output_modes: vec![],
        preferred_transport: None,
        provider: None,
        documentation_url: None,
        icon_url: None,
        supports_authenticated_extended_card: None,
        metadata: Default::default(),
        tags: vec![],
    }
}

// =============================================================================
// Prompt Handlers
// =============================================================================

/// Handler for QueryPromptRequest — queries prompt with version/label/MD5 support
#[derive(Clone)]
pub struct QueryPromptHandler {
    /// Prompt operation service used to resolve prompt queries.
    pub prompt_service: Arc<crate::service::prompt::PromptOperationService>,
}

#[tonic::async_trait]
impl PayloadHandler for QueryPromptHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        use batata_api::remote::model::{
            QueryPromptRequest, QueryPromptResponse, RequestTrait, ResponseTrait,
        };

        let request = QueryPromptRequest::from(payload);
        let request_id = request.request_id();

        let namespace_id = if request.namespace_id.is_empty() {
            "public"
        } else {
            &request.namespace_id
        };

        if request.prompt_key.is_empty() {
            let response = batata_core::error_response!(
                QueryPromptResponse,
                request_id,
                "promptKey is required"
            );
            return Ok(response.build_payload());
        }

        debug!(
            prompt_key = %request.prompt_key,
            namespace = %namespace_id,
            version = %request.version,
            label = %request.label,
            "Processing QueryPromptRequest"
        );

        let version = if request.version.is_empty() {
            None
        } else {
            Some(request.version.as_str())
        };
        let label = if request.label.is_empty() {
            None
        } else {
            Some(request.label.as_str())
        };
        let md5 = if request.md5.is_empty() {
            None
        } else {
            Some(request.md5.as_str())
        };

        match self
            .prompt_service
            .query_prompt(namespace_id, &request.prompt_key, version, label, md5)
            .await
        {
            Ok(Some(info)) => {
                let prompt = info.to_client_prompt();
                let prompt_json = serde_json::to_value(&prompt).unwrap_or_default();
                let mut response = QueryPromptResponse::with_prompt(prompt_json);
                response.response.request_id = request_id;
                Ok(response.build_payload())
            }
            Ok(None) => {
                // NOT_MODIFIED — client already has latest version
                let mut response = QueryPromptResponse::new();
                response.response.request_id = request_id;
                response.response.error_code = 304;
                response.response.message = "Not Modified".to_string();
                Ok(response.build_payload())
            }
            Err(e) => {
                warn!("QueryPromptRequest error: {}", e);
                let response =
                    batata_core::error_response!(QueryPromptResponse, request_id, &e.to_string());
                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "QueryPromptRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Read
    }

    fn sign_type(&self) -> &'static str {
        "ai"
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Ai
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_mcp_registry() -> Arc<McpServerRegistry> {
        Arc::new(McpServerRegistry::new())
    }

    fn test_agent_registry() -> Arc<AgentRegistry> {
        Arc::new(AgentRegistry::new())
    }

    #[test]
    fn test_mcp_server_endpoint_handler_can_handle() {
        let handler = McpServerEndpointHandler {
            mcp_registry: test_mcp_registry(),
            mcp_service: None,
            endpoint_service: None,
        };
        assert_eq!(handler.can_handle(), "McpServerEndpointRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Write);
    }

    #[test]
    fn test_query_mcp_server_handler_can_handle() {
        let handler = QueryMcpServerHandler {
            mcp_registry: test_mcp_registry(),
            mcp_service: None,
        };
        assert_eq!(handler.can_handle(), "QueryMcpServerRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Read);
    }

    #[test]
    fn test_release_mcp_server_handler_can_handle() {
        let handler = ReleaseMcpServerHandler {
            mcp_registry: test_mcp_registry(),
            mcp_service: None,
        };
        assert_eq!(handler.can_handle(), "ReleaseMcpServerRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Write);
    }

    #[test]
    fn test_agent_endpoint_handler_can_handle() {
        let handler = AgentEndpointHandler {
            agent_registry: test_agent_registry(),
            a2a_service: None,
            endpoint_service: None,
        };
        assert_eq!(handler.can_handle(), "AgentEndpointRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Write);
    }

    #[test]
    fn test_query_agent_card_handler_can_handle() {
        let handler = QueryAgentCardHandler {
            agent_registry: test_agent_registry(),
            a2a_service: None,
        };
        assert_eq!(handler.can_handle(), "QueryAgentCardRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Read);
    }

    #[test]
    fn test_release_agent_card_handler_can_handle() {
        let handler = ReleaseAgentCardHandler {
            agent_registry: test_agent_registry(),
            a2a_service: None,
        };
        assert_eq!(handler.can_handle(), "ReleaseAgentCardRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Write);
    }

    #[test]
    fn test_agent_search_rpc_handler_can_handle() {
        let handler = AgentSearchRpcHandler {
            agent_registry: test_agent_registry(),
            a2a_service: None,
        };
        assert_eq!(handler.can_handle(), "AgentSearchRpcRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Read);
    }

    #[test]
    fn test_agent_discovery_rpc_handler_can_handle() {
        let handler = AgentDiscoveryRpcHandler {
            agent_registry: test_agent_registry(),
            a2a_service: None,
            endpoint_service: None,
        };
        assert_eq!(handler.can_handle(), "AgentDiscoveryRpcRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Read);
    }

    #[test]
    fn test_agent_endpoint_register_rpc_handler_can_handle() {
        let handler = AgentEndpointRegisterRpcHandler {
            agent_registry: test_agent_registry(),
            endpoint_service: None,
        };
        assert_eq!(handler.can_handle(), "AgentEndpointRegisterRpcRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Write);
    }

    #[test]
    fn test_agent_endpoint_deregister_rpc_handler_can_handle() {
        let handler = AgentEndpointDeregisterRpcHandler {
            agent_registry: test_agent_registry(),
            a2a_service: None,
            endpoint_service: None,
        };
        assert_eq!(handler.can_handle(), "AgentEndpointDeregisterRpcRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Write);
    }
}
