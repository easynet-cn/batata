//! gRPC handler infrastructure and core handlers
//!
//! This module provides the PayloadHandler trait, handler registry,
//! RPC services, and core gRPC message handlers.

#[macro_use]
pub mod macros;
pub mod cluster;
pub mod distro;
pub mod generic;
pub mod lock;
pub mod rpc;
