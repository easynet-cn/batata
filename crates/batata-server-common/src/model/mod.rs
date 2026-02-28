//! Data models module
//!
//! This module contains shared data models, structures, and types used across
//! both batata-server and batata-console.

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
