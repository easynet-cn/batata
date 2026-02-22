//! Consul-compatible API plugin for Batata
//!
//! This crate provides Consul-compatible HTTP API endpoints that can be integrated
//! with the Batata service discovery and configuration platform.
//!
//! ## Modules
//! - `acl`: Access Control List management
//! - `agent`: Service agent operations (register, deregister)
//! - `catalog`: Service catalog queries
//! - `config_entry`: Config entries management (service-defaults, proxy-defaults, etc.)
//! - `event`: User event operations
//! - `health`: Health check operations
//! - `kv`: Key-Value store operations
//! - `lock`: Distributed lock and semaphore operations
//! - `model`: Data models for Consul API
//! - `operator`: Cluster operator endpoints (Raft, Autopilot, Keyring)
//! - `query`: Prepared query operations
//! - `route`: Route configuration for actix-web
//! - `session`: Distributed session/lock management
//! - `snapshot`: Snapshot save/restore operations
//! - `status`: Cluster status information
//! - `connect`: Service mesh (discovery chain, exported/imported services)
//! - `coordinate`: Network coordinate/RTT endpoints
//! - `peering`: Cluster peering for cross-datacenter service discovery

pub mod acl;
pub mod agent;
pub mod catalog;
pub mod config_entry;
pub mod connect;
pub mod connect_ca;
pub mod coordinate;
pub mod event;
pub mod health;
pub mod health_actor;
pub mod health_executor;
pub mod kv;
pub mod lock;
pub mod model;
pub mod operator;
pub mod peering;
pub mod query;
pub mod route;
pub mod session;
pub mod snapshot;
pub mod status;

// Re-export route functions for easy integration
pub use route::{
    // Individual route scopes (in-memory)
    consul_acl_routes,
    consul_agent_routes,
    consul_agent_routes_persistent,
    // Individual route scopes (real cluster)
    consul_agent_routes_real,
    consul_catalog_routes,
    consul_config_entry_routes,
    consul_config_entry_routes_persistent,
    consul_connect_ca_routes,
    consul_connect_ca_routes_persistent,
    consul_connect_routes,
    consul_connect_routes_persistent,
    consul_coordinate_routes,
    consul_coordinate_routes_persistent,
    consul_event_routes,
    consul_event_routes_persistent,
    consul_health_routes,
    consul_health_routes_persistent,
    consul_kv_routes,
    consul_lock_routes,
    consul_operator_routes,
    consul_operator_routes_real,
    consul_peering_routes,
    consul_peering_routes_persistent,
    consul_query_routes,
    consul_ui_routes,
    // Combined route scopes
    consul_routes,            // In-memory (for development/testing)
    consul_routes_full,       // Persistent + real cluster (recommended for production)
    consul_routes_persistent, // Persistent only
    consul_session_routes,
    consul_snapshot_routes,
    consul_snapshot_routes_persistent,
    consul_status_routes,
    consul_status_routes_real,
};

// Re-export key services
pub use acl::AclService;
pub use agent::ConsulAgentService;
pub use catalog::ConsulCatalogService;
pub use config_entry::{ConsulConfigEntryService, ConsulConfigEntryServicePersistent};
pub use event::{ConsulEventService, ConsulEventServicePersistent};
pub use health::{ConsulHealthService, ConsulHealthServicePersistent};
pub use health_actor::{HealthActorHandle, create_health_actor, CheckConfig, HealthStatus};
pub use health_executor::HealthCheckExecutor;
pub use kv::ConsulKVService;
pub use lock::{ConsulLockService, ConsulSemaphoreService};
pub use operator::{ConsulOperatorService, ConsulOperatorServiceReal};
pub use query::ConsulQueryService;
pub use session::ConsulSessionService;
pub use snapshot::{ConsulSnapshotService, ConsulSnapshotServicePersistent};

// Tier 2 services
pub use connect::ConsulConnectService;
pub use connect_ca::ConsulConnectCAService;
pub use coordinate::{ConsulCoordinateService, ConsulCoordinateServicePersistent};
pub use peering::ConsulPeeringService;
