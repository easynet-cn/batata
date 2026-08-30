// Core services for cluster and connection management

/// Circuit breaker for cluster node fault tolerance.
pub mod circuit_breaker;
/// Cluster module re-exports.
pub mod cluster;
/// Client for inter-node cluster communication.
pub mod cluster_client;
pub mod config_subscriber;
pub mod datacenter;
pub mod distro;
pub mod grpc_auth;
/// Cluster health checking services.
pub mod health_check;
/// Distributed locking services and handlers.
pub mod lock;
/// Cluster member change event handling.
pub mod member_event;
/// Lookup helpers for cluster members.
pub mod member_lookup;
/// Remote connection management services.
pub mod remote;

// Re-export commonly used types
pub use cluster::ServerMemberManager;
pub use config_subscriber::{ConfigKey, ConfigSubscriber, ConfigSubscriberManager};
pub use datacenter::{DatacenterConfig, DatacenterManager, DatacenterStatistics};
pub use grpc_auth::{
    GrpcAuthContext, GrpcAuthRoleProvider, GrpcAuthService, GrpcPermissionInfo, GrpcResource,
    GrpcRoleInfo, PermissionAction, PermissionCheckResult, ResourceType, extract_auth_context,
};
