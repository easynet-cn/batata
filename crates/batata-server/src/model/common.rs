//! Common models and shared application structures
//!
//! This module re-exports types from submodules for backward compatibility.
//! New code should import directly from the specific submodules.

// Re-export server-specific AppState (with console_datasource)
pub use super::app_state::*;
// Re-export Configuration from server-common
pub use batata_server_common::model::config::*;
// Re-export constants from server-common
pub use batata_server_common::model::constants::*;
// Re-export response types from server-common
pub use batata_server_common::model::response::*;
