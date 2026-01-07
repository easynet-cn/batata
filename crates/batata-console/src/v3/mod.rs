//! Console V3 API handlers
//!
//! This module provides the HTTP handlers for the v3 console API.
//! These handlers use the ConsoleDataSource abstraction for data access,
//! allowing them to work in both local and remote modes.

pub mod cluster;
pub mod config;
pub mod health;
pub mod history;
pub mod metrics;
pub mod namespace;
pub mod route;
pub mod server_state;
