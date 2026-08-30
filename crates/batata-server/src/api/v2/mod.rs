//! Batata V2 Open API implementation
//!
//! This module provides the V2 API endpoints compatible with Nacos 2.x/3.x SDKs.
//! The V2 API uses a unified response format with code, message, and data fields.

/// `capacity` module.
pub mod capacity;
/// `client` module.
pub mod client;
/// `cluster` module.
pub mod cluster;
/// `config` module.
pub mod config;
/// `core_ops` module.
pub mod core_ops;
/// `health` module.
pub mod health;
/// `history` module.
pub mod history;
/// `instance` module.
pub mod instance;
/// `listener` module.
pub mod listener;
/// `model` module.
pub mod model;
/// `naming_catalog` module.
pub mod naming_catalog;
/// `operator` module.
pub mod operator;
/// `route` module.
pub mod route;
/// `service` module.
pub mod service;
