//! Configuration service layer
//!
//! This module provides:
//! - Export/Import functionality (pure serialization/deserialization functions)
//! - Encryption service
//! - Cache management
//! - Config change notification
//!
//! Note: Database CRUD operations are handled by PersistenceService in batata-persistence.

pub mod cache;
pub mod encryption;
pub mod export;
pub mod import;
pub mod notifier;
pub mod reconciliation;

pub use encryption::{
    ConfigEncryptionService, ConfigEncryptionServiceBuilder, EncryptionPattern,
    get_encryption_provider,
};
