// Raft gRPC service definitions generated from proto/raft.proto
// This module contains message types and service traits for Raft consensus.
//
// Includes both Nacos Raft (RaftService) and Consul Raft (ConsulRaftService)
// services that share the same proto message types but route to independent
// Raft state machines.

// Include the build.rs generated code (from target/build/out/raft.rs)
mod raft_proto {
    tonic::include_proto!("raft");
}

pub use raft_proto::*;
