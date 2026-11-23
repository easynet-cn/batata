//! Graceful shutdown handling for Batata server
//!
//! This module provides utilities for graceful shutdown of all server components.

use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{info, warn};

/// Shutdown signal sender and receiver
#[derive(Clone)]
pub struct ShutdownSignal {
    sender: broadcast::Sender<()>,
}

impl ShutdownSignal {
    /// Create a new shutdown signal with a broadcast channel
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1);
        Self { sender }
    }

    /// Get a receiver for shutdown notifications
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.sender.subscribe()
    }

    /// Trigger shutdown
    pub fn shutdown(&self) {
        let _ = self.sender.send(());
    }

    /// Check if shutdown has been triggered
    pub fn is_shutdown(&self) -> bool {
        self.sender.receiver_count() == 0
    }
}

impl Default for ShutdownSignal {
    fn default() -> Self {
        Self::new()
    }
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
///
/// Returns the shutdown signal that can be used to notify other components
pub async fn wait_for_shutdown_signal() -> ShutdownSignal {
    let shutdown = ShutdownSignal::new();
    let shutdown_clone = shutdown.clone();

    tokio::spawn(async move {
        let ctrl_c = async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to install SIGTERM handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received Ctrl+C, initiating graceful shutdown...");
            }
            _ = terminate => {
                info!("Received SIGTERM, initiating graceful shutdown...");
            }
        }

        shutdown_clone.shutdown();
    });

    shutdown
}

/// Graceful shutdown coordinator
///
/// Coordinates the shutdown of multiple server components with a timeout
#[derive(Clone)]
pub struct GracefulShutdown {
    shutdown_signal: ShutdownSignal,
    shutdown_timeout: Duration,
}

impl GracefulShutdown {
    /// Create a new graceful shutdown coordinator
    pub fn new(shutdown_signal: ShutdownSignal, shutdown_timeout: Duration) -> Self {
        Self {
            shutdown_signal,
            shutdown_timeout,
        }
    }

    /// Wait for shutdown signal.
    ///
    /// This returns as soon as the signal is received. The caller is
    /// responsible for orchestrating the actual cleanup sequence (setting
    /// server status to DRAINING, stopping subsystems, etc.).
    pub async fn wait_for_shutdown(&self) {
        let mut receiver = self.shutdown_signal.subscribe();
        let _ = receiver.recv().await;

        info!(
            "Shutdown signal received (drain timeout: {:?})",
            self.shutdown_timeout
        );
    }

    /// Returns the configured drain timeout.
    pub fn drain_timeout(&self) -> Duration {
        self.shutdown_timeout
    }

    /// Get a clone of the shutdown signal for passing to components
    pub fn signal(&self) -> ShutdownSignal {
        self.shutdown_signal.clone()
    }
}

/// Run a future with graceful shutdown support
///
/// The future will be cancelled when a shutdown signal is received
pub async fn run_with_shutdown<F, T>(
    future: F,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Option<T>
where
    F: std::future::Future<Output = T>,
{
    tokio::select! {
        result = future => Some(result),
        _ = shutdown_rx.recv() => {
            warn!("Shutdown signal received, cancelling operation");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_signal() {
        let signal = ShutdownSignal::new();
        let mut rx = signal.subscribe();

        // Should not be shutdown initially
        assert!(!signal.is_shutdown());

        // Trigger shutdown in a separate task
        let signal_clone = signal.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            signal_clone.shutdown();
        });

        // Should receive shutdown signal
        let result = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_with_shutdown() {
        let signal = ShutdownSignal::new();
        let rx = signal.subscribe();

        // Start a long-running task
        let task = async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            42
        };

        // Trigger shutdown after a short delay
        let signal_clone = signal.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            signal_clone.shutdown();
        });

        // The task should be cancelled
        let result = run_with_shutdown(task, rx).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn shutdown_signal_broadcast_reaches_multiple_subscribers() {
        let signal = ShutdownSignal::new();
        let mut first = signal.subscribe();
        let mut second = signal.subscribe();

        signal.shutdown();

        let first_result = tokio::time::timeout(Duration::from_millis(100), first.recv()).await;
        let second_result = tokio::time::timeout(Duration::from_millis(100), second.recv()).await;
        assert!(first_result.expect("first subscriber timed out").is_ok());
        assert!(second_result.expect("second subscriber timed out").is_ok());
    }

    #[tokio::test]
    async fn shutdown_signal_clone_shares_broadcast() {
        let signal = ShutdownSignal::new();
        let clone = signal.clone();
        let mut receiver = signal.subscribe();

        clone.shutdown();

        let result = tokio::time::timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(result.expect("cloned signal did not broadcast").is_ok());
    }

    #[test]
    fn graceful_shutdown_drain_timeout_is_exposed() {
        let signal = ShutdownSignal::new();
        let timeout = Duration::from_millis(1234);
        let graceful = GracefulShutdown::new(signal, timeout);
        assert_eq!(graceful.drain_timeout(), timeout);
    }

    #[tokio::test]
    async fn graceful_shutdown_signal_returns_working_clone() {
        let signal = ShutdownSignal::new();
        let graceful = GracefulShutdown::new(signal.clone(), Duration::from_secs(1));
        let mut receiver = signal.subscribe();

        graceful.signal().shutdown();

        let result = tokio::time::timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(
            result
                .expect("graceful signal clone did not broadcast")
                .is_ok()
        );
    }

    #[tokio::test]
    async fn graceful_shutdown_wait_returns_after_signal() {
        let signal = ShutdownSignal::new();
        let graceful = GracefulShutdown::new(signal.clone(), Duration::from_millis(250));
        let signal_clone = signal.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            signal_clone.shutdown();
        });

        tokio::time::timeout(Duration::from_millis(100), graceful.wait_for_shutdown())
            .await
            .expect("wait_for_shutdown did not return after the signal");
    }

    #[tokio::test]
    async fn run_with_shutdown_completes_future_on_normal_finish() {
        let signal = ShutdownSignal::new();
        let rx = signal.subscribe();

        let result = run_with_shutdown(async { 42 }, rx).await;
        assert_eq!(result, Some(42));
    }
}
