//! Initializer module - trait-driven service initialization
//!
//! This module provides initializer traits for different service types,
//! enabling modular and testable service setup.

pub mod traits;
pub mod config_initializer;
pub mod health_check_initializer;
pub mod ai_initializer;
pub mod plugin_initializer;
pub mod auth_initializer;
pub mod encryption_initializer;

pub use traits::*;
