//! Server status lifecycle management.
//!
//! The implementation now lives in `batata-common` so that lower layers (notably the
//! gRPC request path in `batata-core`) can read the status without depending on this
//! crate. This module re-exports it so existing import paths keep working:
//!
//! ```ignore
//! use crate::model::server_status::ServerStatusManager;
//! ```

pub use batata_common::server_status::{ServerStatus, ServerStatusManager};
