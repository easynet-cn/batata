//! Context module - shared application context
//!
//! This module provides context types that hold references to all
//! services and configuration needed across the application.

/// `app_context` module.
pub mod app_context;
/// `deployment_mode` module.
pub mod deployment_mode;

/// Re-exported item.
pub use app_context::AppContext;
/// Re-exported item.
pub use deployment_mode::DeploymentMode;
