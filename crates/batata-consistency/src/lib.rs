//! Batata Consistency - Raft and Distro consensus protocols
//!
//! This crate provides:
//! - Raft implementation (CP protocol for persistent data)
//! - Distro implementation (AP protocol for ephemeral data)
//! - State machine definitions
//! - Log storage

#![allow(clippy::result_large_err)]

pub mod raft;

// Re-export commonly used types
pub use raft::types::*;
