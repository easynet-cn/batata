//! Batata Config - Configuration management service
//!
//! This crate provides:
//! - Config CRUD operations
//! - Config listening/pushing
//! - Gray release (beta configs)
//! - Import/Export functionality
//! - History management
//! - HTTP API handlers (V2, V3 admin, V3 client)
//! - Config fuzzy watch manager

#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]
#![allow(clippy::empty_line_after_doc_comments)]

pub mod api;
pub mod handler;
pub mod model;
pub mod service;

// Re-export commonly used types
pub use model::*;
pub use service::cache::ConfigCacheService;
pub use service::notifier::ConfigChangeNotifier;
pub use service::reconciliation::{ReconciliationConfig, start_reconciliation_task};
