// AI module gRPC handlers for MCP and A2A
// Implements handlers for MCP server and A2A agent management via gRPC

use std::sync::Arc;

use tonic::Status;
use tracing::{debug, warn};

use batata_core::model::Connection;

use crate::{
    api::{
        ai::model::{AgentCard, AgentRegistrationRequest, McpServerRegistration},
        ai::{AgentRegistry, McpServerRegistry},
        grpc::Payload,
        remote::model::{
            AgentEndpointRequest, AgentEndpointResponse, McpServerEndpointRequest,
            McpServerEndpointResponse, QueryAgentCardRequest, QueryAgentCardResponse,
            QueryMcpServerRequest, QueryMcpServerResponse, ReleaseAgentCardRequest,
            ReleaseAgentCardResponse, ReleaseMcpServerRequest, ReleaseMcpServerResponse,
            RequestTrait, ResponseTrait,
        },
    },
    service::rpc::{AuthRequirement, PayloadHandler},
};

// =============================================================================
// MCP Server Handlers
// =============================================================================

/// Handler for McpServerEndpointRequest - register/deregister MCP server endpoint
#[derive(Clone)]
pub struct McpServerEndpointHandler {
    pub mcp_registry: Arc<McpServerRegistry>,
}

#[tonic::async_trait]
impl PayloadHandler for McpServerEndpointHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = McpServerEndpointRequest::from(payload);
        let request_id = request.request_id();
        let operation = request.operation_type.to_lowercase();

        debug!(
            operation = %operation,
            namespace = %request.namespace_id,
            name = %request.mcp_name,
            "Processing MCP server endpoint request"
        );

        match operation.as_str() {
            "register" => {
                let endpoint = format!("{}:{}", request.address, request.port);
                let registration = McpServerRegistration {
                    name: request.mcp_name.clone(),
                    namespace: request.namespace_id.clone(),
                    version: request.version.clone(),
                    endpoint,
                    ..default_mcp_registration()
                };

                match self.mcp_registry.register(registration) {
                    Ok(_server) => {
                        let mut response = McpServerEndpointResponse::new();
                        response.response.request_id = request_id;
                        response.operation_type = "register".to_string();
                        Ok(response.build_payload())
                    }
                    Err(e) => {
                        let response =
                            crate::error_response!(McpServerEndpointResponse, request_id, e);
                        Ok(response.build_payload())
                    }
                }
            }
            "deregister" => {
                match self
                    .mcp_registry
                    .deregister(&request.namespace_id, &request.mcp_name)
                {
                    Ok(()) => {
                        let mut response = McpServerEndpointResponse::new();
                        response.response.request_id = request_id;
                        response.operation_type = "deregister".to_string();
                        Ok(response.build_payload())
                    }
                    Err(e) => {
                        let response =
                            crate::error_response!(McpServerEndpointResponse, request_id, e);
                        Ok(response.build_payload())
                    }
                }
            }
            _ => {
                warn!(operation = %operation, "Unknown MCP endpoint operation type");
                let response = crate::error_response!(
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
        AuthRequirement::Authenticated
    }
}

/// Handler for QueryMcpServerRequest - query MCP server details
#[derive(Clone)]
pub struct QueryMcpServerHandler {
    pub mcp_registry: Arc<McpServerRegistry>,
}

#[tonic::async_trait]
impl PayloadHandler for QueryMcpServerHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = QueryMcpServerRequest::from(payload);
        let request_id = request.request_id();

        debug!(
            namespace = %request.namespace_id,
            name = %request.mcp_name,
            "Querying MCP server"
        );

        match self
            .mcp_registry
            .get(&request.namespace_id, &request.mcp_name)
        {
            Some(server) => {
                let detail = serde_json::to_value(&server).unwrap_or_default();
                let mut response = QueryMcpServerResponse::new();
                response.response.request_id = request_id;
                response.detail = detail;
                Ok(response.build_payload())
            }
            None => {
                let response = crate::error_response!(
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
        AuthRequirement::Authenticated
    }
}

/// Handler for ReleaseMcpServerRequest - publish/release an MCP server
#[derive(Clone)]
pub struct ReleaseMcpServerHandler {
    pub mcp_registry: Arc<McpServerRegistry>,
}

#[tonic::async_trait]
impl PayloadHandler for ReleaseMcpServerHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
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

        match self.mcp_registry.register(registration) {
            Ok(server) => {
                let mut response = ReleaseMcpServerResponse::new();
                response.response.request_id = request_id;
                response.mcp_id = server.id;
                Ok(response.build_payload())
            }
            Err(e) => {
                // If already exists, try update
                let mut registration2: McpServerRegistration =
                    serde_json::from_value(request.server_specification).unwrap_or_else(|_| {
                        McpServerRegistration {
                            name: request.mcp_name.clone(),
                            namespace: request.namespace_id.clone(),
                            ..default_mcp_registration()
                        }
                    });
                registration2.name = request.mcp_name.clone();
                registration2.namespace = request.namespace_id.clone();

                match self.mcp_registry.update(
                    &request.namespace_id,
                    &request.mcp_name,
                    registration2,
                ) {
                    Ok(server) => {
                        let mut response = ReleaseMcpServerResponse::new();
                        response.response.request_id = request_id;
                        response.mcp_id = server.id;
                        Ok(response.build_payload())
                    }
                    Err(e2) => {
                        let response = crate::error_response!(
                            ReleaseMcpServerResponse,
                            request_id,
                            format!("Failed to release MCP server: {}, {}", e, e2)
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
        AuthRequirement::Authenticated
    }
}

// =============================================================================
// A2A Agent Handlers
// =============================================================================

/// Handler for AgentEndpointRequest - register/deregister agent endpoint
#[derive(Clone)]
pub struct AgentEndpointHandler {
    pub agent_registry: Arc<AgentRegistry>,
}

#[tonic::async_trait]
impl PayloadHandler for AgentEndpointHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = AgentEndpointRequest::from(payload);
        let request_id = request.request_id();
        let operation = request.operation_type.to_lowercase();

        debug!(
            operation = %operation,
            namespace = %request.namespace_id,
            name = %request.agent_name,
            "Processing agent endpoint request"
        );

        match operation.as_str() {
            "register" => {
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

                let card = AgentCard {
                    name: request.agent_name.clone(),
                    endpoint: endpoint_url,
                    version,
                    ..default_agent_card()
                };

                let reg_request = AgentRegistrationRequest {
                    card,
                    namespace: request.namespace_id.clone(),
                };

                match self.agent_registry.register(reg_request) {
                    Ok(_agent) => {
                        let mut response = AgentEndpointResponse::new();
                        response.response.request_id = request_id;
                        response.operation_type = "register".to_string();
                        Ok(response.build_payload())
                    }
                    Err(e) => {
                        let response = crate::error_response!(AgentEndpointResponse, request_id, e);
                        Ok(response.build_payload())
                    }
                }
            }
            "deregister" => {
                match self
                    .agent_registry
                    .deregister(&request.namespace_id, &request.agent_name)
                {
                    Ok(()) => {
                        let mut response = AgentEndpointResponse::new();
                        response.response.request_id = request_id;
                        response.operation_type = "deregister".to_string();
                        Ok(response.build_payload())
                    }
                    Err(e) => {
                        let response = crate::error_response!(AgentEndpointResponse, request_id, e);
                        Ok(response.build_payload())
                    }
                }
            }
            _ => {
                warn!(operation = %operation, "Unknown agent endpoint operation type");
                let response = crate::error_response!(
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
        AuthRequirement::Authenticated
    }
}

/// Handler for QueryAgentCardRequest - query agent card details
#[derive(Clone)]
pub struct QueryAgentCardHandler {
    pub agent_registry: Arc<AgentRegistry>,
}

#[tonic::async_trait]
impl PayloadHandler for QueryAgentCardHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = QueryAgentCardRequest::from(payload);
        let request_id = request.request_id();

        debug!(
            namespace = %request.namespace_id,
            name = %request.agent_name,
            "Querying agent card"
        );

        match self
            .agent_registry
            .get(&request.namespace_id, &request.agent_name)
        {
            Some(agent) => {
                let detail = serde_json::to_value(&agent).unwrap_or_default();
                let mut response = QueryAgentCardResponse::new();
                response.response.request_id = request_id;
                response.detail = detail;
                Ok(response.build_payload())
            }
            None => {
                let response = crate::error_response!(
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
        AuthRequirement::Authenticated
    }
}

/// Handler for ReleaseAgentCardRequest - publish/release an agent card
#[derive(Clone)]
pub struct ReleaseAgentCardHandler {
    pub agent_registry: Arc<AgentRegistry>,
}

#[tonic::async_trait]
impl PayloadHandler for ReleaseAgentCardHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
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
                // If already exists, try update
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
                        let response = crate::error_response!(
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
        AuthRequirement::Authenticated
    }
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
        endpoint: String::new(),
        protocol_version: "1.0".to_string(),
        capabilities: Default::default(),
        skills: vec![],
        input_modes: vec![],
        output_modes: vec![],
        authentication: None,
        rate_limits: None,
        metadata: Default::default(),
        tags: vec![],
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
        };
        assert_eq!(handler.can_handle(), "McpServerEndpointRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Authenticated);
    }

    #[test]
    fn test_query_mcp_server_handler_can_handle() {
        let handler = QueryMcpServerHandler {
            mcp_registry: test_mcp_registry(),
        };
        assert_eq!(handler.can_handle(), "QueryMcpServerRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Authenticated);
    }

    #[test]
    fn test_release_mcp_server_handler_can_handle() {
        let handler = ReleaseMcpServerHandler {
            mcp_registry: test_mcp_registry(),
        };
        assert_eq!(handler.can_handle(), "ReleaseMcpServerRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Authenticated);
    }

    #[test]
    fn test_agent_endpoint_handler_can_handle() {
        let handler = AgentEndpointHandler {
            agent_registry: test_agent_registry(),
        };
        assert_eq!(handler.can_handle(), "AgentEndpointRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Authenticated);
    }

    #[test]
    fn test_query_agent_card_handler_can_handle() {
        let handler = QueryAgentCardHandler {
            agent_registry: test_agent_registry(),
        };
        assert_eq!(handler.can_handle(), "QueryAgentCardRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Authenticated);
    }

    #[test]
    fn test_release_agent_card_handler_can_handle() {
        let handler = ReleaseAgentCardHandler {
            agent_registry: test_agent_registry(),
        };
        assert_eq!(handler.can_handle(), "ReleaseAgentCardRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Authenticated);
    }
}
