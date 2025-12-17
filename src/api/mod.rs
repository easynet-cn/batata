// API module organization
// This module contains all API-related components for HTTP and gRPC interfaces

// Configuration management API
pub mod config {
    pub mod model;
}

// gRPC service definitions and implementations
#[path = "grpc/_.rs"]
pub mod grpc;

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
