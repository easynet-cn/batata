//! Builder module - trait-driven application building
//!
//! This module provides a builder pattern for constructing the Batata server
//! application with clear separation of concerns across different phases.

/// `app_builder` module.
pub mod app_builder;

/// Re-exported item.
pub use app_builder::AppBuilder;
