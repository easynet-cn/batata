//! Graceful shutdown trait and utilities
//!
//! This module provides utilities for implementing graceful shutdown
//! across all server components.

use std::sync::Arc;
use std::time::Duration;

use crate::initializer::traits::{GracefulShutdownable, InitResult};

/// Shutdown priority constants
pub mod priority {
    /// HTTP servers should stop first
    pub const HTTP_SERVER: u8 = 10;
    
    /// gRPC servers should stop after HTTP
    pub const GRPC_SERVER: u8 = 20;
    
    /// Health checks should stop with gRPC
    pub const HEALTH_CHECK: u8 = 25;
    
    /// Cluster manager should stop after gRPC
    pub const CLUSTER_MANAGER: u8 = 30;
    
    /// Raft should stop after cluster manager
    pub const RAFT: u8 = 40;
    
    /// Database connection should close last
    pub const DATABASE: u8 = 50;
}

/// Graceful shutdown helper for components
#[derive(Clone)]
pub struct ShutdownHelper {
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    is_shutdown: Arc<std::sync::atomic::AtomicBool>,
}

impl ShutdownHelper {
    /// Create a new shutdown helper
    pub fn new() -> (Self, tokio::sync::watch::Receiver<bool>) {
        let (tx, rx) = tokio::sync::watch::channel(false);
        (
            Self {
                shutdown_tx: tx,
                is_shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
            rx,
        )
    }

    /// Signal shutdown to all listeners
    pub fn shutdown(&self) {
        self.is_shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = self.shutdown_tx.send(true);
    }

    /// Check if shutdown has been signaled
    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Default for ShutdownHelper {
    fn default() -> Self {
        let (tx, _) = tokio::sync::watch::channel(false);
        Self {
            shutdown_tx: tx,
            is_shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

/// Draining utility for graceful request completion
pub struct DrainHelper {
    timeout: Duration,
}

impl DrainHelper {
    /// Create a new drain helper with the specified timeout
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
    
    /// Wait for drain to complete
    pub async fn drain<F, Fut>(&self, check_fn: F) -> InitResult<()>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let start = std::time::Instant::now();
        
        while start.elapsed() < self.timeout {
            if check_fn().await {
                tracing::info!("Drain completed successfully");
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        tracing::warn!("Drain timed out after {:?}", self.timeout);
        Ok(())
    }
}

/// A closure-based shutdown component for wrapping arbitrary shutdown logic
///
/// This makes it easy to integrate any async shutdown function into the
/// ShutdownOrchestrator without having to implement the full trait.
pub struct ClosureShutdown {
    name: &'static str,
    order: u8,
    shutdown_fn: Box<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = InitResult<()>> + Send>> + Send + Sync>,
    drain_fn: Box<dyn Fn(Duration) -> std::pin::Pin<Box<dyn std::future::Future<Output = InitResult<()>> + Send>> + Send + Sync>,
}

impl ClosureShutdown {
    /// Create a new closure-based shutdown component
    pub fn new<F, Fut>(name: &'static str, order: u8, shutdown_fn: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = InitResult<()>> + Send + 'static,
    {
        Self {
            name,
            order,
            shutdown_fn: Box::new(move || Box::pin(shutdown_fn())),
            drain_fn: Box::new(move |_| Box::pin(async { Ok(()) })),
        }
    }

    /// Set a custom drain function
    pub fn with_drain<F, Fut>(mut self, drain_fn: F) -> Self
    where
        F: Fn(Duration) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = InitResult<()>> + Send + 'static,
    {
        self.drain_fn = Box::new(move |timeout| Box::pin(drain_fn(timeout)));
        self
    }
}

#[async_trait::async_trait]
impl GracefulShutdownable for ClosureShutdown {
    fn shutdown_order(&self) -> u8 {
        self.order
    }

    async fn shutdown(&self) -> InitResult<()> {
        tracing::info!("Shutting down: {}", self.name);
        (self.shutdown_fn)().await
    }

    async fn drain(&self, timeout: Duration) -> InitResult<()> {
        tracing::debug!("Draining: {} (timeout: {:?})", self.name, timeout);
        (self.drain_fn)(timeout).await
    }
}

/// Composite shutdown orchestrator
///
/// Orchestrates shutdown of multiple components in priority order.
/// Components with lower shutdown_order values shut down first.
pub struct ShutdownOrchestrator {
    components: Vec<Box<dyn GracefulShutdownable>>,
    drain_timeout: Duration,
}

impl ShutdownOrchestrator {
    /// Create a new empty orchestrator
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
            drain_timeout: Duration::from_secs(30),
        }
    }

    /// Set the drain timeout for all components
    pub fn with_drain_timeout(mut self, timeout: Duration) -> Self {
        self.drain_timeout = timeout;
        self
    }

    /// Add a component to be shut down
    pub fn add<C: GracefulShutdownable + 'static>(&mut self, component: Arc<C>) {
        self.components.push(Box::new(component));
    }

    /// Get the number of registered components
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Check if no components are registered
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// Execute drain phase for all components (in shutdown order)
    pub async fn drain(&mut self) -> InitResult<()> {
        self.components.sort_by_key(|c| c.shutdown_order());

        tracing::info!(
            "Starting drain phase for {} components (timeout: {:?})",
            self.components.len(),
            self.drain_timeout
        );

        for component in &self.components {
            let order = component.shutdown_order();
            tracing::debug!("Draining component (order: {})...", order);
            if let Err(e) = component.drain(self.drain_timeout).await {
                tracing::error!("Drain error (order: {}): {}", order, e);
            }
        }

        tracing::info!("Drain phase complete");
        Ok(())
    }

    /// Execute shutdown phase for all components (in shutdown order)
    pub async fn shutdown(&mut self) -> InitResult<()> {
        self.components.sort_by_key(|c| c.shutdown_order());

        tracing::info!(
            "Starting shutdown phase for {} components",
            self.components.len()
        );

        for component in &self.components {
            let order = component.shutdown_order();
            tracing::info!("Shutting down component (order: {})...", order);
            if let Err(e) = component.shutdown().await {
                tracing::error!("Shutdown error (order: {}): {}", order, e);
                // Continue shutting down other components
            }
        }

        tracing::info!("Shutdown phase complete");
        Ok(())
    }

    /// Execute full graceful shutdown: drain then shutdown
    pub async fn graceful_shutdown(&mut self) -> InitResult<()> {
        self.drain().await?;
        self.shutdown().await?;
        Ok(())
    }
}

impl Default for ShutdownOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
    use std::sync::Arc;
    
    struct MockShutdownable {
        order: Arc<AtomicU8>,
        shutdown_called: Arc<AtomicBool>,
        drain_called: Arc<AtomicBool>,
        shutdown_order_val: u8,
    }

    impl MockShutdownable {
        fn new(order: u8) -> Self {
            Self {
                order: Arc::new(AtomicU8::new(0)),
                shutdown_called: Arc::new(AtomicBool::new(false)),
                drain_called: Arc::new(AtomicBool::new(false)),
                shutdown_order_val: order,
            }
        }
    }

    #[async_trait]
    impl GracefulShutdownable for MockShutdownable {
        fn shutdown_order(&self) -> u8 {
            self.shutdown_order_val
        }

        async fn shutdown(&self) -> InitResult<()> {
            self.order.store(self.shutdown_order_val, Ordering::SeqCst);
            self.shutdown_called.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn drain(&self, _timeout: Duration) -> InitResult<()> {
            self.drain_called.store(true, Ordering::SeqCst);
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_shutdown_helper() {
        let (helper, mut rx) = ShutdownHelper::new();
        
        assert!(!helper.is_shutdown());
        
        helper.shutdown();
        
        // Receiver should receive the signal
        rx.changed().await.unwrap();
        assert!(rx.borrow().to_owned());
    }
    
    #[tokio::test]
    async fn test_drain_helper() {
        let drain = DrainHelper::new(Duration::from_millis(100));
        let counter = Arc::new(std::sync::atomic::AtomicU8::new(0));
        let counter_clone = counter.clone();
        
        let result = drain.drain(|| async {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            counter_clone.load(Ordering::SeqCst) >= 3
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
    
    #[tokio::test]
    async fn test_drain_helper_timeout() {
        let drain = DrainHelper::new(Duration::from_millis(50));
        
        let result = drain.drain(|| async { false }).await;
        
        // Should succeed even if not completed
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_shutdown_orchestrator() {
        let mut orchestrator = ShutdownOrchestrator::new();

        let comp1 = Arc::new(MockShutdownable::new(2));
        let comp2 = Arc::new(MockShutdownable::new(1));
        let comp3 = Arc::new(MockShutdownable::new(3));

        orchestrator.add(comp1.clone());
        orchestrator.add(comp2.clone());
        orchestrator.add(comp3.clone());

        assert_eq!(orchestrator.len(), 3);
        assert!(!orchestrator.is_empty());

        orchestrator.shutdown().await.unwrap();

        // All should be shut down
        assert!(comp1.shutdown_called.load(Ordering::SeqCst));
        assert!(comp2.shutdown_called.load(Ordering::SeqCst));
        assert!(comp3.shutdown_called.load(Ordering::SeqCst));

        // Order should be 1, 2, 3 (sorted by shutdown_order)
        assert_eq!(comp1.order.load(Ordering::SeqCst), 2);
        assert_eq!(comp2.order.load(Ordering::SeqCst), 1);
        assert_eq!(comp3.order.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_shutdown_orchestrator_drain() {
        let mut orchestrator = ShutdownOrchestrator::new().with_drain_timeout(Duration::from_millis(100));

        let comp1 = Arc::new(MockShutdownable::new(10));
        let comp2 = Arc::new(MockShutdownable::new(20));

        orchestrator.add(comp1.clone());
        orchestrator.add(comp2.clone());

        orchestrator.drain().await.unwrap();

        // Both should have drain called
        assert!(comp1.drain_called.load(Ordering::SeqCst));
        assert!(comp2.drain_called.load(Ordering::SeqCst));
        // Shutdown should NOT have been called
        assert!(!comp1.shutdown_called.load(Ordering::SeqCst));
        assert!(!comp2.shutdown_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_shutdown_orchestrator_graceful_shutdown() {
        let mut orchestrator = ShutdownOrchestrator::new().with_drain_timeout(Duration::from_millis(50));

        let comp = Arc::new(MockShutdownable::new(5));
        orchestrator.add(comp.clone());

        orchestrator.graceful_shutdown().await.unwrap();

        assert!(comp.drain_called.load(Ordering::SeqCst));
        assert!(comp.shutdown_called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_shutdown_orchestrator_empty() {
        let orchestrator = ShutdownOrchestrator::new();
        assert_eq!(orchestrator.len(), 0);
        assert!(orchestrator.is_empty());
    }

    #[test]
    fn test_priority_constants() {
        assert!(priority::HTTP_SERVER < priority::GRPC_SERVER);
        assert!(priority::GRPC_SERVER < priority::HEALTH_CHECK);
        assert!(priority::HEALTH_CHECK < priority::CLUSTER_MANAGER);
        assert!(priority::CLUSTER_MANAGER < priority::RAFT);
        assert!(priority::RAFT < priority::DATABASE);
    }
}
