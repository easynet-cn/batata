//! Persistence traits for the unified storage abstraction layer
//!
//! This module defines the core persistence traits that abstract over different
//! storage backends: external database (MySQL/PostgreSQL), standalone embedded
//! (RocksDB), and distributed embedded (Raft + RocksDB).

pub mod auth;
pub mod capacity;
pub mod config;
pub mod namespace;

pub use auth::AuthPersistence;
pub use capacity::CapacityPersistence;
pub use config::ConfigPersistence;
pub use namespace::NamespacePersistence;

use async_trait::async_trait;

use crate::model::StorageMode;

/// Unified persistence service trait
///
/// This is the main interface for all storage operations. Implementations
/// dispatch to the appropriate storage backend based on the configured mode.
#[async_trait]
pub trait PersistenceService:
    ConfigPersistence + NamespacePersistence + AuthPersistence + CapacityPersistence + Send + Sync
{
    /// Get the current storage mode
    fn storage_mode(&self) -> StorageMode;

    /// Health check for the storage backend
    async fn health_check(&self) -> anyhow::Result<()>;
}
