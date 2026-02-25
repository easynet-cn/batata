//! Health check reactor - schedules and manages health check tasks
//!
//! This module provides the HealthCheckReactor that matches Nacos HealthCheckReactor,
//! responsible for scheduling health check tasks for all instances.

use super::config::HealthCheckConfig;
use super::processor::{
    HealthCheckType, HttpHealthCheckProcessor, NoneHealthCheckProcessor, TcpHealthCheckProcessor,
};
use super::task::HealthCheckTask;
use crate::service::{ClusterConfig, NamingService};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, info};

/// Health check reactor message
#[derive(Debug)]
pub enum ReactorMessage {
    /// Schedule a new health check task
    Schedule { task: HealthCheckTask },
    /// Cancel a health check task
    Cancel { task_id: String },
    /// Shutdown the reactor
    Shutdown,
}

/// Health check reactor (matches Nacos HealthCheckReactor)
///
/// The reactor is responsible for:
/// - Scheduling health check tasks for all instances
/// - Managing task lifecycle (create, update, cancel)
/// - Adaptive check interval management
pub struct HealthCheckReactor {
    /// Naming service for accessing instances
    naming_service: Arc<NamingService>,

    /// Health check configuration
    config: Arc<HealthCheckConfig>,

    /// Active health check tasks (task_id -> task)
    tasks: Arc<DashMap<String, HealthCheckTask>>,

    /// Message sender
    sender: mpsc::UnboundedSender<ReactorMessage>,

    /// Task handles (Arc<DashMap> doesn't support JoinHandle, so we use a simple RwLock wrapper)
    task_handles: Arc<tokio::sync::RwLock<std::collections::HashMap<String, JoinHandle<()>>>>,
}

impl HealthCheckReactor {
    /// Create a new health check reactor
    pub fn new(naming_service: Arc<NamingService>, config: Arc<HealthCheckConfig>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        let reactor = Self {
            naming_service,
            config,
            tasks: Arc::new(DashMap::new()),
            sender,
            task_handles: Arc::new(RwLock::new(HashMap::new())),
        };

        // Start the reactor event loop
        reactor.start_event_loop(receiver);

        reactor
    }

    /// Start the reactor event loop
    fn start_event_loop(&self, mut receiver: mpsc::UnboundedReceiver<ReactorMessage>) {
        let naming_service = self.naming_service.clone();
        let config = self.config.clone();
        let tasks = self.tasks.clone();
        let task_handles = self.task_handles.clone();

        tokio::spawn(async move {
            info!("Health check reactor started");

            while let Some(msg) = receiver.recv().await {
                match msg {
                    ReactorMessage::Schedule { task } => {
                        let task_id = task.get_task_id().to_string();

                        // Cancel existing task if present
                        {
                            let mut handles = task_handles.write().await;
                            if let Some(handle) = handles.remove(&task_id) {
                                handle.abort();
                            }
                        }

                        // Store the task
                        tasks.insert(task_id.clone(), task.clone());

                        // Schedule the task
                        Self::schedule_task_loop(
                            task,
                            naming_service.clone(),
                            config.clone(),
                            tasks.clone(),
                            task_handles.clone(),
                        )
                        .await;
                    }
                    ReactorMessage::Cancel { task_id } => {
                        // Cancel the task
                        {
                            let mut handles = task_handles.write().await;
                            if let Some(handle) = handles.remove(&task_id) {
                                handle.abort();
                            }
                        }
                        tasks.remove(&task_id);
                        debug!("Cancelled health check task: {}", task_id);
                    }
                    ReactorMessage::Shutdown => {
                        // Cancel all tasks
                        let mut handles = task_handles.write().await;
                        for handle in handles.values_mut() {
                            handle.abort();
                        }
                        handles.clear();
                        tasks.clear();
                        info!("Health check reactor shutdown");
                        break;
                    }
                }
            }
        });
    }

    /// Schedule a task loop (matches Nacos scheduleCheck)
    async fn schedule_task_loop(
        task: HealthCheckTask,
        _naming_service: Arc<NamingService>,
        _config: Arc<HealthCheckConfig>,
        tasks: Arc<DashMap<String, HealthCheckTask>>,
        task_handles: Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
    ) {
        let task_id = task.get_task_id().to_string();
        let task_id_clone = task_id.clone();

        let handle = tokio::spawn(async move {
            let mut task = task;

            loop {
                // Perform health check
                let result = match task.get_check_type() {
                    HealthCheckType::Tcp => task.do_check(&TcpHealthCheckProcessor::new()).await,
                    HealthCheckType::Http => task.do_check(&HttpHealthCheckProcessor::new()).await,
                    HealthCheckType::None => task.do_check(&NoneHealthCheckProcessor::new()).await,
                };

                debug!(
                    "Health check result for {}: success={}, time={}ms",
                    task.get_task_id(),
                    result.success,
                    result.response_time_ms
                );

                // Get next check interval (adaptive)
                let check_interval = task.get_check_rt_normalized();

                // Check if task is still active
                if tasks.get(&task_id_clone).is_none() {
                    debug!("Task {} cancelled, exiting loop", task_id_clone);
                    break;
                }

                // Update task in map
                tasks.insert(task_id_clone.clone(), task.clone());

                // Wait for next check
                tokio::time::sleep(check_interval).await;
            }
        });

        // Store handle
        let mut handles = task_handles.write().await;
        handles.insert(task_id, handle);
    }

    /// Schedule a health check task for an instance
    pub fn schedule_check(&self, task: HealthCheckTask) {
        if !self.config.is_enabled() {
            debug!(
                "Health check disabled, skipping task: {}",
                task.get_task_id()
            );
            return;
        }

        let task_id = task.get_task_id().to_string();
        debug!("Scheduling health check task: {}", task_id);

        let _ = self.sender.send(ReactorMessage::Schedule { task });
    }

    /// Cancel a health check task
    pub fn cancel_check(&self, task_id: &str) {
        debug!("Cancelling health check task: {}", task_id);
        let _ = self.sender.send(ReactorMessage::Cancel {
            task_id: task_id.to_string(),
        });
    }

    /// Schedule health checks for all instances (called on service registration)
    pub fn schedule_instance_checks(&self, namespace: &str, group_name: &str, service_name: &str) {
        if !self.config.is_enabled() {
            return;
        }

        // Get all instances for this service
        let instances =
            self.naming_service
                .get_instances(namespace, group_name, service_name, "", false);

        for instance in instances {
            // Skip disabled instances
            if !instance.enabled {
                continue;
            }

            // Get cluster configuration
            let cluster_config = self
                .naming_service
                .get_cluster_config(namespace, group_name, service_name, &instance.cluster_name)
                .unwrap_or_else(|| ClusterConfig {
                    name: instance.cluster_name.clone(),
                    ..Default::default()
                });

            // Skip if health check is disabled for this cluster
            if cluster_config.health_check_type.to_uppercase() == "NONE" {
                continue;
            }

            // Create task
            let task = HealthCheckTask::new(
                instance,
                namespace.to_string(),
                group_name.to_string(),
                service_name.to_string(),
                cluster_config,
                self.config.clone(),
                self.naming_service.clone(),
            );

            // Schedule the task
            self.schedule_check(task);
        }
    }

    /// Cancel health checks for an instance
    pub fn cancel_instance_checks(
        &self,
        _namespace: &str,
        _group_name: &str,
        _service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) {
        let task_id = format!("{}:{}:{}", ip, port, cluster_name);
        self.cancel_check(&task_id);
    }

    /// Shutdown the reactor
    pub async fn shutdown(&self) {
        let _ = self.sender.send(ReactorMessage::Shutdown);

        // Wait a bit for tasks to finish
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    /// Get number of active tasks
    pub fn get_active_task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Get all task IDs
    pub fn get_all_task_ids(&self) -> Vec<String> {
        self.tasks.iter().map(|entry| entry.key().clone()).collect()
    }
}

impl Drop for HealthCheckReactor {
    fn drop(&mut self) {
        // Send shutdown message when reactor is dropped
        let _ = self.sender.send(ReactorMessage::Shutdown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_reactor_creation() {
        let naming_service = Arc::new(NamingService::default());
        let config = Arc::new(HealthCheckConfig::default());
        let reactor = HealthCheckReactor::new(naming_service, config);
        assert_eq!(reactor.get_active_task_count(), 0);
        assert!(reactor.get_all_task_ids().is_empty());
    }
}
