//! Consul KV store traits

use async_trait::async_trait;

use crate::error::ConsulKvError;
use crate::model::{KVPair, KVQueryMeta, KVQueryParams, Session, SessionRequest, TxnOp, TxnResult};

/// Consul KV Store — flat key-value storage
///
/// Provides Consul-native KV operations with blocking queries,
/// CAS, sessions, and transactions.
#[async_trait]
pub trait ConsulKvStore: Send + Sync {
    // ========================================================================
    // Basic CRUD
    // ========================================================================

    /// Get a KV pair by key
    async fn get(
        &self,
        key: &str,
        params: &KVQueryParams,
    ) -> Result<(Option<KVPair>, KVQueryMeta), ConsulKvError>;

    /// Get all keys with a prefix
    async fn list(
        &self,
        prefix: &str,
        params: &KVQueryParams,
    ) -> Result<(Vec<KVPair>, KVQueryMeta), ConsulKvError>;

    /// Get only key names with a prefix (no values)
    async fn keys(
        &self,
        prefix: &str,
        separator: Option<&str>,
    ) -> Result<(Vec<String>, KVQueryMeta), ConsulKvError>;

    /// Put a KV pair
    ///
    /// If `params.cas` is set, performs Compare-And-Swap.
    /// If `params.acquire` is set, acquires a lock.
    /// If `params.release` is set, releases a lock.
    ///
    /// Returns true if the operation succeeded (CAS may return false).
    async fn put(
        &self,
        key: &str,
        value: &[u8],
        params: &KVQueryParams,
    ) -> Result<bool, ConsulKvError>;

    /// Delete a key
    ///
    /// If `params.recurse` is true, deletes all keys with the given prefix.
    /// If `params.cas` is set, performs conditional delete.
    async fn delete(&self, key: &str, params: &KVQueryParams) -> Result<bool, ConsulKvError>;

    // ========================================================================
    // Transactions
    // ========================================================================

    /// Execute a transaction (atomic multi-key operation)
    async fn txn(&self, ops: Vec<TxnOp>) -> Result<TxnResult, ConsulKvError>;
}

/// Consul Session Service — distributed locking primitives
///
/// Sessions provide the foundation for distributed locks and
/// leader election in Consul.
#[async_trait]
pub trait ConsulSessionService: Send + Sync {
    /// Create a new session
    async fn create_session(&self, request: SessionRequest) -> Result<String, ConsulKvError>;

    /// Destroy (invalidate) a session
    ///
    /// Depending on session behavior, locked keys are either released or deleted.
    async fn destroy_session(&self, session_id: &str) -> Result<(), ConsulKvError>;

    /// Get a session by ID
    async fn get_session(&self, session_id: &str) -> Result<Option<Session>, ConsulKvError>;

    /// List all sessions
    async fn list_sessions(&self) -> Result<Vec<Session>, ConsulKvError>;

    /// List sessions for a specific node
    async fn list_node_sessions(&self, node: &str) -> Result<Vec<Session>, ConsulKvError>;

    /// Renew a session's TTL
    async fn renew_session(&self, session_id: &str) -> Result<Session, ConsulKvError>;
}

/// Consul Lock Service — high-level distributed locking
///
/// Built on top of KV store and sessions.
#[async_trait]
pub trait ConsulLockService: Send + Sync {
    /// Acquire a lock on a key
    ///
    /// Creates a session if needed and acquires the lock.
    /// Returns the session ID if successful, None if the lock is held.
    async fn acquire(
        &self,
        key: &str,
        value: &[u8],
        session_id: &str,
    ) -> Result<bool, ConsulKvError>;

    /// Release a lock on a key
    async fn release(&self, key: &str, session_id: &str) -> Result<bool, ConsulKvError>;

    /// Check who holds a lock
    async fn lock_holder(&self, key: &str) -> Result<Option<String>, ConsulKvError>;
}
