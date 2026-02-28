//! Console V3 API handlers
//!
//! This module provides the HTTP handlers for the v3 console API.
//! These handlers use the AppState and ConsoleDataSource abstraction for data access.
//! AI handlers (mcp, a2a, plugin) remain in batata-server.

pub mod cluster;
pub mod config;
pub mod health;
pub mod history;
pub mod instance;
pub mod metrics;
pub mod namespace;
pub mod route;
pub mod server_state;
pub mod service;
