//! Dynamic server list management
//!
//! Manages the list of available Nacos servers with health tracking
//! and automatic failover. Compatible with Nacos ServerListManager.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use dashmap::DashMap;
use parking_lot::RwLock;

/// Health status of a server
#[derive(Debug, Clone)]
struct ServerHealth {
    /// Consecutive failure count
    failures: u32,
    /// Whether the server is marked healthy
    healthy: bool,
}

/// Manages server list with health tracking and round-robin selection.
pub struct ServerListManager {
    /// All known servers (address -> health)
    servers: DashMap<String, ServerHealth>,
    /// Ordered server list for round-robin
    server_list: RwLock<Vec<String>>,
    /// Current index for round-robin selection
    current_index: AtomicUsize,
    /// Threshold for marking server unhealthy
    max_failures: u32,
}

impl ServerListManager {
    /// Create from a comma-separated server address string
    pub fn new(server_addrs: &str) -> Self {
        let addrs: Vec<String> = server_addrs
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let servers = DashMap::new();
        for addr in &addrs {
            servers.insert(
                addr.clone(),
                ServerHealth {
                    failures: 0,
                    healthy: true,
                },
            );
        }

        Self {
            servers,
            server_list: RwLock::new(addrs),
            current_index: AtomicUsize::new(0),
            max_failures: 3,
        }
    }

    /// Create from a Vec of addresses with custom max_failures.
    pub fn from_addrs(addrs: &[String], max_failures: u32) -> Self {
        let servers = DashMap::new();
        for addr in addrs {
            servers.insert(
                addr.clone(),
                ServerHealth {
                    failures: 0,
                    healthy: true,
                },
            );
        }
        Self {
            servers,
            server_list: RwLock::new(addrs.to_vec()),
            current_index: AtomicUsize::new(0),
            max_failures,
        }
    }

    /// Get the next healthy server using round-robin
    pub fn next_server(&self) -> Option<String> {
        let list = self.server_list.read();
        if list.is_empty() {
            return None;
        }

        let len = list.len();
        for _ in 0..len {
            let index = self.current_index.fetch_add(1, Ordering::Relaxed) % len;
            let addr = &list[index];
            if let Some(health) = self.servers.get(addr)
                && health.healthy
            {
                return Some(addr.clone());
            }
        }

        // All unhealthy, return any server (best-effort)
        Some(list[0].clone())
    }

    /// Mark a server as having a successful request
    pub fn mark_success(&self, addr: &str) {
        if let Some(mut health) = self.servers.get_mut(addr) {
            health.failures = 0;
            health.healthy = true;
        }
    }

    /// Mark a server as having a failed request
    pub fn mark_failure(&self, addr: &str) {
        if let Some(mut health) = self.servers.get_mut(addr) {
            health.failures += 1;
            if health.failures >= self.max_failures {
                health.healthy = false;
                tracing::warn!(
                    "Server {} marked unhealthy after {} consecutive failures",
                    addr,
                    health.failures
                );
            }
        }
    }

    /// Get the number of healthy servers
    pub fn healthy_count(&self) -> usize {
        self.servers
            .iter()
            .filter(|entry| entry.value().healthy)
            .count()
    }

    /// Get total server count
    pub fn total_count(&self) -> usize {
        self.servers.len()
    }

    /// Get all server addresses
    pub fn all_servers(&self) -> Vec<String> {
        self.server_list.read().clone()
    }

    /// Get only healthy server addresses
    pub fn healthy_servers(&self) -> Vec<String> {
        self.servers
            .iter()
            .filter(|entry| entry.value().healthy)
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Reset all servers to healthy
    pub fn reset_all(&self) {
        for mut entry in self.servers.iter_mut() {
            entry.failures = 0;
            entry.healthy = true;
        }
    }

    /// Update server list (e.g., from endpoint discovery)
    pub fn update_servers(&self, addrs: Vec<String>) {
        // Add new servers
        for addr in &addrs {
            self.servers.entry(addr.clone()).or_insert(ServerHealth {
                failures: 0,
                healthy: true,
            });
        }

        // Remove servers no longer in the list
        let to_remove: Vec<String> = self
            .servers
            .iter()
            .filter(|entry| !addrs.contains(entry.key()))
            .map(|entry| entry.key().clone())
            .collect();
        for addr in to_remove {
            self.servers.remove(&addr);
        }

        *self.server_list.write() = addrs;
    }

    /// Start periodic server list refresh from an endpoint
    pub fn start_refresh_task(self: &Arc<Self>, endpoint: String, interval: Duration) {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default();

            let mut timer = tokio::time::interval(interval);
            loop {
                timer.tick().await;
                match client.get(&endpoint).send().await {
                    Ok(response) => {
                        if let Ok(body) = response.text().await {
                            let addrs: Vec<String> = body
                                .lines()
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            if !addrs.is_empty() {
                                manager.update_servers(addrs);
                                tracing::debug!(
                                    "Server list refreshed: {} servers",
                                    manager.total_count()
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to refresh server list from {}: {}", endpoint, e);
                    }
                }
            }
        });
    }
}

impl Default for ServerListManager {
    fn default() -> Self {
        Self::new("127.0.0.1:8848")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_from_string() {
        let mgr = ServerListManager::new("10.0.0.1:8848, 10.0.0.2:8848, 10.0.0.3:8848");
        assert_eq!(mgr.total_count(), 3);
        assert_eq!(mgr.healthy_count(), 3);
    }

    #[test]
    fn test_round_robin() {
        let mgr = ServerListManager::new("a:8848,b:8848,c:8848");
        let s1 = mgr.next_server().unwrap();
        let s2 = mgr.next_server().unwrap();
        let s3 = mgr.next_server().unwrap();
        let s4 = mgr.next_server().unwrap();
        // Should cycle: a, b, c, a
        assert_eq!(s1, "a:8848");
        assert_eq!(s2, "b:8848");
        assert_eq!(s3, "c:8848");
        assert_eq!(s4, "a:8848");
    }

    #[test]
    fn test_mark_failure_and_recovery() {
        let mgr = ServerListManager::new("a:8848,b:8848");
        mgr.mark_failure("a:8848");
        mgr.mark_failure("a:8848");
        mgr.mark_failure("a:8848");
        assert_eq!(mgr.healthy_count(), 1); // a unhealthy

        mgr.mark_success("a:8848");
        assert_eq!(mgr.healthy_count(), 2); // a recovered
    }

    #[test]
    fn test_skip_unhealthy() {
        let mgr = ServerListManager::new("a:8848,b:8848");
        // Mark a as unhealthy
        for _ in 0..3 {
            mgr.mark_failure("a:8848");
        }
        // Should always return b
        for _ in 0..5 {
            assert_eq!(mgr.next_server().unwrap(), "b:8848");
        }
    }

    #[test]
    fn test_all_unhealthy_returns_first() {
        let mgr = ServerListManager::new("a:8848,b:8848");
        for _ in 0..3 {
            mgr.mark_failure("a:8848");
            mgr.mark_failure("b:8848");
        }
        // Should still return something (best-effort)
        assert!(mgr.next_server().is_some());
    }

    #[test]
    fn test_update_servers() {
        let mgr = ServerListManager::new("a:8848,b:8848");
        mgr.update_servers(vec!["b:8848".into(), "c:8848".into()]);
        assert_eq!(mgr.total_count(), 2);
        assert!(mgr.all_servers().contains(&"c:8848".to_string()));
        assert!(!mgr.all_servers().contains(&"a:8848".to_string()));
    }

    #[test]
    fn test_reset_all() {
        let mgr = ServerListManager::new("a:8848,b:8848");
        for _ in 0..3 {
            mgr.mark_failure("a:8848");
        }
        assert_eq!(mgr.healthy_count(), 1);
        mgr.reset_all();
        assert_eq!(mgr.healthy_count(), 2);
    }

    #[test]
    fn test_empty_server_list() {
        let mgr = ServerListManager::new("");
        assert_eq!(mgr.total_count(), 0);
        assert!(mgr.next_server().is_none());
    }
}
