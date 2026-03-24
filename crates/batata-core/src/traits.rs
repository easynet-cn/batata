//! Trait abstractions for core services
//!
//! These traits live in batata-core (rather than batata-common) because they
//! depend on types from batata-api (e.g., `Payload`).

use batata_api::grpc::Payload;
use batata_common::ConnectionInfo;

/// Client connection manager trait
///
/// Abstracts gRPC client connection management operations.
/// This allows services to push messages and query connections
/// without depending on the concrete `ConnectionManager` type.
///
/// Note: `register()` and `unregister()` are intentionally excluded from this
/// trait because they manage `GrpcClient` lifecycle (including channel senders)
/// and are only needed by the gRPC bidirectional stream handler.
#[async_trait::async_trait]
pub trait ClientConnectionManager: Send + Sync {
    /// Get the current number of connected clients
    fn connection_count(&self) -> usize;

    /// Check if a connection exists
    fn has_connection(&self, connection_id: &str) -> bool;

    /// Get all connection IDs
    fn get_all_connection_ids(&self) -> Vec<String>;

    /// Get connection info for a specific connection
    fn get_connection_info(&self, connection_id: &str) -> Option<ConnectionInfo>;

    /// Get all connection infos
    fn get_all_connection_infos(&self) -> Vec<ConnectionInfo>;

    /// Count connections from a given IP
    fn connections_for_ip(&self, ip: &str) -> usize;

    /// Push a gRPC payload to a specific connection
    async fn push_message(&self, connection_id: &str, payload: Payload) -> bool;

    /// Push a gRPC payload to multiple connections
    async fn push_message_to_many(
        &self,
        connection_ids: &[String],
        payload: Payload,
    ) -> usize;

    /// Send a connection reset request to a client
    async fn load_single(&self, connection_id: &str, redirect_address: Option<&str>) -> bool;

    /// Eject excess connections to reach target count
    async fn load_count(&self, target_count: usize, redirect_address: Option<&str>) -> usize;

    /// Update the last active timestamp for a connection
    fn touch_connection(&self, connection_id: &str);
}

