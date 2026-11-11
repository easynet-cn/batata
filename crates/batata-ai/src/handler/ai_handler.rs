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
        AgentDiscoveryResponse, AgentDiscoveryResult, AgentDiscoveryRpcRequest,
        AgentEndpointDeregisterRpcRequest, AgentEndpointInfo, AgentEndpointOperationResponse,
        AgentEndpointRegisterRpcRequest, AgentEndpointRequest, AgentEndpointResponse,
        AgentPublishRpcRequest, AgentPublishRpcResponse, AgentSearchPage, AgentSearchResponse,
        AgentSearchRpcRequest, AgentSubscribeRpcRequest, AgentSubscribeRpcResponse,
        AgentUnsubscribeRpcRequest, AgentUnsubscribeRpcResponse, EndpointSet,
        McpServerEndpointRequest, McpServerEndpointResponse, QueryAgentCardRequest,
        QueryAgentCardResponse, QueryMcpServerRequest, QueryMcpServerResponse,
        ReleaseAgentCardRequest, ReleaseAgentCardResponse, ReleaseMcpServerRequest,
        ReleaseMcpServerResponse, RequestTrait, ResponseTrait,
    },
};

use batata_core::handler::rpc::{AuthRequirement, PayloadHandler};

use crate::{
    model::{AgentCard, AgentProvider, AgentRegistrationRequest, McpServerRegistration},
    registry::a2a::AgentRegistry,
    registry::mcp::McpServerRegistry,
    service::{
        AiEndpointService,
        agent_rad_watch::AgentRadWatchService,
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
                .get_mcp_server_detail(
                    &request.namespace_id,
                    None,
                    Some(&request.mcp_name),
                    None,
                    // gRPC carries no end-user identity; visibility is not
                    // enforced on this internal path.
                    None,
                )
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
                // gRPC carries no end-user identity; visibility is not
                // enforced on this internal path.
                .get_agent_card(&request.namespace_id, &request.agent_name, None, None)
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
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = AgentSearchRpcRequest::from(payload);
        let request_id = request.request_id();

        // Extract inner search request (fallback to empty default)
        let search = request.search_request.unwrap_or_default();
        let namespace_id = if search.namespace_id.is_empty() {
            "public"
        } else {
            &search.namespace_id
        };
        let page_no = if search.page_no == 0 {
            1
        } else {
            search.page_no
        };
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
                // gRPC carries no end-user identity; visibility is not
                // enforced on this internal path.
                .list_agents(
                    namespace_id,
                    agent_name_filter,
                    "blur",
                    page_no,
                    page_size,
                    None,
                )
                .await
            {
                Ok(page) => {
                    let entries: Vec<AgentCatalogEntry> = page
                        .page_items
                        .iter()
                        .map(|v| {
                            let online = v
                                .version_info
                                .as_ref()
                                .and_then(|vi| vi.online_versions.as_ref());
                            let latest_version = online
                                .and_then(|ov| ov.first())
                                .map(|s| s.version.clone())
                                .or_else(|| {
                                    v.version_info
                                        .as_ref()
                                        .and_then(|vi| vi.editing_version.clone())
                                })
                                .unwrap_or_default();
                            let versions: Vec<AgentCatalogVersion> = online
                                .map(|ov| {
                                    ov.iter()
                                        .map(|s| AgentCatalogVersion {
                                            version: s.version.clone(),
                                            labels: s.labels.clone().unwrap_or_default(),
                                            protocols: s
                                                .protocols
                                                .clone()
                                                .unwrap_or_else(|| vec!["a2a".to_string()]),
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            AgentCatalogEntry {
                                agent_name: v.agent_name.clone(),
                                display_name: v.display_name.clone().unwrap_or_default(),
                                description: v.description.clone().unwrap_or_default(),
                                icon_url: v.icon_url.clone().unwrap_or_default(),
                                provider: None,
                                tags: v.tags.clone().unwrap_or_default(),
                                latest_version,
                                versions,
                            }
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
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
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
                && let Ok(versions) = svc.list_versions(namespace_id, &reference.agent_name).await
            {
                // If label is "latest" or empty, find the latest version
                let label = if reference.label.is_empty() {
                    "latest"
                } else {
                    &reference.label
                };
                // The published "latest" is the online version; fall back to the
                // highest version when no version has reached the online state.
                let resolve_latest = || {
                    versions
                        .page_items
                        .iter()
                        .find(|v| v.status == "online")
                        .or_else(|| versions.page_items.first())
                        .map(|v| v.version.clone())
                };
                if label == "latest" {
                    if let Some(latest) = resolve_latest() {
                        version = latest;
                    }
                } else {
                    // Try to match by label (not yet supported; fallback to latest)
                    if let Some(latest) = resolve_latest() {
                        version = latest;
                    }
                }
            }
            // Fall back to in-memory registry
            if version.is_empty()
                && let Some(agent) = self.agent_registry.get(namespace_id, &reference.agent_name)
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
            let endpoints =
                ep_svc.get_agent_endpoints(namespace_id, &reference.agent_name, &resolved_version);
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
        let digest_input = format!(
            "{}:{}:{}",
            reference.agent_name,
            resolved_version,
            endpoint_list.len()
        );
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
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
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
                format!(
                    "{}://{}:{}{}",
                    scheme, endpoint.address, endpoint.port, endpoint.path
                )
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
        request.registration_batch.as_ref().map(|batch| {
            (
                GrpcResource::ai(&batch.namespace_id, &batch.agent_name),
                PermissionAction::Write,
            )
        })
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
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
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
            && let Some(ref ep_svc) = self.endpoint_service
        {
            for vd in &versions.page_items {
                let endpoints =
                    ep_svc.get_agent_endpoints(namespace_id, &request.agent_name, &vd.version);
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
        ..Default::default()
    }
}

// =============================================================================
// AI-RAD: Agent Publish / Subscribe / Unsubscribe (Nacos 3.3.0 GA `radV1` / `radWatchV1`)
// =============================================================================

/// Handler for AgentPublishRpcRequest — code-first agent definition publication.
///
/// Mirrors GA `AgentPublishRequestHandler`. Builds an `AgentCard` from the wire
/// `AgentPublishRequest` and delegates to `A2aAgentService::publish_agent_from_code`,
/// returning the resulting `AgentVersionDetail`.
#[derive(Clone)]
pub struct AgentPublishRpcHandler {
    /// Optional config-backed A2A agent service.
    pub a2a_service: Option<Arc<dyn A2aAgentService>>,
}

#[tonic::async_trait]
impl PayloadHandler for AgentPublishRpcHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = AgentPublishRpcRequest::from(payload);
        let request_id = request.request_id();

        let publish = match &request.publish_request {
            Some(p) => p,
            None => {
                let response = batata_core::error_response!(
                    AgentPublishRpcResponse,
                    request_id,
                    "publishRequest is required"
                );
                return Ok(response.build_payload());
            }
        };

        let namespace_id = request
            .namespace_id
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "public".to_string());

        if publish.agent_name.is_empty() {
            let response = batata_core::error_response!(
                AgentPublishRpcResponse,
                request_id,
                "agentName is required"
            );
            return Ok(response.build_payload());
        }

        // Build an AgentCard from the GA wire request. The version detail
        // (including callInterfaces) is produced by the code-first publish
        // service, so the card only carries the declared metadata here.
        let card = AgentCard {
            name: publish.agent_name.clone(),
            display_name: publish.display_name.clone(),
            description: publish.description.clone(),
            version: publish
                .version
                .clone()
                .unwrap_or_else(|| "1.0.0".to_string()),
            icon_url: publish.icon_url.clone(),
            provider: publish.provider.as_ref().map(|p| AgentProvider {
                organization: p.name.clone(),
                url: p.url.clone(),
            }),
            tags: publish.tags.clone(),
            ..Default::default()
        };

        match &self.a2a_service {
            Some(svc) => {
                match svc
                    .publish_agent_from_code(&namespace_id, &card, publish.auto_submit)
                    .await
                {
                    Ok(detail) => {
                        let mut response = AgentPublishRpcResponse::new();
                        response.response.request_id = request_id;
                        response.version_detail = Some(detail);
                        Ok(response.build_payload())
                    }
                    Err(e) => {
                        let response = batata_core::error_response!(
                            AgentPublishRpcResponse,
                            request_id,
                            format!("Failed to publish agent: {}", e)
                        );
                        Ok(response.build_payload())
                    }
                }
            }
            None => {
                let response = batata_core::error_response!(
                    AgentPublishRpcResponse,
                    request_id,
                    "AI agent service is unavailable"
                );
                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "AgentPublishRpcRequest"
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
        let request = AgentPublishRpcRequest::from(payload);
        let namespace_id = request
            .namespace_id
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "public".to_string());
        let agent_name = request
            .publish_request
            .as_ref()
            .map(|p| p.agent_name.clone())
            .unwrap_or_default();
        Some((
            GrpcResource::ai(&namespace_id, &agent_name),
            PermissionAction::Write,
        ))
    }
}

/// Handler for AgentSubscribeRpcRequest — install a connection-owned RAD Watch.
///
/// Mirrors GA `AgentSubscribeRpcRequestHandler`. Delegates to
/// `AgentRadWatchService::subscribe`, returning the server-generated `watchKey`
/// and the observed discovery fingerprint.
#[derive(Clone)]
pub struct AgentSubscribeRpcHandler {
    /// RAD watch service that owns the installed watch intents.
    pub rad_watch: Arc<AgentRadWatchService>,
}

#[tonic::async_trait]
impl PayloadHandler for AgentSubscribeRpcHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = AgentSubscribeRpcRequest::from(payload);
        let request_id = request.request_id();
        let connection_id = connection.meta_info.connection_id.clone();
        let discovery = request.discovery_request.clone().unwrap_or_default();

        match self
            .rad_watch
            .subscribe(
                &connection_id,
                &request.client_watch_id,
                discovery,
                &request.materialized_fingerprint,
            )
            .await
        {
            Ok((watch_key, observed_fingerprint)) => {
                let mut response = AgentSubscribeRpcResponse::new();
                response.response.request_id = request_id;
                response.watch_key = Some(watch_key);
                response.observed_fingerprint = Some(observed_fingerprint);
                response.refresh_required = false;
                Ok(response.build_payload())
            }
            Err(e) => {
                let response =
                    batata_core::error_response!(AgentSubscribeRpcResponse, request_id, e);
                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "AgentSubscribeRpcRequest"
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
        let request = AgentSubscribeRpcRequest::from(payload);
        let namespace_id = request
            .discovery_request
            .as_ref()
            .and_then(|d| {
                if d.namespace_id.is_empty() {
                    None
                } else {
                    Some(d.namespace_id.clone())
                }
            })
            .unwrap_or_else(|| "public".to_string());
        let agent_name = request
            .discovery_request
            .as_ref()
            .map(|d| d.reference.agent_name.clone())
            .unwrap_or_default();
        Some((
            GrpcResource::ai(&namespace_id, &agent_name),
            PermissionAction::Read,
        ))
    }
}

/// Handler for AgentUnsubscribeRpcRequest — remove one connection-owned RAD Watch.
///
/// Mirrors GA `AgentUnsubscribeRpcRequestHandler`. Delegates to
/// `AgentRadWatchService::unsubscribe`.
#[derive(Clone)]
pub struct AgentUnsubscribeRpcHandler {
    /// RAD watch service that owns the installed watch intents.
    pub rad_watch: Arc<AgentRadWatchService>,
}

#[tonic::async_trait]
impl PayloadHandler for AgentUnsubscribeRpcHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = AgentUnsubscribeRpcRequest::from(payload);
        let request_id = request.request_id();
        let connection_id = connection.meta_info.connection_id.clone();

        self.rad_watch
            .unsubscribe(&connection_id, &request.watch_key);
        let mut response = AgentUnsubscribeRpcResponse::new();
        response.response.request_id = request_id;
        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "AgentUnsubscribeRpcRequest"
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

    fn resource_from_payload(
        &self,
        _payload: &Payload,
    ) -> Option<(GrpcResource, PermissionAction)> {
        // Unsubscribe operates on the caller's own watch_key; no agent-scoped
        // resource is resolved on the read path.
        None
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

    // --- AI-RAD: Agent Publish / Subscribe / Unsubscribe (GA `radV1` / `radWatchV1`) ---

    use crate::service::agent_rad_watch::{AgentRadWatchService, WatchPusher};
    use batata_api::remote::model::{
        AgentDiscoveryRequest, AgentPublishRequest, AgentReference, Request,
    };
    use batata_core::model::{AtomicLastActive, ConnectionMeta};
    use std::collections::HashMap;

    /// No-op `WatchPusher` used to construct a `AgentRadWatchService` in unit tests.
    struct TestPusher;

    #[async_trait::async_trait]
    impl WatchPusher for TestPusher {
        async fn push(&self, _connection_id: &str, _payload: batata_api::grpc::Payload) -> bool {
            true
        }
    }

    /// Test `A2aAgentService` double used to construct a `AgentRadWatchService`
    /// in unit/integration tests. Returns one online version so the discovery
    /// projector resolves an Available fingerprint, and a known version detail
    /// from `publish_agent_from_code`.
    struct RadTestA2a;

    #[async_trait::async_trait]
    impl crate::service::traits::A2aAgentService for RadTestA2a {
        async fn register_agent(
            &self,
            _card: &batata_common::model::ai::a2a::AgentCard,
            _namespace: &str,
            _registration_type: &str,
        ) -> anyhow::Result<String> {
            Ok(String::new())
        }
        async fn get_agent_card(
            &self,
            _namespace: &str,
            _agent_name: &str,
            _version: Option<&str>,
            _user: Option<&str>,
        ) -> anyhow::Result<Option<batata_common::model::ai::a2a::RegisteredAgent>> {
            Ok(None)
        }
        async fn update_agent_card(
            &self,
            _card: &batata_common::model::ai::a2a::AgentCard,
            _namespace: &str,
            _registration_type: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn delete_agent(
            &self,
            _namespace: &str,
            _agent_name: &str,
            _version: Option<&str>,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn list_agents(
            &self,
            _namespace: &str,
            _agent_name: Option<&str>,
            _search_type: &str,
            _page_no: u32,
            _page_size: u32,
            _user: Option<&str>,
        ) -> anyhow::Result<batata_common::model::Page<batata_common::model::ai::a2a::AgentSummary>>
        {
            Ok(Default::default())
        }
        async fn list_versions(
            &self,
            _namespace: &str,
            _agent_name: &str,
        ) -> anyhow::Result<
            batata_common::model::Page<batata_common::model::ai::a2a::AgentVersionSummary>,
        > {
            Ok(batata_common::model::Page {
                page_items: vec![batata_common::model::ai::a2a::AgentVersionSummary {
                    version: "1.0.0".to_string(),
                    status: "online".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            })
        }
        async fn find_by_skill(
            &self,
            _skill: &str,
        ) -> anyhow::Result<Vec<batata_common::model::ai::a2a::RegisteredAgent>> {
            Ok(Vec::new())
        }
        async fn batch_register(
            &self,
            _request: batata_common::model::ai::a2a::BatchAgentRegistrationRequest,
        ) -> anyhow::Result<batata_common::model::ai::a2a::BatchRegistrationResponse> {
            Ok(Default::default())
        }
        async fn stats(&self) -> anyhow::Result<batata_common::model::ai::a2a::AgentRegistryStats> {
            Ok(Default::default())
        }
        async fn create_agent_draft(
            &self,
            _namespace: &str,
            _card: &batata_common::model::ai::a2a::AgentCard,
            _overwrite: bool,
            _call_interfaces: Option<Vec<batata_common::model::ai::a2a::AgentCallInterface>>,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(Default::default())
        }
        async fn update_agent_draft(
            &self,
            _namespace: &str,
            _card: &batata_common::model::ai::a2a::AgentCard,
            _call_interfaces: Option<Vec<batata_common::model::ai::a2a::AgentCallInterface>>,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(Default::default())
        }
        async fn delete_agent_draft(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn submit_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(Default::default())
        }
        async fn publish_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(Default::default())
        }
        async fn force_publish_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(Default::default())
        }
        async fn redraft_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(Default::default())
        }
        async fn online_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(Default::default())
        }
        async fn offline_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(Default::default())
        }
        async fn update_agent_labels(
            &self,
            _namespace: &str,
            _name: &str,
            _labels: std::collections::HashMap<String, String>,
        ) -> anyhow::Result<std::collections::HashMap<String, String>> {
            Ok(std::collections::HashMap::new())
        }
        async fn update_agent_scope(
            &self,
            _namespace: &str,
            _name: &str,
            _new_scope: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_runtime_endpoints(
            &self,
            _namespace: &str,
            _agent_name: &str,
            _protocol: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::ConsoleRuntimeEndpointView> {
            Ok(Default::default())
        }
        async fn publish_agent_from_code(
            &self,
            _namespace: &str,
            _card: &batata_common::model::ai::a2a::AgentCard,
            _auto_submit: bool,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(batata_common::model::ai::a2a::AgentVersionDetail {
                namespace_id: "public".to_string(),
                name: "my-agent".to_string(),
                version: "1.0.0".to_string(),
                status: "online".to_string(),
                ..Default::default()
            })
        }
    }

    fn test_rad_watch() -> Arc<AgentRadWatchService> {
        Arc::new(AgentRadWatchService::with_pusher(
            Arc::new(RadTestA2a),
            Arc::new(TestPusher),
        ))
    }

    #[test]
    fn test_agent_publish_rpc_handler_can_handle() {
        let handler = AgentPublishRpcHandler { a2a_service: None };
        assert_eq!(handler.can_handle(), "AgentPublishRpcRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Write);
    }

    #[test]
    fn test_agent_subscribe_rpc_handler_can_handle() {
        let handler = AgentSubscribeRpcHandler {
            rad_watch: test_rad_watch(),
        };
        assert_eq!(handler.can_handle(), "AgentSubscribeRpcRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Read);
    }

    #[test]
    fn test_agent_unsubscribe_rpc_handler_can_handle() {
        let handler = AgentUnsubscribeRpcHandler {
            rad_watch: test_rad_watch(),
        };
        assert_eq!(handler.can_handle(), "AgentUnsubscribeRpcRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Read);
    }

    /// Build a minimal `Connection` with a known `connection_id` for handler tests.
    fn test_connection(connection_id: &str) -> Connection {
        let mut meta = ConnectionMeta::default();
        meta.connection_id = connection_id.to_string();
        Connection {
            traced: false,
            ability_table: HashMap::new(),
            meta_info: meta,
            last_active: AtomicLastActive::default(),
        }
    }

    #[tokio::test]
    async fn rad_publish_handler_returns_version_detail() {
        let handler = AgentPublishRpcHandler {
            a2a_service: Some(Arc::new(RadTestA2a)),
        };
        let publish_request = AgentPublishRequest {
            agent_name: "my-agent".to_string(),
            display_name: "My Agent".to_string(),
            description: "desc".to_string(),
            icon_url: None,
            provider: None,
            tags: vec![],
            extensions: None,
            version: Some("1.0.0".to_string()),
            call_interfaces: vec![],
            author: None,
            change_description: None,
            based_on_version: None,
            auto_submit: false,
        };
        let req = AgentPublishRpcRequest {
            request: Request {
                request_id: "r1".to_string(),
                ..Default::default()
            },
            module: "ai".to_string(),
            namespace_id: Some("public".to_string()),
            publish_request: Some(publish_request),
        };
        let payload = req.to_payload(None);
        let conn = test_connection("conn-pub");
        let resp_payload = handler.handle(&conn, &payload).await.unwrap();

        let body = resp_payload.body.as_ref().unwrap().value.clone();
        let resp: AgentPublishRpcResponse = serde_json::from_slice(&body).unwrap();
        let detail = resp.version_detail.expect("versionDetail must be present");
        assert_eq!(detail.version, "1.0.0");
        assert_eq!(detail.status, "online");
    }

    #[tokio::test]
    async fn rad_subscribe_handler_returns_watch_key_and_fingerprint() {
        let handler = AgentSubscribeRpcHandler {
            rad_watch: test_rad_watch(),
        };
        let req = AgentSubscribeRpcRequest {
            request: Request {
                request_id: "r2".to_string(),
                ..Default::default()
            },
            module: "ai".to_string(),
            client_watch_id: "w1".to_string(),
            discovery_request: Some(AgentDiscoveryRequest {
                namespace_id: "public".to_string(),
                reference: AgentReference {
                    agent_name: "my-agent".to_string(),
                    ..Default::default()
                },
                filter: None,
            }),
            materialized_fingerprint: String::new(),
        };
        let payload = req.to_payload(None);
        let conn = test_connection("conn-sub");
        let resp_payload = handler.handle(&conn, &payload).await.unwrap();

        let body = resp_payload.body.as_ref().unwrap().value.clone();
        let resp: AgentSubscribeRpcResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.watch_key.as_deref(), Some("conn-sub:w1"));
        let fp = resp.observed_fingerprint.unwrap_or_default();
        assert!(
            fp.starts_with("sha256-canonical-json-v1:"),
            "observed fingerprint is computed: {fp}"
        );
        assert!(!resp.refresh_required);
    }

    #[tokio::test]
    async fn rad_unsubscribe_handler_succeeds() {
        let handler = AgentUnsubscribeRpcHandler {
            rad_watch: test_rad_watch(),
        };
        let req = AgentUnsubscribeRpcRequest {
            request: Request {
                request_id: "r3".to_string(),
                ..Default::default()
            },
            module: "ai".to_string(),
            watch_key: "conn-sub:w1".to_string(),
        };
        let payload = req.to_payload(None);
        let conn = test_connection("conn-sub");
        let resp_payload = handler.handle(&conn, &payload).await.unwrap();

        let body = resp_payload.body.as_ref().unwrap().value.clone();
        let resp: AgentUnsubscribeRpcResponse = serde_json::from_slice(&body).unwrap();
        assert!(resp.response.success, "unsubscribe reports success");
    }
}
