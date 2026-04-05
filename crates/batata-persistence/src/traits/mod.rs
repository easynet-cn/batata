//! Persistence traits for the unified storage abstraction layer
//!
//! This module defines the core persistence traits that abstract over different
//! storage backends: external database (MySQL/PostgreSQL), standalone embedded
//! (RocksDB), and distributed embedded (Raft + RocksDB).

pub mod ai_resource;
pub mod auth;
pub mod capacity;
pub mod config;
pub mod namespace;

pub use ai_resource::AiResourcePersistence;
pub use auth::AuthPersistence;
pub use capacity::CapacityPersistence;
pub use config::ConfigPersistence;
pub use namespace::NamespacePersistence;

use async_trait::async_trait;

use crate::model::{DeployTopology, StorageBackend, StorageMode};

/// Unified persistence service trait
///
/// This is the main interface for all storage operations. Implementations
/// dispatch to the appropriate storage backend based on the configured mode.
#[async_trait]
pub trait PersistenceService:
    ConfigPersistence
    + NamespacePersistence
    + AuthPersistence
    + CapacityPersistence
    + AiResourcePersistence
    + Send
    + Sync
{
    /// Get the storage backend type
    fn storage_backend(&self) -> StorageBackend;

    /// Get the deploy topology
    fn deploy_topology(&self) -> DeployTopology;

    /// Get the storage mode (derived from backend + topology)
    fn storage_mode(&self) -> StorageMode {
        StorageMode::from_dimensions(self.storage_backend(), self.deploy_topology())
    }

    /// Health check for the storage backend
    async fn health_check(&self) -> anyhow::Result<()>;

    /// Invalidate a config entry from the read cache (if any).
    /// Called by cluster sync handlers when a remote node notifies of a config change,
    /// ensuring Follower nodes don't serve stale cached data until TTL expiry.
    /// Default implementation is a no-op (embedded/SQL backends have no read cache).
    fn invalidate_config_read_cache(&self, _data_id: &str, _group: &str, _tenant: &str) {}
}
