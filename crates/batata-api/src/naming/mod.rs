//! Naming/Service discovery API models
//!
//! This module defines request/response models used in Batata service discovery.

/// Naming request/response models.
pub mod model;
/// Naming service provider trait.
pub mod traits;

pub use model::*;
pub use traits::NamingServiceProvider;
