//! Connection management abstraction
//!
//! Provides traits for managing long-lived connections (gRPC, WebSocket)
//! between server and clients. Domain crates use these to push notifications
//! (config changes, service changes) to connected clients.

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::FoundationError;

/// Connection manager — manages long-lived client connections
#[async_trait]
pub trait ConnectionManager: Send + Sync {
    /// Total number of active connections
    fn connection_count(&self) -> usize;

    /// Check if a connection exists
    fn has_connection(&self, connection_id: &str) -> bool;

    /// Get all connection IDs
    fn all_connection_ids(&self) -> Vec<String>;

    /// Get connection info by ID
    fn get_connection_info(&self, connection_id: &str) -> Option<ConnectionInfo>;

    /// Get all connections from a specific client IP
    fn connections_for_ip(&self, ip: &str) -> Vec<ConnectionInfo>;

    /// Push a message to a specific connection
    async fn push_message(
        &self,
        connection_id: &str,
        message: Bytes,
    ) -> Result<(), FoundationError>;

    /// Push a message to multiple connections
    async fn push_message_to_many(
        &self,
        connection_ids: &[String],
        message: Bytes,
    ) -> Result<PushResult, FoundationError>;

    /// Touch/refresh a connection (update last active time)
    fn touch_connection(&self, connection_id: &str);
}

/// Information about a client connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// Unique connection identifier
    pub connection_id: String,
    /// Client IP address
    pub client_ip: String,
    /// Client port
    pub client_port: u16,
    /// Connection creation timestamp
    pub connected_at: i64,
    /// Last active timestamp
    pub last_active_at: i64,
    /// Client metadata (SDK version, app name, etc.)
    pub metadata: std::collections::HashMap<String, String>,
}

/// Result of a batch push operation
#[derive(Debug, Clone)]
pub struct PushResult {
    pub success_count: usize,
    pub failure_count: usize,
    pub failed_connections: Vec<String>,
}

/// Listener for connection lifecycle events
#[async_trait]
pub trait ConnectionEventListener: Send + Sync {
    /// Called when a new connection is established
    async fn on_connected(&self, info: &ConnectionInfo);

    /// Called when a connection is closed
    async fn on_disconnected(&self, connection_id: &str);
}
