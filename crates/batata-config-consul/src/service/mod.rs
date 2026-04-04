//! In-memory Consul KV store implementation
//!
//! DashMap-based concurrent KV store with Consul-native semantics:
//! - Flat key paths with prefix scanning
//! - CAS (Compare-And-Swap) for optimistic concurrency
//! - Session-based locking and ephemeral keys
//! - Blocking queries via monotonic index
//! - Transactions with atomic multi-key operations

mod kv;
mod session;

pub use kv::ConsulKvServiceImpl;
pub use session::ConsulSessionServiceImpl;
