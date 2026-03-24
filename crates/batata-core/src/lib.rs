//! Batata Core - Cluster and connection management
//!
//! This crate provides:
//! - Server member management
//! - Connection management
//! - Health checking
//! - Task scheduling
//! - Abstraction layer for multi-registry support

pub mod abstraction;
pub mod handler;
pub mod model;
pub mod service;
pub mod traits;

// Re-export ClientConnectionManager trait
pub use traits::ClientConnectionManager;

/// Re-exports from batata-api for handler macro compatibility.
/// Handler macros use `$crate::api::grpc::Payload` etc.
pub mod api {
    pub use batata_api::distro;
    pub use batata_api::grpc;
    pub use batata_api::remote;
}

// Re-export cluster module
pub mod cluster {
    pub use crate::service::cluster::{ServerMemberManager, ServerMemberManagerConfig};
    pub use batata_common::ClusterHealthSummary;
}

// Re-export commonly used types
pub use model::{
    AtomicLastActive, Configuration, Connection, ConnectionMeta, GrpcClient, PageParam,
};
pub use service::remote::ConnectionEventListener;
pub use service::{ConfigKey, ConfigSubscriber, ConfigSubscriberManager};

// Re-export gRPC auth types
pub use service::{
    GrpcAuthContext, GrpcAuthRoleProvider, GrpcAuthService, GrpcPermissionInfo, GrpcResource,
    GrpcRoleInfo, PermissionAction, PermissionCheckResult, ResourceType, extract_auth_context,
};

// Re-export common functions
pub use batata_common::local_ip;
