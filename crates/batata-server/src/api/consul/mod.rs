//! Module `api::consul` of the `batata-server` crate.
// Consul-compatible API implementation
// Re-exports from batata_plugin_consul crate with local extensions

// Re-export the plugin and all types from batata_plugin_consul
/// Re-exported item.
pub use batata_plugin_consul::ConsulPlugin;
/// Re-exported item.
pub use batata_plugin_consul::acl;
/// Re-exported item.
pub use batata_plugin_consul::agent;
/// Re-exported item.
pub use batata_plugin_consul::catalog;
/// Re-exported item.
pub use batata_plugin_consul::event;
/// Re-exported item.
pub use batata_plugin_consul::health;
/// Re-exported item.
pub use batata_plugin_consul::model;
/// Re-exported item.
pub use batata_plugin_consul::query;
/// Re-exported item.
pub use batata_plugin_consul::session;
/// Re-exported item.
pub use batata_plugin_consul::status;

// Local KV module with export/import handlers that need AppState
/// `kv` module.
pub mod kv;

// Route module that combines plugin routes with local export/import routes
/// `route` module.
pub mod route;

// Re-export service types
/// Re-exported item.
pub use batata_plugin_consul::AclService;
/// Re-exported item.
pub use batata_plugin_consul::ConsulAgentService;
/// Re-exported item.
pub use batata_plugin_consul::ConsulCatalogService;
/// Re-exported item.
pub use batata_plugin_consul::ConsulEventService;
/// Re-exported item.
pub use batata_plugin_consul::ConsulHealthService;
/// Re-exported item.
pub use batata_plugin_consul::ConsulIndexProvider;
/// Re-exported item.
pub use batata_plugin_consul::ConsulPeeringService;
/// Re-exported item.
pub use batata_plugin_consul::ConsulQueryService;
/// Re-exported item.
pub use batata_plugin_consul::ConsulSessionService;
/// Re-exported item.
pub use batata_plugin_consul::kv::ConsulKVService;
/// Re-exported item.
pub use batata_plugin_consul::model::*;
