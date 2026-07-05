//! Server registry - manages lifecycle of all servers
//!
//! This module provides a centralized registry for managing
//! server lifecycle (start, stop, health check).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use tokio::sync::RwLock;

use crate::context::AppContext;
use crate::initializer::traits::{
    InitResult, ServerHandle, ServerHealth, ServerKind, ServerLifecycle,
};

/// Server registry entry
struct ServerEntry {
    lifecycle: Arc<dyn ServerLifecycle>,
    state: ServerHealth,
    handle: Option<ServerHandle>,
}

/// Server registry - manages all server lifecycles
///
/// Registers, starts, stops, and monitors health of all servers.
/// Servers are tracked by their ServerKind for easy lookup.
pub struct ServerRegistry {
    servers: RwLock<HashMap<ServerKind, ServerEntry>>,
}

impl ServerRegistry {
    /// Create a new empty server registry
    pub fn new() -> Self {
        Self {
            servers: RwLock::new(HashMap::new()),
        }
    }
    
    /// Register a server lifecycle
    pub async fn register(&self, lifecycle: Arc<dyn ServerLifecycle>) {
        let mut servers = self.servers.write().await;
        servers.insert(
            lifecycle.kind(),
            ServerEntry {
                lifecycle,
                state: ServerHealth::Starting,
                handle: None,
            },
        );
    }
    
    /// Start a registered server
    pub async fn start(&self, kind: ServerKind, ctx: &AppContext) -> InitResult<ServerHandle> {
        let lifecycle = {
            let servers = self.servers.read().await;
            servers.get(&kind).map(|e| e.lifecycle.clone())
        };
        
        let lifecycle = lifecycle.context(format!("Server {} not registered", kind))?;
        let handle = lifecycle.start(ctx).await?;
        
        {
            let mut servers = self.servers.write().await;
            if let Some(entry) = servers.get_mut(&kind) {
                entry.handle = Some(handle.clone());
                entry.state = ServerHealth::Running;
            }
        }
        
        Ok(handle)
    }
    
    /// Stop a registered server
    pub async fn stop(&self, kind: ServerKind) -> InitResult<()> {
        let (lifecycle, handle) = {
            let mut servers = self.servers.write().await;
            if let Some(entry) = servers.get_mut(&kind) {
                entry.state = ServerHealth::Stopping;
                (entry.lifecycle.clone(), entry.handle.clone())
            } else {
                return Ok(()); // Not registered, nothing to stop
            }
        };
        
        if let Some(h) = handle {
            lifecycle.stop(&h).await?;
        }
        
        {
            let mut servers = self.servers.write().await;
            if let Some(entry) = servers.get_mut(&kind) {
                entry.state = ServerHealth::Stopped;
                entry.handle = None;
            }
        }
        
        Ok(())
    }
    
    /// Get health status of a server
    pub async fn health(&self, kind: ServerKind) -> Option<ServerHealth> {
        let servers = self.servers.read().await;
        servers.get(&kind).map(|e| e.state)
    }
    
    /// Get all server health statuses
    pub async fn health_all(&self) -> Vec<(ServerKind, ServerHealth)> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .map(|(k, e)| (k.clone(), e.state))
            .collect()
    }
    
    /// Set server to draining state
    pub async fn set_draining(&self, kind: ServerKind) {
        let mut servers = self.servers.write().await;
        if let Some(entry) = servers.get_mut(&kind) {
            entry.state = ServerHealth::Draining;
        }
    }
    
    /// Set all servers to draining state
    pub async fn set_all_draining(&self) {
        let mut servers = self.servers.write().await;
        for entry in servers.values_mut() {
            entry.state = ServerHealth::Draining;
        }
    }
    
    /// Check if a server is registered
    pub async fn contains(&self, kind: ServerKind) -> bool {
        let servers = self.servers.read().await;
        servers.contains_key(&kind)
    }
    
    /// Get the number of registered servers
    pub async fn len(&self) -> usize {
        let servers = self.servers.read().await;
        servers.len()
    }
    
    /// Check if the registry is empty
    pub async fn is_empty(&self) -> bool {
        let servers = self.servers.read().await;
        servers.is_empty()
    }
}

impl Default for ServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ServerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerRegistry")
            .field("servers", &"RwLock<HashMap>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_registry_is_empty() {
        let registry = ServerRegistry::new();
        assert!(registry.is_empty().await);
        assert_eq!(registry.len().await, 0);
    }

    #[tokio::test]
    async fn test_health_on_empty_registry() {
        let registry = ServerRegistry::new();
        assert_eq!(registry.health(ServerKind::GrpcSdk).await, None);
        assert!(registry.health_all().await.is_empty());
    }

    #[tokio::test]
    async fn test_set_draining_on_empty_registry_is_noop() {
        let registry = ServerRegistry::new();
        registry.set_draining(ServerKind::GrpcSdk).await;
        registry.set_all_draining().await;
        // Should not panic
    }

    #[tokio::test]
    async fn test_contains_on_empty_registry() {
        let registry = ServerRegistry::new();
        assert!(!registry.contains(ServerKind::GrpcSdk).await);
        assert!(!registry.contains(ServerKind::HttpMain).await);
    }

    #[tokio::test]
    async fn test_stop_on_empty_registry_is_ok() {
        let registry = ServerRegistry::new();
        let result = registry.stop(ServerKind::GrpcSdk).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_default_registry_is_empty() {
        let registry = ServerRegistry::default();
        // Default should create a valid empty registry
        drop(registry);
    }

    #[test]
    fn test_debug_format() {
        let registry = ServerRegistry::new();
        let debug = format!("{:?}", registry);
        assert!(debug.contains("ServerRegistry"));
    }
}
