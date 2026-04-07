//! Context traits for dependency injection
//!
//! These traits abstract away concrete implementations, allowing
//! different crates to depend only on the traits they need.

mod ai;
mod auth;
mod cluster;
mod config;
mod core_ctx;
mod messaging;
mod plugin;

pub use ai::*;
pub use auth::*;
pub use cluster::*;
pub use config::*;
pub use core_ctx::*;
pub use messaging::*;
pub use plugin::*;
