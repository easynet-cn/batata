// Consul-compatible API implementation
// Provides HTTP endpoints compatible with Consul Agent API

pub mod acl;
pub mod agent;
pub mod catalog;
pub mod health;
pub mod kv;
pub mod model;
pub mod route;

// Re-export service types (avoid glob to prevent name conflicts)
pub use acl::AclService;
pub use agent::ConsulAgentService;
pub use catalog::ConsulCatalogService;
pub use health::ConsulHealthService;
pub use kv::ConsulKVService;
pub use model::*;
pub use route::consul_routes;
