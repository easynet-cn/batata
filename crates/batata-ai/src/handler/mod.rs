//! gRPC handlers for MCP and A2A management.
//!
//! Implements `PayloadHandler` for MCP server and A2A agent management over gRPC,
//! using config-backed operation services when available and falling back to
//! in-memory registries.

/// gRPC payload handlers implementing MCP and A2A management operations.
pub mod ai_handler;
