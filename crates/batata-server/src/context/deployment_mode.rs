//! Deployment mode enumeration
//!
//! Defines how the Batata server is deployed and what components
//! are active.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Deployment mode for the Batata server
///
/// Determines which servers and services are started based on
/// the deployment configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentMode {
    /// Merged mode: Console (8081) + Main Server (8848) in same process
    #[default]
    Merged,
    
    /// Server only: Main Server (8848) + gRPC (9848/9849), no console
    Server,
    
    /// Console only: Connects to remote server
    Console,
    
    /// Server with MCP: Main Server + MCP Registry (9080)
    ServerWithMcp,
}

impl DeploymentMode {
    /// Create from configuration string
    pub fn from_str(s: &str) -> Self {
        match s {
            "server" => DeploymentMode::Server,
            "console" => DeploymentMode::Console,
            "serverWithMcp" | "server_with_mcp" => DeploymentMode::ServerWithMcp,
            _ => DeploymentMode::Merged,
        }
    }
    
    /// Get the configuration key value
    pub fn as_str(&self) -> &'static str {
        match self {
            DeploymentMode::Merged => "merged",
            DeploymentMode::Server => "server",
            DeploymentMode::Console => "console",
            DeploymentMode::ServerWithMcp => "serverWithMcp",
        }
    }
    
    /// Check if this mode includes the main HTTP server
    pub fn has_main_server(&self) -> bool {
        matches!(self, DeploymentMode::Merged | DeploymentMode::Server | DeploymentMode::ServerWithMcp)
    }
    
    /// Check if this mode includes the console HTTP server
    pub fn has_console_server(&self) -> bool {
        matches!(self, DeploymentMode::Merged | DeploymentMode::Console)
    }
    
    /// Check if this mode includes MCP registry
    pub fn has_mcp_registry(&self) -> bool {
        matches!(self, DeploymentMode::ServerWithMcp)
    }
}

impl fmt::Display for DeploymentMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<String> for DeploymentMode {
    fn from(s: String) -> Self {
        Self::from_str(&s)
    }
}

impl From<&str> for DeploymentMode {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_deployment_mode_from_str() {
        assert_eq!(DeploymentMode::from_str("merged"), DeploymentMode::Merged);
        assert_eq!(DeploymentMode::from_str("server"), DeploymentMode::Server);
        assert_eq!(DeploymentMode::from_str("console"), DeploymentMode::Console);
        assert_eq!(DeploymentMode::from_str("serverWithMcp"), DeploymentMode::ServerWithMcp);
        assert_eq!(DeploymentMode::from_str("unknown"), DeploymentMode::Merged);
    }
    
    #[test]
    fn test_deployment_mode_as_str() {
        assert_eq!(DeploymentMode::Merged.as_str(), "merged");
        assert_eq!(DeploymentMode::Server.as_str(), "server");
        assert_eq!(DeploymentMode::Console.as_str(), "console");
        assert_eq!(DeploymentMode::ServerWithMcp.as_str(), "serverWithMcp");
    }
    
    #[test]
    fn test_deployment_mode_has_server() {
        assert!(DeploymentMode::Merged.has_main_server());
        assert!(DeploymentMode::Server.has_main_server());
        assert!(!DeploymentMode::Console.has_main_server());
        assert!(DeploymentMode::ServerWithMcp.has_main_server());
    }
    
    #[test]
    fn test_deployment_mode_has_console() {
        assert!(DeploymentMode::Merged.has_console_server());
        assert!(!DeploymentMode::Server.has_console_server());
        assert!(DeploymentMode::Console.has_console_server());
        assert!(!DeploymentMode::ServerWithMcp.has_console_server());
    }
    
    #[test]
    fn test_deployment_mode_has_mcp() {
        assert!(!DeploymentMode::Merged.has_mcp_registry());
        assert!(!DeploymentMode::Server.has_mcp_registry());
        assert!(!DeploymentMode::Console.has_mcp_registry());
        assert!(DeploymentMode::ServerWithMcp.has_mcp_registry());
    }
    
    #[test]
    fn test_deployment_mode_display() {
        assert_eq!(format!("{}", DeploymentMode::Merged), "merged");
        assert_eq!(format!("{}", DeploymentMode::Server), "server");
    }
    
    #[test]
    fn test_deployment_mode_default() {
        assert_eq!(DeploymentMode::default(), DeploymentMode::Merged);
    }
}
