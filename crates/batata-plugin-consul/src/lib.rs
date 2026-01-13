//! Consul-compatible API plugin for Batata
//!
//! This crate provides Consul-compatible HTTP API endpoints that can be integrated
//! with the Batata service discovery and configuration platform.
//!
//! ## Modules
//! - `acl`: Access Control List management
//! - `agent`: Service agent operations (register, deregister)
//! - `catalog`: Service catalog queries
//! - `health`: Health check operations
//! - `kv`: Key-Value store operations
//! - `model`: Data models for Consul API
//! - `route`: Route configuration for actix-web

pub mod acl;
pub mod agent;
pub mod catalog;
pub mod health;
pub mod kv;
pub mod model;
pub mod route;

// Re-export route functions for easy integration
pub use route::{
    consul_acl_routes, consul_agent_routes, consul_catalog_routes, consul_health_routes,
    consul_kv_routes, consul_routes,
};

// Re-export key services
pub use acl::AclService;
pub use agent::ConsulAgentService;
pub use catalog::ConsulCatalogService;
pub use health::ConsulHealthService;
