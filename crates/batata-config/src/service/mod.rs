//! Configuration service layer
//!
//! This module provides database operations for configuration management:
//! - Config CRUD operations
//! - Config history tracking
//! - Namespace management
//! - Tag management
//! - Gray release (beta configs)
//! - Export/Import functionality
//! - Capacity quota management
//! - Audit logging
//! - Aggregate config (datumId-based)

pub mod aggr;
pub mod audit;
pub mod capacity;
pub mod config;
pub mod encryption;
pub mod export;
pub mod history;
pub mod import;
pub mod namespace;

pub use aggr::*;
pub use audit::*;
pub use capacity::*;
pub use config::*;
pub use encryption::{ConfigEncryptionService, ConfigEncryptionServiceBuilder, EncryptionPattern};
