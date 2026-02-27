// AI Endpoint Service - NamingService integration for MCP/A2A endpoints
// Manages service instance registration in the NamingService for AI endpoints

use std::sync::Arc;

use tracing::{debug, info};

use batata_api::naming::model::Instance;

use super::constants::*;
use crate::service::naming::NamingService;

/// AI Endpoint Service for managing NamingService integrations
pub struct AiEndpointService {
    naming_service: Arc<NamingService>,
}

impl AiEndpointService {
    pub fn new(naming_service: Arc<NamingService>) -> Self {
        Self { naming_service }
    }

    /// Register an MCP server endpoint in NamingService
    pub fn create_mcp_endpoint(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
        address: &str,
        port: u16,
    ) {
        let service_name = mcp_service_name(name, version);

        let mut metadata = std::collections::HashMap::new();
        metadata.insert(METADATA_AI_MCP_SERVICE.to_string(), "true".to_string());
        metadata.insert("version".to_string(), version.to_string());

        let instance = Instance {
            ip: address.to_string(),
            port: port as i32,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: service_name.clone(),
            metadata,
            ..Default::default()
        };

        self.naming_service
            .register_instance(namespace, MCP_ENDPOINT_GROUP, &service_name, instance);

        info!(
            name = %name,
            version = %version,
            address = %address,
            port = %port,
            "MCP endpoint registered in NamingService"
        );
    }

    /// Get MCP server endpoints from NamingService
    pub fn get_mcp_endpoints(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> Vec<EndpointInfo> {
        let service_name = mcp_service_name(name, version);

        let instances = self.naming_service.get_instances(
            namespace,
            MCP_ENDPOINT_GROUP,
            &service_name,
            "",   // all clusters
            false, // include unhealthy
        );

        instances
            .into_iter()
            .map(|inst| EndpointInfo {
                address: inst.ip,
                port: inst.port as u16,
                healthy: inst.healthy,
                metadata: inst.metadata,
            })
            .collect()
    }

    /// Deregister an MCP server endpoint from NamingService
    pub fn delete_mcp_endpoint(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
        address: &str,
        port: u16,
    ) {
        let service_name = mcp_service_name(name, version);

        let instance = Instance {
            ip: address.to_string(),
            port: port as i32,
            cluster_name: "DEFAULT".to_string(),
            service_name: service_name.clone(),
            ..Default::default()
        };

        self.naming_service
            .deregister_instance(namespace, MCP_ENDPOINT_GROUP, &service_name, &instance);

        debug!(
            name = %name,
            version = %version,
            "MCP endpoint deregistered from NamingService"
        );
    }

    /// Register an A2A agent endpoint in NamingService
    pub fn create_agent_endpoint(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
        address: &str,
        port: u16,
    ) {
        let service_name = a2a_service_name(name, version);

        let mut metadata = std::collections::HashMap::new();
        metadata.insert(METADATA_AI_A2A_SERVICE.to_string(), "true".to_string());
        metadata.insert("version".to_string(), version.to_string());

        let instance = Instance {
            ip: address.to_string(),
            port: port as i32,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: service_name.clone(),
            metadata,
            ..Default::default()
        };

        self.naming_service
            .register_instance(namespace, AGENT_ENDPOINT_GROUP, &service_name, instance);

        info!(
            name = %name,
            version = %version,
            address = %address,
            port = %port,
            "A2A endpoint registered in NamingService"
        );
    }

    /// Get A2A agent endpoints from NamingService
    pub fn get_agent_endpoints(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> Vec<EndpointInfo> {
        let service_name = a2a_service_name(name, version);

        let instances = self.naming_service.get_instances(
            namespace,
            AGENT_ENDPOINT_GROUP,
            &service_name,
            "",
            false,
        );

        instances
            .into_iter()
            .map(|inst| EndpointInfo {
                address: inst.ip,
                port: inst.port as u16,
                healthy: inst.healthy,
                metadata: inst.metadata,
            })
            .collect()
    }

    /// Deregister an A2A agent endpoint from NamingService
    pub fn delete_agent_endpoint(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
        address: &str,
        port: u16,
    ) {
        let service_name = a2a_service_name(name, version);

        let instance = Instance {
            ip: address.to_string(),
            port: port as i32,
            cluster_name: "DEFAULT".to_string(),
            service_name: service_name.clone(),
            ..Default::default()
        };

        self.naming_service
            .deregister_instance(namespace, AGENT_ENDPOINT_GROUP, &service_name, &instance);

        debug!(
            name = %name,
            version = %version,
            "A2A endpoint deregistered from NamingService"
        );
    }
}

/// Endpoint info returned from queries
#[derive(Debug, Clone)]
pub struct EndpointInfo {
    pub address: String,
    pub port: u16,
    pub healthy: bool,
    pub metadata: std::collections::HashMap<String, String>,
}
