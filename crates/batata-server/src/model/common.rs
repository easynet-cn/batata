//! Common models and shared application structures
//!
//! This module re-exports types from submodules for backward compatibility.
//! New code should import directly from the specific submodules:
//! - `crate::model::config::Configuration`
//! - `crate::model::app_state::AppState`
//! - `crate::model::response::{Result, ErrorResult, ConsoleException}`
//! - `crate::model::constants::*`

// Re-export everything from submodules for backward compatibility
pub use super::app_state::*;
pub use super::config::*;
pub use super::constants::*;
pub use super::response::*;
