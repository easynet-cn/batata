//! Batata Consistency - Raft and Distro consensus protocols
//!
//! This crate provides:
//! - Raft implementation (CP protocol for persistent data)
//! - Distro implementation (AP protocol for ephemeral data)
//! - State machine definitions
//! - Log storage
//! - Distributed locking (ADV-001 to ADV-005)

#![allow(clippy::result_large_err)]

pub mod lock;
pub mod raft;

// Re-export commonly used types
pub use raft::types::*;

// Re-export reader and state machine
pub use raft::reader::RocksDbReader;
pub use raft::state_machine::RocksStateMachine;

// Re-export lock types
pub use lock::{
    DistributedLock, DistributedLockService, LockAcquireRequest, LockAcquireResult, LockCommand,
    LockCommandResponse, LockQueryRequest, LockReleaseRequest, LockReleaseResult, LockRenewRequest,
    LockRenewResult, LockState, LockStats, MemoryLockService,
};
