//! Naming/Service discovery API models
//!
//! This module defines request/response models used in Batata service discovery.

/// Naming request/response models.
pub mod model;
/// Surface-independent service create/update payload.
pub mod service_form;
/// Naming service provider trait.
pub mod traits;

pub use model::*;
pub use service_form::*;
pub use traits::{DeleteServiceError, NamingServiceProvider};
