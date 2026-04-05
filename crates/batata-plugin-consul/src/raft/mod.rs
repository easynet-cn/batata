/// Consul Raft integration module.
///
/// Provides the plugin handler and request types for routing Consul
/// write operations through the unified core Raft group via `PluginWrite`.
pub mod plugin_handler;
pub mod request;

pub use plugin_handler::{CONSUL_PLUGIN_ID, ConsulRaftPluginHandler, ConsulRaftWriter};
pub use request::{ConsulRaftRequest, ConsulRaftResponse};
