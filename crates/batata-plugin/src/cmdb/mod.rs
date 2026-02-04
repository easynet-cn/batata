//! CMDB Plugin - Configuration Management Database Integration
//!
//! This module provides:
//! - CMDB Plugin SPI (PLG-201)
//! - Label sync (PLG-202)
//! - Entity mapping (PLG-203)

mod model;
mod service;

pub use model::*;
pub use service::*;
