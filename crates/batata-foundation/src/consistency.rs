//! Consistency protocol abstraction
//!
//! Provides traits for both strong consistency (CP/Raft) and eventual
//! consistency (AP/Distro) protocols.
//!
//! Domain crates choose which protocol to use based on their semantics:
//! - Nacos persistent services → CP (Raft)
//! - Nacos ephemeral services → AP (Distro)
//! - Consul services → CP (Raft)
//! - Config data → CP (Raft) for both Nacos and Consul

use async_trait::async_trait;
use bytes::Bytes;

use crate::FoundationError;

/// Strong consistency protocol (CP) — Raft-based
///
/// All writes go through leader and are replicated to a quorum.
/// Reads can be served from leader (linearizable) or any node (stale).
#[async_trait]
pub trait CpProtocol: Send + Sync {
    /// Submit a write operation to the consensus group
    ///
    /// The `group` parameter allows multiplexing multiple state machines
    /// over the same Raft cluster (e.g., "naming", "config", "auth").
    async fn write(
        &self,
        group: &str,
        data: WriteRequest,
    ) -> Result<WriteResponse, FoundationError>;

    /// Read data from the state machine
    ///
    /// `linearizable`: if true, ensures read-after-write consistency
    /// by confirming leadership before reading.
    async fn read(
        &self,
        group: &str,
        data: ReadRequest,
        linearizable: bool,
    ) -> Result<ReadResponse, FoundationError>;
}

/// Eventual consistency protocol (AP) — Distro-based
///
/// Data is replicated to all nodes asynchronously. Each node is
/// responsible for a portion of the data (consistent hashing).
#[async_trait]
pub trait ApProtocol: Send + Sync {
    /// Put data to the responsible node
    async fn put(&self, key: &str, value: Bytes) -> Result<(), FoundationError>;

    /// Get data — may be from local node or forwarded to responsible node
    async fn get(&self, key: &str) -> Result<Option<Bytes>, FoundationError>;

    /// Remove data
    async fn remove(&self, key: &str) -> Result<(), FoundationError>;

    /// Get all keys this node is responsible for
    async fn responsible_keys(&self) -> Result<Vec<String>, FoundationError>;

    /// Verify data consistency with a peer
    async fn verify(&self, peer: &str, keys: &[String]) -> Result<VerifyResult, FoundationError>;

    /// Sync data from a peer (pull)
    async fn sync_from(&self, peer: &str, keys: &[String]) -> Result<SyncResult, FoundationError>;

    /// Get a full snapshot for initial sync
    async fn snapshot(&self) -> Result<Bytes, FoundationError>;
}

/// Write request for CP protocol
#[derive(Debug, Clone)]
pub struct WriteRequest {
    /// Operation type
    pub operation: String,
    /// Serialized data
    pub data: Bytes,
}

/// Write response from CP protocol
#[derive(Debug, Clone)]
pub struct WriteResponse {
    /// Whether the write was successful
    pub success: bool,
    /// Optional response data
    pub data: Option<Bytes>,
}

/// Read request for CP protocol
#[derive(Debug, Clone)]
pub struct ReadRequest {
    /// Operation/query type
    pub operation: String,
    /// Serialized query parameters
    pub data: Bytes,
}

/// Read response from CP protocol
#[derive(Debug, Clone)]
pub struct ReadResponse {
    /// Serialized response data
    pub data: Option<Bytes>,
}

/// Result of data verification between peers
#[derive(Debug, Clone)]
pub struct VerifyResult {
    /// Keys that are consistent
    pub consistent: Vec<String>,
    /// Keys that are inconsistent (need sync)
    pub inconsistent: Vec<String>,
    /// Keys missing on the peer
    pub missing_on_peer: Vec<String>,
    /// Keys missing locally
    pub missing_locally: Vec<String>,
}

/// Result of data sync from a peer
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// Number of items synced
    pub synced: u64,
    /// Number of items that failed to sync
    pub failed: u64,
}

/// State machine processor — handles applied writes and reads
///
/// Domain crates implement this to process their specific operations
/// after they are committed by the consistency protocol.
#[async_trait]
pub trait StateMachineProcessor: Send + Sync {
    /// The group name this processor handles (e.g., "naming", "config")
    fn group(&self) -> &str;

    /// Process a committed write operation
    async fn on_apply(&self, data: &[u8]) -> Result<Option<Bytes>, FoundationError>;

    /// Process a read query
    async fn on_read(&self, data: &[u8]) -> Result<Option<Bytes>, FoundationError>;

    /// Create a snapshot of the current state
    async fn on_snapshot(&self) -> Result<Bytes, FoundationError>;

    /// Restore state from a snapshot
    async fn on_restore(&self, data: &[u8]) -> Result<(), FoundationError>;
}
