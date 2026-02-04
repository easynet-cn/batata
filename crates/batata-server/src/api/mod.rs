//! API module organization
//!
//! This module contains all API-related components for HTTP and gRPC interfaces.

// Configuration management API
pub mod config {
    pub mod model;
}

// Consul-compatible API implementation
pub mod consul;

// gRPC service definitions - re-exported from batata-api crate
// (generated from proto/nacos_grpc_service.proto)
pub use batata_api::grpc;

// Common API models and utilities
pub mod model;

// Naming/Service discovery API
pub mod naming {
    pub mod model;
}

// Remote communication API models
pub mod remote {
    pub mod model;
}

// OpenAPI documentation
pub mod openapi;

// Raft consensus gRPC service definitions - re-exported from batata-api crate
pub use batata_api::raft;

// Distro protocol API - re-exported from batata-api crate
pub use batata_api::distro;

// Nacos V2 Open API implementation
// Note: V1 API is NOT supported. Batata follows Nacos 3.x direction
// which focuses on V2 and V3 APIs for modern clients.
pub mod v2;

// AI Capabilities API (MCP Server Registry, A2A Communication)
pub mod ai;

// Cloud Native Integration API (Kubernetes Sync, Prometheus SD)
pub mod cloud;
