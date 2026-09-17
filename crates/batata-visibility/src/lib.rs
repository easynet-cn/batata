//! Batata Visibility Plugin — Resource visibility control for AI resources
//!
//! Aligned with Nacos `plugin/visibility` module.
//!
//! # Architecture
//! - `VisibilityService` trait — SPI for custom visibility implementations
//! - `VisibilityPluginManager` — loads and dispatches to registered services
//! - `DefaultVisibilityService` — built-in PUBLIC/PRIVATE logic
//! - Models — `ValidationResult`, `QueryAdvisor`, `BaseVisibilityPredicate`, etc.
//!
//! # Quick Start
//! ```rust,ignore
//! use batata_visibility::{VisibilityPluginManager, DefaultVisibilityService};
//! use std::sync::Arc;
//!
//! let manager = VisibilityPluginManager::instance();
//! manager.register(Arc::new(DefaultVisibilityService::new()));
//! ```

#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]
#![allow(clippy::empty_line_after_doc_comments)]

pub mod constants;
pub mod default_impl;
pub mod manager;
pub mod model;
pub mod spi;

// Public exports
pub use constants::{ACTION_READ, ACTION_WRITE, SCOPE_PRIVATE, SCOPE_PUBLIC};
pub use default_impl::DefaultVisibilityService;
pub use manager::VisibilityPluginManager;
pub use model::{
    AuthorizedResources, BaseVisibilityPredicate, GenericVisibilityResource, QueryAdvisor,
    ValidationResult, VisibilityQueryContext, VisibilityResource,
};
pub use spi::VisibilityService;
