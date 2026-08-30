/// Consul Raft integration module.
///
/// Provides the plugin handler and request types for routing Consul
/// write operations through the unified core Raft group via `PluginWrite`.
pub mod apply_hook;
/// The `plugin_handler` module.
pub mod plugin_handler;
/// The `request` module.
pub mod request;

pub use apply_hook::{ConsulApplyHook, SharedConsulApplyHook, new_shared_hook};
pub use plugin_handler::{CONSUL_PLUGIN_ID, ConsulRaftPluginHandler, ConsulRaftWriter};
pub use request::{ConsulRaftRequest, ConsulRaftResponse};
