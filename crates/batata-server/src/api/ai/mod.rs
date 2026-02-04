//! AI Capabilities API
//!
//! This module provides API endpoints for AI model server management,
//! including MCP (Model Content Protocol) server registration and
//! A2A (Agent-to-Agent) communication.

pub mod a2a;
pub mod mcp;
pub mod model;

// Re-export registry types
pub use a2a::{AgentRegistry, AgentRegistryStats};
pub use mcp::{McpRegistryStats, McpServerRegistry};

// Re-export configure functions for route setup
pub use a2a::configure as configure_a2a;
pub use mcp::configure as configure_mcp;
