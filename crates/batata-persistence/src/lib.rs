//! Batata Persistence - Database entities and persistence layer
//!
//! This crate provides:
//! - SeaORM entity definitions (auto-generated)
//! - Persistence trait abstractions for unified storage
//! - Domain model types for persistence operations

#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]
#![allow(clippy::empty_line_after_doc_comments)]

/// Bincode (de)serialization helpers re-exported from `batata_common`.
pub mod bincode {
    pub use batata_common::bincode::{deserialize, serialize};
}
/// Distributed (Raft cluster) persistence backend.
pub mod distributed;
/// Standalone embedded (RocksDB) persistence backend.
pub mod embedded;
/// `SeaORM` entity definitions.
pub mod entity;
/// Storage-agnostic domain model types returned by the persistence traits.
pub mod model;
/// External database (MySQL/PostgreSQL via SeaORM) persistence backend.
pub mod search_util;
pub mod sql;
/// Persistence trait abstractions for the unified storage layer.
pub mod traits;

// Re-export sea-orm for convenience
pub use sea_orm;

// Re-export entity prelude
pub use entity::prelude::*;

// Re-export persistence traits
pub use traits::{
    AiResourcePersistence, AuthPersistence, CapacityPersistence, ConfigPersistence,
    NamespacePersistence, PersistenceService,
};

// Re-export SQL backend
pub use sql::ExternalDbPersistService;

// Re-export embedded backend
pub use embedded::EmbeddedPersistService;

// Re-export distributed backend
pub use distributed::DistributedPersistService;

// Re-export model types
pub use model::{
    AiResourceInfo, AiResourceVersionInfo, CapacityInfo, ConfigGrayStorageData,
    ConfigHistoryStorageData, ConfigStorageData, DeployTopology, NamespaceInfo, Page,
    PermissionInfo, PipelineExecutionInfo, RoleInfo, StorageBackend, StorageMode, UserInfo,
};
