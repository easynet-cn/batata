//! Distributed Lock Implementation (ADV-001 to ADV-005)
//!
//! This module provides:
//! - Distributed lock data model (ADV-001)
//! - Lock acquire/release API (ADV-002)
//! - Lock renewal mechanism (ADV-003)
//! - Lock auto-release on timeout (ADV-004)
//! - Raft-based lock implementation (ADV-005)

mod model;
mod service;

pub use model::*;
pub use service::*;
