// Core business logic module
// This module contains the core functionality and business logic of the application

// Unified abstraction layer for multi-registry support (Nacos, Consul)
pub mod abstraction;

// Core data models and structures
pub mod model;

// Raft consensus protocol for strong consistency
pub mod raft;

// Core service implementations
// Note: Cluster-related modules (ServerMemberManager, health_check, distro, member_event,
// member_lookup, cluster_client, circuit_breaker) are now provided by batata_core crate
pub mod service {
    pub mod remote; // Remote connection and communication management
}
