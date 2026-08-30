//! API module organization
//!
//! This module contains all API-related components for HTTP and gRPC interfaces.

// Configuration management API
/// `config` module.
pub mod config {
        /// `model` module.
pub mod model;
}

// Consul-compatible API implementation
#[cfg(feature = "consul")]
/// `consul` module.
pub mod consul;

// gRPC service definitions - re-exported from batata-api crate
// (generated from proto/nacos_grpc_service.proto)
/// Re-exported item.
pub use batata_api::grpc;

// Common API models and utilities
/// `model` module.
pub mod model;

// Naming/Service discovery API
/// `naming` module.
pub mod naming {
        /// `model` module.
pub mod model;
}

// Remote communication API models
/// `remote` module.
pub mod remote {
        /// `model` module.
pub mod model;
}

// Raft consensus gRPC service definitions - re-exported from batata-api crate
/// Re-exported item.
pub use batata_api::raft;

// Distro protocol API - re-exported from batata-api crate
/// Re-exported item.
pub use batata_api::distro;

// Shared logic between V2 and V3 API implementations
/// `shared` module.
pub mod shared;

// Batata V2 Open API implementation (Nacos V2-compatible)
// Note: V1 API is NOT supported. Batata follows Nacos 3.x direction
// which focuses on V2 and V3 APIs for modern clients.
/// `v2` module.
pub mod v2;

// Batata V3 Admin and Client API implementation
/// `v3` module.
pub mod v3;

// AI Capabilities API (MCP Server Registry, A2A Communication)
/// `ai` module.
pub mod ai;

// Cloud Native Integration API (Kubernetes Sync, Prometheus SD)
/// `cloud` module.
pub mod cloud;

// Prometheus metrics endpoint using metrics-exporter-prometheus
/// `metrics` module.
pub mod metrics;
