// AI Endpoint Service - NamingService integration for MCP/A2A endpoints
// Manages service instance registration in the NamingService for AI endpoints

use std::sync::Arc;

use tracing::{debug, info};

use batata_api::naming::NamingServiceProvider;
use batata_api::naming::model::Instance;

use super::constants::*;
use super::publication_gate::{
    AgentRuntimePublicationCapacityGate, DEFAULT_MAX_PUBLICATIONS_PER_CLIENT, PublicationOverLimit,
};

/// AI Endpoint Service for managing NamingService integrations
pub struct AiEndpointService {
    naming_service: Arc<dyn NamingServiceProvider>,
    /// Caps how many Runtime Endpoint entries one publisher Client may own.
    publication_gate: AgentRuntimePublicationCapacityGate,
}

impl AiEndpointService {
    /// Creates a new `AiEndpointService` backed by the given naming service,
    /// using [`DEFAULT_MAX_PUBLICATIONS_PER_CLIENT`].
    pub fn new(naming_service: Arc<dyn NamingServiceProvider>) -> Self {
        Self::with_publication_limit(naming_service, DEFAULT_MAX_PUBLICATIONS_PER_CLIENT)
    }

    /// Creates a new `AiEndpointService` with an explicit per-client
    /// publication cap.
    pub fn with_publication_limit(
        naming_service: Arc<dyn NamingServiceProvider>,
        max_publications_per_client: usize,
    ) -> Self {
        Self {
            naming_service,
            publication_gate: AgentRuntimePublicationCapacityGate::new(max_publications_per_client),
        }
    }

    /// The publication capacity gate backing this service.
    pub fn publication_gate(&self) -> &AgentRuntimePublicationCapacityGate {
        &self.publication_gate
    }

    /// Replace one publisher Client's complete Agent Endpoint batch.
    ///
    /// Admitted through the capacity gate, so a Client that already owns the
    /// configured maximum is rejected instead of growing without bound. A blank
    /// `client_id` is admitted without accounting.
    pub fn register_agent_endpoint_batch(
        &self,
        client_id: &str,
        namespace: &str,
        name: &str,
        version: &str,
        endpoints: &[(String, u16)],
    ) -> Result<(), PublicationOverLimit> {
        let key = AgentRuntimePublicationCapacityGate::publication_key(namespace, name, version);
        self.publication_gate
            .register_logical(client_id, &key, endpoints.len(), || {
                for (address, port) in endpoints {
                    self.create_agent_endpoint(namespace, name, version, address, *port);
                }
            })
    }

    /// Remove one Agent version's endpoints and release its reservation.
    ///
    /// The reservation survives a failed removal, so a Client stays charged for
    /// what it still owns.
    pub fn deregister_agent_endpoint_batch(
        &self,
        client_id: &str,
        namespace: &str,
        name: &str,
        version: &str,
    ) {
        let key = AgentRuntimePublicationCapacityGate::publication_key(namespace, name, version);
        self.publication_gate
            .deregister_logical(client_id, &key, || {
                let endpoints = self.get_agent_endpoints(namespace, name, version);
                for ep in &endpoints {
                    self.delete_agent_endpoint(namespace, name, version, &ep.address, ep.port);
                }
            });
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

        self.naming_service.register_instance(
            namespace,
            MCP_ENDPOINT_GROUP,
            &service_name,
            instance,
        );

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

        // Zero-copy snapshot — metadata is cloned once per entry into response.
        let instances = self.naming_service.get_instances_snapshot(
            namespace,
            MCP_ENDPOINT_GROUP,
            &service_name,
            "",    // all clusters
            false, // include unhealthy
        );

        instances
            .iter()
            .map(|inst| EndpointInfo {
                address: inst.ip.clone(),
                port: inst.port as u16,
                healthy: inst.healthy,
                metadata: inst.metadata.clone(),
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

        self.naming_service.deregister_instance(
            namespace,
            MCP_ENDPOINT_GROUP,
            &service_name,
            &instance,
        );

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

        self.naming_service.register_instance(
            namespace,
            AGENT_ENDPOINT_GROUP,
            &service_name,
            instance,
        );

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

        // Zero-copy snapshot.
        let instances = self.naming_service.get_instances_snapshot(
            namespace,
            AGENT_ENDPOINT_GROUP,
            &service_name,
            "",
            false,
        );

        instances
            .iter()
            .map(|inst| EndpointInfo {
                address: inst.ip.clone(),
                port: inst.port as u16,
                healthy: inst.healthy,
                metadata: inst.metadata.clone(),
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

        self.naming_service.deregister_instance(
            namespace,
            AGENT_ENDPOINT_GROUP,
            &service_name,
            &instance,
        );

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
    /// Endpoint IP address.
    pub address: String,
    /// Endpoint port.
    pub port: u16,
    /// Whether the endpoint is currently healthy.
    pub healthy: bool,
    /// Arbitrary metadata attached to the endpoint.
    pub metadata: std::collections::HashMap<String, String>,
}
