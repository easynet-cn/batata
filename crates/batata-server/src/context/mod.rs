//! Context module - shared application context
//!
//! This module provides context types that hold references to all
//! services and configuration needed across the application.

pub mod app_context;
pub mod deployment_mode;

pub use app_context::AppContext;
pub use deployment_mode::DeploymentMode;
