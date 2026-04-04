//! Adapter layer for bridging `NacosNamingServiceImpl` to the legacy
//! `NamingServiceProvider` trait from `batata-api`.
//!
//! This module provides:
//! - Type conversion between `batata_api::Instance` and `NacosInstance`
//! - An implementation of `NamingServiceProvider` for `NacosNamingServiceImpl`
//!
//! This enables the new domain-isolated implementation to work with
//! existing HTTP/gRPC handlers without modifying them.
//!
//! Migration path:
//! 1. Use this adapter to run the new implementation with existing handlers
//! 2. Gradually migrate handlers to use `NacosNamingService` trait directly
//! 3. Remove this adapter when migration is complete

mod convert;
mod provider;
#[cfg(test)]
mod tests;

pub use convert::{from_api_instance, to_api_instance};
