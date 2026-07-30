//! Webhook Plugin - Event notification system
//!
//! This module provides:
//! - Webhook Plugin SPI (PLG-101)
//! - Config change notification (PLG-102)
//! - Service change notification (PLG-103)
//! - Retry mechanism (PLG-104)

mod config_plugin;
mod model;
mod service;

pub use config_plugin::*;
pub use model::*;
pub use service::*;
