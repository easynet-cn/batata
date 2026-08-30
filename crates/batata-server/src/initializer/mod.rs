//! Initializer module - trait-driven service initialization
//!
//! This module provides initializer traits for different service types,
//! enabling modular and testable service setup.

/// `traits` module.
pub mod traits;
/// `config_initializer` module.
pub mod config_initializer;
/// `health_check_initializer` module.
pub mod health_check_initializer;
/// `ai_initializer` module.
pub mod ai_initializer;
/// `plugin_initializer` module.
pub mod plugin_initializer;
/// `auth_initializer` module.
pub mod auth_initializer;
/// `encryption_initializer` module.
pub mod encryption_initializer;

/// Re-exported item.
pub use traits::*;
