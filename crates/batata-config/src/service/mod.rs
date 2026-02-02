//! Configuration service layer
//!
//! This module provides database operations for configuration management:
//! - Config CRUD operations
//! - Config history tracking
//! - Namespace management
//! - Tag management
//! - Gray release (beta configs)
//! - Export/Import functionality

pub mod config;
pub mod encryption;
pub mod export;
pub mod history;
pub mod import;
pub mod namespace;

pub use config::*;
pub use encryption::{
    ConfigEncryptionService, ConfigEncryptionServiceBuilder, EncryptionPattern,
};
