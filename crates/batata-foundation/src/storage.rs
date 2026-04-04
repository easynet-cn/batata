//! Storage engine abstraction
//!
//! Provides a generic key-value storage interface that can be backed by
//! RocksDB, an embedded database, or an external database (MySQL/PostgreSQL).
//!
//! Domain crates define their own key schemas and serialization logic on top
//! of this raw byte-level interface.

use async_trait::async_trait;
use bytes::Bytes;

use crate::FoundationError;

/// Raw key-value storage engine
///
/// This is the lowest-level storage abstraction. Domain crates build
/// higher-level typed stores on top of this.
///
/// Implementations:
/// - `RocksDbEngine` — for Raft log and local state
/// - `MemoryEngine` — for embedded/testing mode
/// - `SqlEngine` — for MySQL/PostgreSQL persistence
#[async_trait]
pub trait StorageEngine: Send + Sync {
    /// Get a value by key
    async fn get(&self, key: &[u8]) -> Result<Option<Bytes>, FoundationError>;

    /// Put a key-value pair
    async fn put(&self, key: &[u8], value: &[u8]) -> Result<(), FoundationError>;

    /// Delete a key
    async fn delete(&self, key: &[u8]) -> Result<(), FoundationError>;

    /// Check if a key exists
    async fn exists(&self, key: &[u8]) -> Result<bool, FoundationError> {
        Ok(self.get(key).await?.is_some())
    }

    /// Scan keys with a prefix, returning key-value pairs
    async fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Bytes, Bytes)>, FoundationError>;

    /// Scan keys within a range [start, end)
    async fn scan_range(
        &self,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<(Bytes, Bytes)>, FoundationError>;

    /// Delete all keys with a given prefix
    async fn delete_prefix(&self, prefix: &[u8]) -> Result<u64, FoundationError>;

    /// Atomically apply a batch of operations
    async fn write_batch(&self, ops: Vec<BatchOp>) -> Result<(), FoundationError>;
}

/// A single operation in a batch write
#[derive(Debug, Clone)]
pub enum BatchOp {
    Put { key: Bytes, value: Bytes },
    Delete { key: Bytes },
}

/// Snapshot capability for storage engines that support it
///
/// Used by consistency protocols (Raft) for snapshot-based replication.
#[async_trait]
pub trait SnapshotStorage: StorageEngine {
    /// Create a snapshot, returning the snapshot data
    async fn create_snapshot(&self) -> Result<Bytes, FoundationError>;

    /// Restore from a snapshot
    async fn restore_snapshot(&self, data: &[u8]) -> Result<(), FoundationError>;
}

/// Storage backend metadata
pub trait StorageInfo: Send + Sync {
    /// Storage backend name (e.g., "rocksdb", "mysql", "memory")
    fn backend_name(&self) -> &str;

    /// Whether this storage is persistent across restarts
    fn is_persistent(&self) -> bool;

    /// Whether this storage supports transactions
    fn supports_transactions(&self) -> bool;
}
