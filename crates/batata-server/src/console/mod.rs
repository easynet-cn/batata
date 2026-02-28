// Console web interface module
// AI handler endpoints remain in batata-server since they depend on server-specific AI types.
// Non-AI console handlers have been moved to the batata-console crate.

pub mod client;

pub mod v3 {
    pub mod a2a; // A2A agent management console endpoints
    pub mod mcp; // MCP server management console endpoints
    pub mod plugin; // Plugin management console endpoints
}
