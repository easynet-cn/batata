#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]
#![allow(clippy::empty_line_after_doc_comments)]
//! Batata API - shared protocol contract
//!
//! This crate provides the protocol contract shared by Batata clients and
//! the server:
//! - Common API models and constants
//! - gRPC service definitions (generated from proto)
//! - HTTP API request/response models
//!
//! It deliberately excludes server-only modules (`raft`, `distro`,
//! `validation`, now in `batata-server-api`) and has no dependency on
//! `batata-common`, so external SDK clients can depend on it without pulling
//! in server-side heavy dependencies.

/// Shared macros for API definitions.
#[macro_use]
pub mod macros;
/// Config API models.
pub mod config;
/// gRPC service definitions (generated from proto).
pub mod grpc;
/// Common API models and constants.
pub mod model;
/// Naming/service discovery API models.
pub mod naming;
/// Remote API models.
pub mod remote;

// Re-export commonly used types
pub use model::*;

/// Returns the local non-loopback IPv4 address, or "127.0.0.1" if none found.
///
/// Moved here from `batata-common` so SDK clients can resolve the local IP
/// without depending on the server-side utility crate.
pub fn local_ip() -> String {
    if_addrs::get_if_addrs()
        .ok()
        .and_then(|addrs| {
            addrs
                .into_iter()
                .find(|iface| !iface.is_loopback() && matches!(iface.addr, if_addrs::IfAddr::V4(_)))
                .and_then(|iface| match iface.addr {
                    if_addrs::IfAddr::V4(addr) => Some(addr.ip.to_string()),
                    _ => None,
                })
        })
        .unwrap_or_else(|| "127.0.0.1".to_string())
}
