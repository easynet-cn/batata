//! Nacos-compatible Configuration Management
//!
//! Implements configuration management with Nacos-native semantics:
//! - **Namespace → Group → DataId** hierarchy
//! - **Gray release** (beta, tag, percentage, IP-range rules)
//! - **Long-polling** based change notification
//! - **MD5-based** change detection
//! - **Config encryption** for sensitive data
//! - **History** and version tracking
//!
//! This crate is completely independent from `batata-config-consul`.
//! It depends only on `batata-foundation` for infrastructure traits.

pub mod error;
pub mod model;
pub mod service;
pub mod traits;
