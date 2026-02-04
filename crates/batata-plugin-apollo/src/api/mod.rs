//! Apollo API handlers
//!
//! HTTP handlers for Apollo Config Service API compatibility.

pub mod advanced;
mod config;
mod configfiles;
mod notification;
pub mod openapi;
mod route;

pub use config::*;
pub use configfiles::*;
pub use notification::*;
pub use openapi::*;
pub use route::*;
