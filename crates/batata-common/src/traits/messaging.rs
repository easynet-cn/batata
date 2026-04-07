//! Messaging traits: PayloadHandler, PayloadHandlerRegistry, HeartbeatService,
//! GrpcConnectionManager and related types.

use std::sync::Arc;

/// Payload handler registry trait
///
/// Used to register and dispatch gRPC payload handlers.
pub trait PayloadHandlerRegistry: Send + Sync {
    /// Register a handler for a specific message type
    fn register(&self, handler: Arc<dyn PayloadHandler>);

    /// Get a handler by type name
    fn get_handler(&self, type_name: &str) -> Option<Arc<dyn PayloadHandler>>;
}

/// Payload handler trait for gRPC message handling
#[async_trait::async_trait]
pub trait PayloadHandler: Send + Sync {
    /// Get the type name this handler processes
    fn type_name(&self) -> &'static str;

    /// Handle the payload and return a response
    async fn handle(
        &self,
        request_id: &str,
        payload: &[u8],
        metadata: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<Vec<u8>>;
}

/// Heartbeat service trait for health check tracking
///
/// Abstracts heartbeat recording and removal operations,
/// allowing AppState to hold a trait object instead of `Arc<dyn Any>`.
pub trait HeartbeatService: Send + Sync {
    /// Record a heartbeat for an instance
    #[allow(clippy::too_many_arguments)]
    fn record_heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
        heartbeat_timeout: i64,
        ip_delete_timeout: i64,
    );

    /// Remove heartbeat tracking for an instance
    fn remove_heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    );
}

/// Trait for managing gRPC client connections.
///
/// Abstracts connection registration, lookup, and lifecycle management,
/// allowing naming handlers and other components to depend on the trait
/// rather than the concrete `ConnectionManager` type.
#[async_trait::async_trait]
pub trait GrpcConnectionManager: Send + Sync {
    /// Register a new gRPC client connection.
    /// Returns true if successfully registered, false if rejected.
    async fn register_connection(&self, connection_id: &str, client: GrpcClientInfo) -> bool;

    /// Unregister a gRPC client connection
    async fn unregister_connection(&self, connection_id: &str);

    /// Get a connection's client IP by connection ID
    fn get_client_ip(&self, connection_id: &str) -> Option<String>;

    /// Get all connection IDs
    fn get_all_connection_ids(&self) -> Vec<String>;

    /// Get the number of active connections
    fn connection_count(&self) -> usize;
}

/// Minimal gRPC client info for the trait boundary
#[derive(Debug, Clone)]
pub struct GrpcClientInfo {
    pub client_ip: String,
    pub client_port: u16,
    pub app_name: String,
    pub sdk: String,
    pub labels: std::collections::HashMap<String, String>,
}
