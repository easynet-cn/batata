//! Control Plugin - Rate limiting and connection control
//!
//! This module provides:
//! - Control Plugin SPI (PLG-001)
//! - TPS Rate Limiting (PLG-002)
//! - Connection Limiting (PLG-003)
//! - Rule Storage (PLG-004)

mod model;
mod rule_store;
mod service;

pub use model::*;
pub use rule_store::*;
pub use service::*;
