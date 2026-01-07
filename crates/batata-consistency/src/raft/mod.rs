// Raft consensus module for Batata cluster
// Provides strong consistency for persistent data using the Raft protocol

pub mod config;
pub mod grpc_service;
pub mod log_store;
pub mod network;
pub mod node;
pub mod request;
pub mod state_machine;
pub mod types;

// Re-export commonly used types
pub use config::RaftConfig;
pub use grpc_service::{RaftGrpcService, RaftManagementGrpcService};
pub use node::{RaftNode, RaftNodeBuilder};
pub use request::{RaftRequest, RaftResponse};
pub use types::{NodeId, Raft, RaftMetrics, ServerState, TypeConfig, calculate_node_id};
