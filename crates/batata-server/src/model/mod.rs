//! Data models module
//!
//! This module contains all data models, structures, and shared types used across the application.
//!
//! # Module Structure
//!
//! - `constants` - Server-specific constants and re-exports from batata_api
//! - `config` - Configuration management
//! - `response` - HTTP response types (Result, ErrorResult, ConsoleException)
//! - `app_state` - Application state shared across handlers
//! - `common` - Re-exports for backward compatibility
//! - `tls` - TLS configuration for gRPC servers

pub mod app_state;
pub mod common;
pub mod config;
pub mod constants;
pub mod response;
pub mod tls;

// Re-export commonly used types at the module level
pub use app_state::AppState;
pub use config::Configuration;
pub use constants::*;
pub use response::{ConsoleException, ErrorResult, Result};
pub use tls::GrpcTlsConfig;
