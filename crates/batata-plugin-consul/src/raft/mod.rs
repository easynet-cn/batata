/// Consul-dedicated Raft consensus module.
///
/// Provides an independent Raft group for Consul operations, separate
/// from the Nacos Raft. This gives Consul its own Raft log index space,
/// matching the original Consul's architecture where all indexes
/// (CreateIndex, ModifyIndex, X-Consul-Index) derive from the Raft log.
///
/// ## Architecture
///
/// ```text
/// Nacos Raft (existing)           Consul Raft (this module)
/// ├── Log:  data/raft/logs/       ├── Log:  {consul_dir}/raft/logs/
/// ├── State: data/raft/state/     ├── State: {consul_dir}/raft/state/
/// └── gRPC: port 9849             └── gRPC: port 9850
/// ```
pub mod http_handler;
pub mod log_store;
pub mod network;
pub mod node;
pub mod request;
pub mod state_machine;
pub mod types;

pub use node::ConsulRaftNode;
pub use request::{ConsulRaftRequest, ConsulRaftResponse};
pub use types::NodeId;
