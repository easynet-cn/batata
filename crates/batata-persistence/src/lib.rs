//! Batata Persistence - Database entities and persistence layer
//!
//! This crate provides:
//! - SeaORM entity definitions (auto-generated)
//! - Persistence trait abstractions for unified storage
//! - Domain model types for persistence operations

pub mod distributed;
pub mod embedded;
pub mod entity;
pub mod model;
pub mod sql;
pub mod traits;

// Re-export sea-orm for convenience
pub use sea_orm;

// Re-export entity prelude
pub use entity::prelude::*;

// Re-export persistence traits
pub use traits::{
    AuthPersistence, CapacityPersistence, ConfigPersistence, NamespacePersistence,
    PersistenceService,
};

// Re-export SQL backend
pub use sql::ExternalDbPersistService;

// Re-export embedded backend
pub use embedded::EmbeddedPersistService;

// Re-export distributed backend
pub use distributed::DistributedPersistService;

// Re-export model types
pub use model::{
    CapacityInfo, ConfigGrayStorageData, ConfigHistoryStorageData, ConfigStorageData,
    NamespaceInfo, Page, PermissionInfo, RoleInfo, StorageMode, UserInfo,
};
