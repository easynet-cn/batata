//! Consul-compatible API plugin for Batata
//!
//! This crate provides Consul-compatible HTTP API endpoints that can be integrated
//! with the Batata service discovery and configuration platform.
//!
//! ## Modules
//! - `acl`: Access Control List management
//! - `agent`: Service agent operations (register, deregister)
//! - `catalog`: Service catalog queries
//! - `event`: User event operations
//! - `health`: Health check operations
//! - `kv`: Key-Value store operations
//! - `lock`: Distributed lock and semaphore operations
//! - `model`: Data models for Consul API
//! - `query`: Prepared query operations
//! - `route`: Route configuration for actix-web
//! - `session`: Distributed session/lock management
//! - `status`: Cluster status information

pub mod acl;
pub mod agent;
pub mod catalog;
pub mod event;
pub mod health;
pub mod kv;
pub mod lock;
pub mod model;
pub mod query;
pub mod route;
pub mod session;
pub mod status;

// Re-export route functions for easy integration
pub use route::{
    // Individual route scopes (in-memory)
    consul_acl_routes,
    // Individual route scopes (persistent)
    consul_acl_routes_persistent,
    consul_agent_routes,
    consul_agent_routes_persistent,
    // Individual route scopes (real cluster)
    consul_agent_routes_real,
    consul_catalog_routes,
    consul_event_routes,
    consul_event_routes_persistent,
    consul_health_routes,
    consul_health_routes_persistent,
    consul_kv_routes,
    consul_kv_routes_persistent,
    consul_query_routes,
    consul_query_routes_persistent,
    // Combined route scopes
    consul_routes,            // In-memory (for development/testing)
    consul_routes_full,       // Persistent + real cluster (recommended for production)
    consul_routes_persistent, // Persistent only
    consul_session_routes,
    consul_session_routes_persistent,
    consul_status_routes,
    consul_status_routes_real,
};

// Re-export key services
pub use acl::{AclService, AclServicePersistent};
pub use agent::ConsulAgentService;
pub use catalog::ConsulCatalogService;
pub use event::{ConsulEventService, ConsulEventServicePersistent};
pub use health::{ConsulHealthService, ConsulHealthServicePersistent};
pub use kv::{ConsulKVService, ConsulKVServicePersistent};
pub use lock::{ConsulLockService, ConsulSemaphoreService};
pub use query::{ConsulQueryService, ConsulQueryServicePersistent};
pub use session::{ConsulSessionService, ConsulSessionServicePersistent};
