//! Batata Client - Rust SDK for Nacos clients
//!
//! This crate provides:
//! - HTTP client with authentication, retry, and failover
//! - API client with typed methods for all console endpoints
//! - Model types for API responses

pub mod api;
pub mod http;
pub mod model;

pub use api::BatataApiClient;
pub use http::{BatataHttpClient, HttpClientConfig};
pub use model::*;
