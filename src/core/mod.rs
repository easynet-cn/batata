// Core business logic module
// This module contains the core functionality and business logic of the application

// Core data models and structures
pub mod model;

// Raft consensus protocol for strong consistency
pub mod raft;

// Core service implementations
pub mod service {
    pub mod cluster;        // Cluster management and coordination
    pub mod cluster_client; // Cluster gRPC client for inter-node communication
    pub mod distro;         // Distro protocol for ephemeral data sync
    pub mod health_check;   // Cluster member health check
    pub mod member_event;   // Member change event handling
    pub mod member_lookup;  // Member discovery strategies
    pub mod remote;         // Remote connection and communication management
}
