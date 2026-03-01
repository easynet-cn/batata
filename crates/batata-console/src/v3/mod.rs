//! Console V3 API handlers
//!
//! This module provides the HTTP handlers for the v3 console API.
//! These handlers use the AppState and ConsoleDataSource abstraction for data access.

pub mod ai_a2a;
pub mod ai_mcp;
pub mod ai_plugin;
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
