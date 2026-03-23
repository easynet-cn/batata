//! Naming/Service discovery API models
//!
//! This module defines request/response models used in Nacos service discovery.

pub mod model;
pub mod traits;

pub use model::*;
pub use traits::NamingServiceProvider;
