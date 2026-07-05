//! Server trait definitions
//!
//! This module re-exports the core server traits from the initializer module.

pub use crate::initializer::traits::{
    ServerKind, ServerHealth, ServerHandle, ServerLifecycle,
};
