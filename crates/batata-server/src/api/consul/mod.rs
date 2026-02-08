// Consul-compatible API implementation
// Re-exports from batata_plugin_consul crate with local extensions

// Re-export all types from batata_plugin_consul
pub use batata_plugin_consul::acl;
pub use batata_plugin_consul::agent;
pub use batata_plugin_consul::catalog;
pub use batata_plugin_consul::event;
pub use batata_plugin_consul::health;
pub use batata_plugin_consul::lock;
pub use batata_plugin_consul::model;
pub use batata_plugin_consul::query;
pub use batata_plugin_consul::session;
pub use batata_plugin_consul::status;

// Local KV module with export/import handlers that need AppState
pub mod kv;

// Route module that combines plugin routes with local export/import routes
pub mod route;

// Re-export service types
pub use batata_plugin_consul::AclService;
pub use batata_plugin_consul::ConsulAgentService;
pub use batata_plugin_consul::ConsulCatalogService;
pub use batata_plugin_consul::ConsulEventService;
pub use batata_plugin_consul::ConsulHealthService;
pub use batata_plugin_consul::ConsulLockService;
pub use batata_plugin_consul::ConsulQueryService;
pub use batata_plugin_consul::ConsulSemaphoreService;
pub use batata_plugin_consul::ConsulSessionService;
pub use batata_plugin_consul::kv::ConsulKVService;
pub use batata_plugin_consul::model::*;
