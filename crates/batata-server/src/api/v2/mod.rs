//! Nacos V2 Open API implementation
//!
//! This module provides the V2 API endpoints compatible with Nacos 2.x/3.x.
//! The V2 API uses a unified response format with code, message, and data fields.

pub mod capacity;
pub mod client;
pub mod cluster;
pub mod config;
pub mod core_ops;
pub mod health;
pub mod history;
pub mod instance;
pub mod listener;
pub mod model;
pub mod naming_catalog;
pub mod operator;
pub mod route;
pub mod service;
