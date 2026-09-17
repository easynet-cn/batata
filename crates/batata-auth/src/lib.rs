//! Batata Auth - Authentication and authorization
//!
//! This crate provides:
//! - JWT token handling
//! - RBAC permission model
//! - User, Role, Permission services
//! - Auth middleware

#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]
#![allow(clippy::empty_line_after_doc_comments)]

pub mod model;
pub mod plugin;
pub mod service;

// Re-export commonly used types
pub use model::*;
