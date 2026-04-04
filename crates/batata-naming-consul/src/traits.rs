//! Consul naming service traits
//!
//! These traits define the Consul-specific service discovery contract.
//! They follow Consul's Agent/Catalog/Health API separation.
//! NO Nacos types or concepts.

use async_trait::async_trait;

use crate::error::ConsulNamingError;
use crate::model::{
    AgentService, AgentServiceRegistration, BlockingQuery, CatalogNode, CatalogService,
    CheckDefinition, CheckStatus, ConsulServiceQuery, HealthCheck, ServiceEntry,
};

/// Consul Agent Service — local service management
///
/// Manages services registered on the local agent.
/// This is the primary registration endpoint for Consul clients.
#[async_trait]
pub trait ConsulAgentService: Send + Sync {
    /// Register a service on the local agent
    ///
    /// Handles:
    /// - Service registration with health checks
    /// - Automatic check registration
    /// - Anti-entropy sync to catalog
    async fn register_service(
        &self,
        registration: AgentServiceRegistration,
    ) -> Result<(), ConsulNamingError>;

    /// Deregister a service by service ID
    async fn deregister_service(&self, service_id: &str) -> Result<(), ConsulNamingError>;

    /// Get all services on this agent
    async fn list_services(
        &self,
    ) -> Result<std::collections::HashMap<String, AgentService>, ConsulNamingError>;

    /// Get a specific service by ID
    async fn get_service(
        &self,
        service_id: &str,
    ) -> Result<Option<AgentService>, ConsulNamingError>;

    /// Enable maintenance mode for a service
    async fn enable_maintenance(
        &self,
        service_id: &str,
        reason: &str,
    ) -> Result<(), ConsulNamingError>;

    /// Disable maintenance mode for a service
    async fn disable_maintenance(&self, service_id: &str) -> Result<(), ConsulNamingError>;
}

/// Consul Health Service — health check management
///
/// Manages health checks and provides health-aware service queries.
#[async_trait]
pub trait ConsulHealthService: Send + Sync {
    // ========================================================================
    // Check Registration
    // ========================================================================

    /// Register a health check
    async fn register_check(&self, check: CheckDefinition) -> Result<(), ConsulNamingError>;

    /// Deregister a health check
    async fn deregister_check(&self, check_id: &str) -> Result<(), ConsulNamingError>;

    // ========================================================================
    // TTL Check Updates
    // ========================================================================

    /// Set check to passing (TTL)
    async fn pass_check(&self, check_id: &str, note: &str) -> Result<(), ConsulNamingError>;

    /// Set check to warning (TTL)
    async fn warn_check(&self, check_id: &str, note: &str) -> Result<(), ConsulNamingError>;

    /// Set check to critical (TTL)
    async fn fail_check(&self, check_id: &str, note: &str) -> Result<(), ConsulNamingError>;

    /// Update check output (TTL)
    async fn update_check(
        &self,
        check_id: &str,
        status: CheckStatus,
        output: &str,
    ) -> Result<(), ConsulNamingError>;

    // ========================================================================
    // Health Queries
    // ========================================================================

    /// Get all checks on the agent
    async fn list_checks(
        &self,
    ) -> Result<std::collections::HashMap<String, HealthCheck>, ConsulNamingError>;

    /// Get checks for a specific service
    async fn get_service_checks(
        &self,
        service_id: &str,
    ) -> Result<Vec<HealthCheck>, ConsulNamingError>;

    /// Get healthy service instances (passing checks only)
    async fn get_healthy_service_instances(
        &self,
        service: &str,
        query: &ConsulServiceQuery,
    ) -> Result<(Vec<ServiceEntry>, QueryMeta), ConsulNamingError>;

    /// Get all service instances with health info
    async fn get_service_health(
        &self,
        service: &str,
        query: &ConsulServiceQuery,
    ) -> Result<(Vec<ServiceEntry>, QueryMeta), ConsulNamingError>;

    /// Get health checks in a specific state
    async fn get_checks_by_state(
        &self,
        state: CheckStatus,
    ) -> Result<Vec<HealthCheck>, ConsulNamingError>;
}

/// Consul Catalog Service — cluster-wide service directory
///
/// Provides a cluster-wide view of all services across all nodes.
/// Data is synced via Raft consensus.
#[async_trait]
pub trait ConsulCatalogService: Send + Sync {
    /// List all services in the catalog
    async fn list_services(
        &self,
        query: &BlockingQuery,
    ) -> Result<(std::collections::HashMap<String, Vec<String>>, QueryMeta), ConsulNamingError>;

    /// Get all nodes providing a specific service
    async fn get_service(
        &self,
        service: &str,
        query: &ConsulServiceQuery,
    ) -> Result<(Vec<CatalogService>, QueryMeta), ConsulNamingError>;

    /// List all nodes in the catalog
    async fn list_nodes(
        &self,
        query: &BlockingQuery,
    ) -> Result<(Vec<CatalogNode>, QueryMeta), ConsulNamingError>;

    /// Get a specific node with its services
    async fn get_node(
        &self,
        node: &str,
        query: &BlockingQuery,
    ) -> Result<Option<NodeServices>, ConsulNamingError>;
}

/// Node with its registered services
#[derive(Debug, Clone)]
pub struct NodeServices {
    pub node: CatalogNode,
    pub services: std::collections::HashMap<String, AgentService>,
}

/// Metadata returned with blocking query responses
#[derive(Debug, Clone, Default)]
pub struct QueryMeta {
    /// Current Raft index (for subsequent blocking queries)
    pub last_index: u64,
    /// Whether the result was served from cache
    pub known_leader: bool,
    /// Duration the request blocked
    pub request_time_ms: u64,
}
