//! Health check reactor - schedules and manages health check tasks
//!
//! This module provides the HealthCheckReactor that matches Nacos HealthCheckReactor,
//! responsible for scheduling health check tasks for all instances.

use super::config::HealthCheckConfig;
use super::interceptor::HealthCheckInterceptorChain;
use super::processor::{
    HealthCheckType, HttpHealthCheckProcessor, NoneHealthCheckProcessor, TcpHealthCheckProcessor,
};
use super::registry::InstanceCheckRegistry;
use super::registry_task::RegistryCheckTask;
use super::task::HealthCheckTask;
use crate::service::{ClusterConfig, NamingService};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Bounded capacity for the reactor message channel. Large enough to absorb
/// cluster-wide registration bursts (rolling upgrades, mass joins) but small
/// enough that a stuck event loop applies backpressure instead of OOMing.
const REACTOR_CHANNEL_CAPACITY: usize = 4096;

/// Health check reactor message
#[derive(Debug)]
pub enum ReactorMessage {
    /// Schedule a new health check task
    Schedule {
        /// The task to schedule.
        task: Box<HealthCheckTask>,
    },
    /// Cancel a health check task
    Cancel {
        /// The identifier of the task to cancel.
        task_id: String,
    },
    /// Schedule a registry-driven check
    ScheduleRegistryCheck {
        /// The registry check key to schedule.
        check_key: String,
    },
    /// Shutdown the reactor
    Shutdown,
}

/// Health check reactor (matches Nacos HealthCheckReactor)
///
/// The reactor is responsible for:
/// - Scheduling health check tasks for all instances
/// - Managing task lifecycle (create, update, cancel)
/// - Adaptive check interval management
/// - Scheduling registry-driven checks for unified health system
///
/// The reactor is protocol-agnostic: it does not require a `NamingService`.
/// That service is only consulted by `schedule_instance_checks()`, the
/// Nacos-specific path that scans registered instances. Facades that model
/// their own health checks (e.g. the Consul plugin) construct the reactor with
/// [`HealthCheckReactor::new_registry_only`] and drive it entirely through
/// `set_registry()` + `schedule_registry_check()`.
pub struct HealthCheckReactor {
    /// Naming service for accessing instances.
    ///
    /// `None` for registry-only reactors: no Nacos instance scan is possible
    /// (and none is wanted) in that mode.
    naming_service: Option<Arc<NamingService>>,

    /// Health check configuration
    config: Arc<HealthCheckConfig>,

    /// Active task IDs. Presence = task is alive; the per-task state lives
    /// inside the spawned loop, so we deliberately don't store it here — that
    /// avoids cloning a full HealthCheckTask on every tick.
    tasks: Arc<DashMap<Arc<str>, ()>>,

    /// Message sender. Bounded — bursts beyond capacity are dropped with a warning
    /// to protect the reactor from runaway queue growth during cluster events.
    sender: mpsc::Sender<ReactorMessage>,

    /// Task handles for cancellation
    task_handles: Arc<DashMap<Arc<str>, JoinHandle<()>>>,

    /// Optional registry for unified health checks.
    /// Wrapped in Arc<RwLock<>> so the event loop always sees the latest registry
    /// even when `set_registry()` is called after the event loop has started.
    registry: Arc<RwLock<Option<Arc<InstanceCheckRegistry>>>>,

    /// Interceptor chain for cluster-aware health check execution.
    /// Wrapped in Arc<RwLock<>> so the event loop always sees the latest chain
    /// even after `upgrade_to_cluster()` replaces it.
    interceptor_chain: Arc<RwLock<Arc<HealthCheckInterceptorChain>>>,
}

impl HealthCheckReactor {
    /// Create a new health check reactor backed by a Nacos `NamingService`.
    ///
    /// Use this when the reactor must scan registered Nacos instances
    /// (`schedule_instance_checks`). See [`Self::new_registry_only`] for the
    /// protocol-agnostic variant.
    pub fn new(naming_service: Arc<NamingService>, config: Arc<HealthCheckConfig>) -> Self {
        Self::with_optional_naming_service(Some(naming_service), config)
    }

    /// Create a reactor that is driven purely by the check registry.
    ///
    /// Facades that own their own health-check model (the Consul plugin, for
    /// example) have no Nacos instances to scan: their checks live in an
    /// `InstanceCheckRegistry` and are scheduled through
    /// `schedule_registry_check()`. Passing a dummy `NamingService` here would
    /// silently produce an always-empty scan, so the dependency is simply not
    /// taken.
    pub fn new_registry_only(config: Arc<HealthCheckConfig>) -> Self {
        Self::with_optional_naming_service(None, config)
    }

    fn with_optional_naming_service(
        naming_service: Option<Arc<NamingService>>,
        config: Arc<HealthCheckConfig>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(REACTOR_CHANNEL_CAPACITY);

        let standalone_chain = Arc::new(HealthCheckInterceptorChain::standalone(config.clone()));
        let reactor = Self {
            naming_service,
            config,
            tasks: Arc::new(DashMap::new()),
            sender,
            task_handles: Arc::new(DashMap::new()),
            registry: Arc::new(RwLock::new(None)),
            interceptor_chain: Arc::new(RwLock::new(standalone_chain)),
        };

        // Start the reactor event loop
        reactor.start_event_loop(receiver);

        reactor
    }

    /// Set the registry for unified health checks.
    /// Can be called after the event loop has started — the loop reads through the RwLock.
    pub fn set_registry(&self, registry: Arc<InstanceCheckRegistry>) {
        *self.registry.write() = Some(registry);
    }

    /// Replace the interceptor chain (e.g., upgrading from standalone to cluster mode).
    /// Must be called **before** health check tasks are scheduled.
    pub fn set_interceptor_chain(&self, chain: Arc<HealthCheckInterceptorChain>) {
        *self.interceptor_chain.write() = chain;
    }

    /// Start the reactor event loop
    fn start_event_loop(&self, mut receiver: mpsc::Receiver<ReactorMessage>) {
        let tasks = self.tasks.clone();
        let task_handles = self.task_handles.clone();
        let registry_lock = self.registry.clone();
        let interceptor_chain_lock = self.interceptor_chain.clone();

        tokio::spawn(async move {
            info!("Health check reactor started");

            while let Some(msg) = receiver.recv().await {
                match msg {
                    ReactorMessage::Schedule { task } => {
                        let task = *task;
                        let task_id: Arc<str> = Arc::from(task.get_task_id());

                        // Cancel existing task if present
                        if let Some((_, handle)) = task_handles.remove(task_id.as_ref()) {
                            handle.abort();
                        }

                        // Mark task as active (presence-only, no value clone)
                        tasks.insert(Arc::clone(&task_id), ());

                        // Read the latest interceptor chain (lock released immediately)
                        let chain = interceptor_chain_lock.read().clone();

                        // Schedule the task
                        Self::schedule_task_loop(
                            task,
                            task_id,
                            tasks.clone(),
                            task_handles.clone(),
                            chain,
                        );
                    }
                    ReactorMessage::Cancel { task_id } => {
                        // Cancel the task
                        if let Some((_, handle)) = task_handles.remove(task_id.as_str()) {
                            handle.abort();
                        }
                        tasks.remove(task_id.as_str());
                        debug!("Cancelled health check task: {}", task_id);
                    }
                    ReactorMessage::ScheduleRegistryCheck { check_key } => {
                        let registry = registry_lock.read().clone();
                        if let Some(ref reg) = registry {
                            let reg_clone = reg.clone();
                            let check_key_arc: Arc<str> = Arc::from(check_key.as_str());

                            // Cancel existing handle if present
                            if let Some((_, handle)) = task_handles.remove(check_key_arc.as_ref()) {
                                handle.abort();
                            }

                            // Cluster responsibility gate.
                            //
                            // Every node used to probe every active check, which
                            // multiplied probe traffic by the cluster size. That
                            // was a deliberate trade-off while check status was
                            // purely local: a non-responsible node that skipped
                            // the probe would serve stale status from its own
                            // registry. Status is now replicated through Raft
                            // (`RaftHealthReplicator` + `RegistryApplyHook`), so
                            // the responsible node's probe result reaches every
                            // node and gating is safe.
                            //
                            // The shard key is the *probed target address*
                            // (`responsible_id()` → `ip:port`), never the check
                            // id: several checks can point at the same backend,
                            // and hashing by check id would spread them across
                            // different nodes, making each node's aggregate a
                            // partial view.
                            //
                            // Note this is a batata deviation from upstream
                            // Consul, where each agent probes its own local
                            // checks and nothing is sharded. Batata has no
                            // agent/server split, so without a gate the same
                            // backend would be probed N times per interval.
                            let chain_lock = Arc::clone(&interceptor_chain_lock);

                            let handle = tokio::spawn(async move {
                                let task = RegistryCheckTask::new(check_key, reg_clone);
                                loop {
                                    // `None` means the check was removed from the
                                    // registry while we were sleeping.
                                    let interval = match task.interval() {
                                        Some(interval) => interval,
                                        None => {
                                            debug!(
                                                "Registry check {} removed, stopping",
                                                task.check_key()
                                            );
                                            break;
                                        }
                                    };

                                    // Not responsible → skip this round but keep
                                    // the loop alive: membership can change and
                                    // responsibility can move to this node.
                                    let responsible_id = task.responsible_id();
                                    let should_probe =
                                        chain_lock.read().should_execute(&responsible_id);
                                    if !should_probe {
                                        debug!(
                                            "Registry check {} skipped: node not responsible for {}",
                                            task.check_key(),
                                            responsible_id
                                        );
                                        tokio::time::sleep(interval).await;
                                        continue;
                                    }

                                    task.execute().await;

                                    tokio::time::sleep(interval).await;
                                }
                            });

                            task_handles.insert(check_key_arc, handle);
                        } else {
                            debug!(
                                "Registry not set, skipping ScheduleRegistryCheck for {}",
                                check_key
                            );
                        }
                    }
                    ReactorMessage::Shutdown => {
                        // Cancel all tasks
                        for entry in task_handles.iter() {
                            entry.value().abort();
                        }
                        task_handles.clear();
                        tasks.clear();
                        info!("Health check reactor shutdown");
                        break;
                    }
                }
            }
        });
    }

    /// Schedule a task loop (matches Nacos scheduleCheck).
    ///
    /// Sync because the body only spawns a task and inserts a handle — no
    /// awaits are needed. Previously this was `async fn` and took unused
    /// `naming_service` / `config` Arcs that the caller cloned per schedule
    /// event, producing pure waste.
    fn schedule_task_loop(
        task: HealthCheckTask,
        task_id: Arc<str>,
        tasks: Arc<DashMap<Arc<str>, ()>>,
        task_handles: Arc<DashMap<Arc<str>, JoinHandle<()>>>,
        interceptor_chain: Arc<HealthCheckInterceptorChain>,
    ) {
        let loop_task_id = Arc::clone(&task_id);

        let handle = tokio::spawn(async move {
            let mut task = task;

            loop {
                // Responsibility check: skip if this node is not responsible
                if !interceptor_chain.should_execute(task.get_task_id()) {
                    // Still sleep and re-check — responsibility can change when members change
                    tokio::time::sleep(task.get_check_rt_normalized()).await;
                    if !tasks.contains_key(loop_task_id.as_ref()) {
                        break;
                    }
                    continue;
                }

                // Perform health check
                let result = match task.get_check_type() {
                    HealthCheckType::Tcp => task.do_check(&TcpHealthCheckProcessor::new()).await,
                    HealthCheckType::Http => task.do_check(&HttpHealthCheckProcessor::new()).await,
                    HealthCheckType::None
                    | HealthCheckType::Ttl
                    | HealthCheckType::Grpc
                    | HealthCheckType::Mysql
                    | HealthCheckType::Script
                    | HealthCheckType::Docker => {
                        task.do_check(&NoneHealthCheckProcessor::new()).await
                    }
                };

                debug!(
                    "Health check result for {}: success={}, time={}ms",
                    task.get_task_id(),
                    result.success,
                    result.response_time_ms
                );

                // Get next check interval (adaptive)
                let check_interval = task.get_check_rt_normalized();

                // Check if task is still active — presence-only, no value update.
                // The per-tick task state (consecutive_failures, adaptive
                // interval, etc.) lives in this local `task` binding; we no
                // longer mirror it into `tasks`, which eliminates a full
                // HealthCheckTask clone on every tick.
                if !tasks.contains_key(loop_task_id.as_ref()) {
                    debug!("Task {} cancelled, exiting loop", loop_task_id);
                    break;
                }

                // Wait for next check
                tokio::time::sleep(check_interval).await;
            }
        });

        // Store handle
        task_handles.insert(task_id, handle);
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

        if let Err(err) = self.sender.try_send(ReactorMessage::Schedule {
            task: Box::new(task),
        }) {
            warn!(
                "Reactor channel full or closed, dropping Schedule({}): {}",
                task_id, err
            );
        }
    }

    /// Cancel a health check task
    pub fn cancel_check(&self, task_id: &str) {
        debug!("Cancelling health check task: {}", task_id);
        if let Err(err) = self.sender.try_send(ReactorMessage::Cancel {
            task_id: task_id.to_string(),
        }) {
            warn!(
                "Reactor channel full or closed, dropping Cancel({}): {}",
                task_id, err
            );
        }
    }

    /// Schedule a registry-driven check (for unified health system)
    pub fn schedule_registry_check(&self, check_key: &str) {
        debug!("Scheduling registry check: {}", check_key);
        if let Err(err) = self.sender.try_send(ReactorMessage::ScheduleRegistryCheck {
            check_key: check_key.to_string(),
        }) {
            warn!(
                "Reactor channel full or closed, dropping ScheduleRegistryCheck({}): {}",
                check_key, err
            );
        }
    }

    /// Schedule health checks for all instances (called on service registration)
    ///
    /// This is the Nacos-specific entry point: it scans instances registered in
    /// the `NamingService` and turns each eligible one into a `HealthCheckTask`.
    /// Registry-only reactors (see [`Self::new_registry_only`]) have no
    /// `NamingService` and therefore nothing to scan; calling this on them is a
    /// no-op rather than an error, because callers such as the shared startup
    /// path may not know which flavour they hold.
    pub fn schedule_instance_checks(&self, namespace: &str, group_name: &str, service_name: &str) {
        if !self.config.is_enabled() {
            return;
        }

        let Some(naming_service) = self.naming_service.as_ref() else {
            debug!(
                "Registry-only reactor: skipping Nacos instance scan for {}/{}/{}",
                namespace, group_name, service_name
            );
            return;
        };

        // Snapshot instances for this service (zero-copy Arc clones).
        // We early-skip disabled/persistent instances without paying any
        // value-clone cost; only survivors get deep-cloned into HealthCheckTask.
        let snapshot =
            naming_service.get_instances_snapshot(namespace, group_name, service_name, "", false);

        for arc in snapshot {
            // Skip disabled instances
            if !arc.enabled {
                continue;
            }

            // Skip persistent instances — their health state is authoritative
            // from Raft (PersistentInstanceUpdate) and must not be mutated by
            // a local active health probe, which would bypass consensus and
            // create split views across the cluster. Ephemeral instances
            // continue through active checking + heartbeat expiration.
            if !arc.ephemeral {
                continue;
            }

            // Get cluster configuration
            let cluster_config = naming_service
                .get_cluster_config(namespace, group_name, service_name, &arc.cluster_name)
                .unwrap_or_else(|| ClusterConfig {
                    name: arc.cluster_name.clone(),
                    ..Default::default()
                });

            // Skip if health check is disabled for this cluster
            if cluster_config.health_check_type.to_uppercase() == "NONE" {
                continue;
            }

            // Create task (clone the Instance once, for survivors only).
            let instance = (*arc).clone();
            let task = HealthCheckTask::new(
                instance,
                namespace.to_string(),
                group_name.to_string(),
                service_name.to_string(),
                cluster_config,
                self.config.clone(),
                naming_service.clone(),
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
        let _ = self.sender.send(ReactorMessage::Shutdown).await;

        // Wait a bit for tasks to finish
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    /// Get number of active tasks
    pub fn get_active_task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Get all task IDs
    pub fn get_all_task_ids(&self) -> Vec<String> {
        self.tasks
            .iter()
            .map(|entry| entry.key().as_ref().to_owned())
            .collect()
    }
}

impl Drop for HealthCheckReactor {
    fn drop(&mut self) {
        // Best-effort non-blocking shutdown signal; if the channel is full or
        // closed the event loop will exit on its own once the sender drops.
        let _ = self.sender.try_send(ReactorMessage::Shutdown);
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

    #[tokio::test]
    async fn test_registry_only_reactor_holds_no_naming_service() {
        let config = Arc::new(HealthCheckConfig::default());
        let reactor = HealthCheckReactor::new_registry_only(config);
        assert!(reactor.naming_service.is_none());
    }

    #[tokio::test]
    async fn test_registry_only_reactor_ignores_nacos_instance_scan() {
        let config = Arc::new(HealthCheckConfig::default());
        let reactor = HealthCheckReactor::new_registry_only(config);
        // The Nacos-specific scan must be a no-op instead of panicking: shared
        // startup paths may call it without knowing which reactor they hold.
        reactor.schedule_instance_checks("public", "DEFAULT_GROUP", "svc");
        assert_eq!(reactor.get_active_task_count(), 0);
    }

    #[tokio::test]
    async fn test_reactor_schedule_and_cancel() {
        let naming_service = Arc::new(NamingService::default());
        let config = Arc::new(HealthCheckConfig::default());
        let reactor = HealthCheckReactor::new(naming_service.clone(), config);

        // Schedule a check via send message
        reactor.cancel_check("nonexistent"); // Should not panic

        // Verify no tasks
        assert_eq!(reactor.get_active_task_count(), 0);
    }

    #[tokio::test]
    async fn test_reactor_shutdown() {
        let naming_service = Arc::new(NamingService::default());
        let config = Arc::new(HealthCheckConfig::default());
        let reactor = HealthCheckReactor::new(naming_service, config);

        reactor.shutdown().await;
        assert_eq!(
            reactor.get_active_task_count(),
            0,
            "All tasks should be cleared after shutdown"
        );
    }

    #[tokio::test]
    async fn test_reactor_schedule_registry_check_without_registry() {
        let naming_service = Arc::new(NamingService::default());
        let config = Arc::new(HealthCheckConfig::default());
        let reactor = HealthCheckReactor::new(naming_service, config);

        // Schedule registry check without setting registry — should not panic
        reactor.schedule_registry_check("some-check");

        // Give event loop time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // No crash = success
    }

    /// Reverted 2026-04-15: the previous version of this test asserted that
    /// non-responsible cluster nodes skip probing when a cluster interceptor
    /// chain is set. That gate was removed because per-node registries do
    /// not replicate `update_check_result` through Raft yet, so gating the
    /// probe makes other nodes' health responses go stale. Until status
    /// replication lands, every node probes every active check.
    #[tokio::test]
    #[ignore = "cluster gate removed; needs Raft status replication first"]
    async fn test_reactor_registry_check_respects_cluster_gate() {
        use crate::healthcheck::interceptor::HealthCheckInterceptorChain;
        use crate::healthcheck::registry::*;
        use batata_core::service::distro::DistroMapper;

        let naming_service = Arc::new(NamingService::default());
        let config = Arc::new(HealthCheckConfig::default());
        let reactor = HealthCheckReactor::new(naming_service.clone(), config.clone());

        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(naming_service));

        let check_config = InstanceCheckConfig {
            check_id: "cluster-gate-test".to_string(),
            name: "Cluster gate test".to_string(),
            check_type: CheckType::Tcp,
            namespace: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "svc".to_string(),
            ip: "10.10.10.10".to_string(),
            port: 12345,
            cluster_name: "DEFAULT".to_string(),
            http_url: None,
            // Use 127.0.0.1:19 so if the probe WERE to run it would fail,
            // letting us detect a probe by observing a status change. We
            // assert below that no such change occurs because the gate
            // blocks execution.
            tcp_addr: Some("127.0.0.1:19".to_string()),
            grpc_addr: None,
            db_url: None,
            script: None,
            args: None,
            docker_container_id: None,
            interval: Duration::from_millis(100),
            timeout: Duration::from_millis(200),
            ttl: None,
            success_before_passing: 0,
            failures_before_critical: 0,
            deregister_critical_after: None,
            initial_status: CheckStatus::Passing,
            notes: String::new(),
            service_tags: vec![],
        };
        registry.register_check(check_config);
        reactor.set_registry(registry.clone());

        // Build a cluster chain where the local address is NOT the node
        // responsible for "10.10.10.10:12345".
        let mapper = Arc::new(DistroMapper::new());
        let members = vec![
            "10.0.0.1:8848".to_string(),
            "10.0.0.2:8848".to_string(),
            "10.0.0.3:8848".to_string(),
        ];
        mapper.update_members(members.clone());
        let responsible = mapper
            .responsible_node("10.10.10.10:12345")
            .expect("mapper must pick a responsible node");
        let non_responsible = members
            .iter()
            .find(|m| *m != &responsible)
            .cloned()
            .expect("must find a non-responsible member");

        let chain = Arc::new(HealthCheckInterceptorChain::cluster(
            config,
            mapper,
            non_responsible,
        ));
        reactor.set_interceptor_chain(chain);

        reactor.schedule_registry_check("cluster-gate-test");

        // Let the loop tick several times — if the gate works the probe
        // never executes and the initial Passing status is retained.
        tokio::time::sleep(Duration::from_millis(500)).await;

        let (_, status) = registry
            .get_check("cluster-gate-test")
            .expect("check must still exist");
        assert_eq!(
            status.status,
            CheckStatus::Passing,
            "Non-responsible node must not probe; status must retain initial Passing"
        );
        assert!(
            status.output.is_empty(),
            "No probe ran, so no output should have been recorded: {:?}",
            status.output
        );
    }

    #[tokio::test]
    async fn test_reactor_schedule_registry_check_executes() {
        use crate::healthcheck::registry::*;

        let naming_service = Arc::new(NamingService::default());
        let config = Arc::new(HealthCheckConfig::default());
        let reactor = HealthCheckReactor::new(naming_service.clone(), config);

        // Create registry and set it AFTER reactor creation (reproducing real startup order)
        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(naming_service));

        // Register a TCP check pointing to an unreachable port
        let check_config = InstanceCheckConfig {
            check_id: "reactor-exec-test".to_string(),
            name: "Reactor execution test".to_string(),
            check_type: CheckType::Tcp,
            namespace: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "test-svc".to_string(),
            ip: "127.0.0.1".to_string(),
            port: 19,
            cluster_name: "DEFAULT".to_string(),
            http_url: None,
            tcp_addr: Some("127.0.0.1:19".to_string()), // Port 19 — typically not listening
            grpc_addr: None,
            db_url: None,
            script: None,
            args: None,
            docker_container_id: None,
            interval: Duration::from_millis(100), // Fast interval for test
            timeout: Duration::from_millis(200),
            ttl: None,
            success_before_passing: 0,
            failures_before_critical: 0,
            deregister_critical_after: None,
            initial_status: CheckStatus::Passing,
            notes: String::new(),
            service_tags: vec![],
        };
        registry.register_check(check_config);

        // Set registry on reactor (this is the fix — previously the event loop would never see it)
        reactor.set_registry(registry.clone());

        // Schedule the check
        reactor.schedule_registry_check("reactor-exec-test");

        // Wait for at least one execution cycle
        tokio::time::sleep(Duration::from_millis(600)).await;

        // Verify the check status changed from Passing to Critical
        let (_, status) = registry
            .get_check("reactor-exec-test")
            .expect("Check should still exist");
        assert_eq!(
            status.status,
            CheckStatus::Critical,
            "TCP check to unreachable port should transition to Critical after reactor execution"
        );
        assert!(
            !status.output.is_empty(),
            "Check output should contain failure details"
        );
    }

    /// End-to-end test: register service + TCP check → reactor executes →
    /// status becomes Critical → deregister monitor reaps → service removed.
    #[tokio::test]
    async fn test_end_to_end_check_execute_and_deregister() {
        use crate::healthcheck::deregister_monitor::DeregisterMonitor;
        use crate::healthcheck::registry::*;
        use crate::model::Instance;

        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let reactor = HealthCheckReactor::new(naming_service.clone(), config);

        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(
            naming_service.clone(),
        ));

        // Step 1: Register an instance in the naming service
        let instance = Instance {
            ip: "127.0.0.1".to_string(),
            port: 19, // unreachable port
            cluster_name: "DEFAULT".to_string(),
            service_name: "e2e-svc".to_string(),
            healthy: true,
            enabled: true,
            ephemeral: false,
            ..Default::default()
        };
        naming_service.register_instance("public", "DEFAULT_GROUP", "e2e-svc", instance);

        // Verify instance exists
        let instances =
            naming_service.get_instances("public", "DEFAULT_GROUP", "e2e-svc", "", false);
        assert_eq!(instances.len(), 1, "Instance should be registered");

        // Step 2: Register a TCP check with deregister_critical_after
        let check_config = InstanceCheckConfig {
            check_id: "e2e-tcp-check".to_string(),
            name: "E2E TCP check".to_string(),
            check_type: CheckType::Tcp,
            namespace: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "e2e-svc".to_string(),
            ip: "127.0.0.1".to_string(),
            port: 19,
            cluster_name: "DEFAULT".to_string(),
            http_url: None,
            tcp_addr: Some("127.0.0.1:19".to_string()),
            grpc_addr: None,
            db_url: None,
            script: None,
            args: None,
            docker_container_id: None,
            interval: Duration::from_millis(50),
            timeout: Duration::from_millis(100),
            ttl: None,
            success_before_passing: 0,
            failures_before_critical: 0,
            deregister_critical_after: Some(Duration::from_millis(1)),
            initial_status: CheckStatus::Passing,
            notes: String::new(),
            service_tags: vec![],
        };
        registry.register_check(check_config);

        // Step 3: Wire reactor and schedule the check
        reactor.set_registry(registry.clone());
        reactor.schedule_registry_check("e2e-tcp-check");

        // Step 4: Wait for the check to execute and fail
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Verify check went Critical
        let (_, status) = registry
            .get_check("e2e-tcp-check")
            .expect("Check should still exist");
        assert_eq!(
            status.status,
            CheckStatus::Critical,
            "TCP check should be Critical after failing"
        );

        // Step 5: Run deregister monitor reap
        let monitor = DeregisterMonitor::new(registry.clone(), 30);
        // The check has been Critical for ~400ms, threshold is 1ms → should reap
        monitor.reap_critical_instances();

        // Step 6: Verify instance was deregistered
        let instances =
            naming_service.get_instances("public", "DEFAULT_GROUP", "e2e-svc", "", false);
        assert_eq!(
            instances.len(),
            0,
            "Instance should be deregistered after critical threshold exceeded"
        );

        // Verify check was also removed
        assert!(
            registry.get_check("e2e-tcp-check").is_none(),
            "Check should be removed after deregistration"
        );
    }

    #[tokio::test]
    async fn test_reactor_disabled_config() {
        let naming_service = Arc::new(NamingService::default());
        let mut config = HealthCheckConfig::default();
        config.health_check_enabled = false;
        let config = Arc::new(config);
        let reactor = HealthCheckReactor::new(naming_service.clone(), config);

        // schedule_instance_checks should be a no-op when disabled
        reactor.schedule_instance_checks("public", "DEFAULT_GROUP", "test-svc");

        // Give time to process
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            reactor.get_active_task_count(),
            0,
            "No tasks should be scheduled when disabled"
        );
    }
}
