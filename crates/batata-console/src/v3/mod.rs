//! Console V3 API handlers
//!
//! This module provides the HTTP handlers for the v3 console API.
//! These handlers use the AppState and ConsoleDataSource abstraction for data access.

pub mod ai_a2a;
pub mod ai_agentspec;
pub mod ai_mcp;
pub mod ai_pipeline;
pub mod ai_plugin;
pub mod ai_skill;
pub mod audit;
pub mod cluster;
pub mod cmdb;
pub mod config;
pub mod control;
pub mod health;
pub mod history;
pub mod instance;
pub mod metrics;
pub mod namespace;
pub mod plugin;
pub mod prometheus;
pub mod route;
pub mod server_state;
pub mod service;
pub mod sync;
pub mod tracing_api;
