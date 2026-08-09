// Raft gRPC service definitions generated from proto/raft.proto
// This module contains message types and service traits for Raft consensus.
//
// Defines RaftService (core RPCs) and RaftManagementService (cluster ops).
// Consul writes are routed through the core Raft group via the plugin
// handler mechanism (RaftPluginHandler), not a separate gRPC service.

// Include the build.rs generated code (from target/build/out/raft.rs)
mod raft_proto {
    tonic::include_proto!("raft");
}

pub use raft_proto::*;
