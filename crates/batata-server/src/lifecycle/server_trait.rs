//! Server trait definitions
//!
//! This module re-exports the core server traits from the initializer module.

/// Re-exported item.
pub use crate::initializer::traits::{
    ServerKind, ServerHealth, ServerHandle, ServerLifecycle,
};
