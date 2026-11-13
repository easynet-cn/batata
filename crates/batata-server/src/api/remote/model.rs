//! Module `api::remote::model` of the `batata-server` crate.
// Remote API models for Batata protocol communication (Nacos gRPC protocol compatible)
// This file re-exports types from batata_api::remote::model

// Re-export all types from batata_api::remote::model, plus the server-only
// AI/MCP/RAD models that moved to batata-ai.
/// Re-exported item.
pub use batata_ai::rad_model::*;
/// Re-exported item.
pub use batata_api::remote::model::*;
