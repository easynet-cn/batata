//! Naming gRPC redo service with state machine.
//!
//! Matches Nacos Java `NamingGrpcRedoService` with:
//! - `InstanceRedoData` and `SubscriberRedoData` types with state flags
//! - Scheduled redo task (configurable interval, default 500ms)
//! - Connection event integration (`onDisConnect` / `onConnected`)

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use batata_api::naming::model::Instance;
use dashmap::DashMap;
use tracing::info;

/// Default redo delay interval in milliseconds (matches Nacos `DEFAULT_REDO_DELAY_TIME`).
pub const DEFAULT_REDO_DELAY_MS: u64 = 500;

/// State-tracked data for a registered instance.
///
/// Matches Nacos Java `InstanceRedoData` with three state flags:
/// - `registered`: the instance was successfully registered on the server
/// - `unregistering`: a deregistration is in progress
/// - `expected_registered`: the client expects this instance to be registered
pub struct InstanceRedoData {
    pub namespace: String,
    pub group_name: String,
    pub service_name: String,
    pub instance: Instance,
    /// Whether the server has confirmed registration
    registered: AtomicBool,
    /// Whether a deregistration is in progress
    unregistering: AtomicBool,
    /// Whether the client expects this to be registered
    expected_registered: AtomicBool,
}

impl InstanceRedoData {
    pub fn new(
        namespace: String,
        group_name: String,
        service_name: String,
        instance: Instance,
    ) -> Self {
        Self {
            namespace,
            group_name,
            service_name,
            instance,
            registered: AtomicBool::new(false),
            unregistering: AtomicBool::new(false),
            expected_registered: AtomicBool::new(true),
        }
    }

    /// Whether this instance needs to be re-registered.
    /// `true` if not registered AND expected to be registered AND not currently unregistering.
    pub fn is_need_redo(&self) -> bool {
        !self.registered.load(Ordering::Relaxed)
            && self.expected_registered.load(Ordering::Relaxed)
            && !self.unregistering.load(Ordering::Relaxed)
    }

    /// Whether this instance needs to be unregistered from server.
    /// `true` if registered AND unregistering flag is set.
    pub fn is_need_remove(&self) -> bool {
        self.registered.load(Ordering::Relaxed) && self.unregistering.load(Ordering::Relaxed)
    }

    pub fn set_registered(&self, registered: bool) {
        self.registered.store(registered, Ordering::Relaxed);
    }

    pub fn is_registered(&self) -> bool {
        self.registered.load(Ordering::Relaxed)
    }

    pub fn set_unregistering(&self) {
        self.unregistering.store(true, Ordering::Relaxed);
        self.expected_registered.store(false, Ordering::Relaxed);
    }

    pub fn set_expected_registered(&self, expected: bool) {
        self.expected_registered.store(expected, Ordering::Relaxed);
    }
}

/// State-tracked data for a service subscription.
///
/// Matches Nacos Java `SubscriberRedoData`.
pub struct SubscriberRedoData {
    pub namespace: String,
    pub group_name: String,
    pub service_name: String,
    pub clusters: String,
    /// Whether the server has confirmed the subscription
    registered: AtomicBool,
    /// Whether the client expects this subscription to be active
    expected_registered: AtomicBool,
}

impl SubscriberRedoData {
    pub fn new(
        namespace: String,
        group_name: String,
        service_name: String,
        clusters: String,
    ) -> Self {
        Self {
            namespace,
            group_name,
            service_name,
            clusters,
            registered: AtomicBool::new(false),
            expected_registered: AtomicBool::new(true),
        }
    }

    /// Whether this subscription needs to be re-established.
    pub fn is_need_redo(&self) -> bool {
        !self.registered.load(Ordering::Relaxed) && self.expected_registered.load(Ordering::Relaxed)
    }

    pub fn set_registered(&self, registered: bool) {
        self.registered.store(registered, Ordering::Relaxed);
    }

    pub fn is_registered(&self) -> bool {
        self.registered.load(Ordering::Relaxed)
    }

    pub fn set_expected_registered(&self, expected: bool) {
        self.expected_registered.store(expected, Ordering::Relaxed);
    }
}

/// Redo service that manages state for instance registrations and subscriptions.
///
/// Matches Nacos Java `NamingGrpcRedoService`:
/// - Tracks registered instances and subscriptions with state flags
/// - Runs a periodic redo task to retry failed operations
/// - Responds to connection events by marking all items for redo
pub struct NamingGrpcRedoService {
    /// Registered instance tracking
    registered_instances: DashMap<String, Arc<InstanceRedoData>>,
    /// Subscription tracking
    subscribers: DashMap<String, Arc<SubscriberRedoData>>,
    /// Whether the client is currently connected
    connected: AtomicBool,
    /// Shutdown flag
    shutdown: AtomicBool,
    /// Redo task interval
    redo_delay: Duration,
}

impl NamingGrpcRedoService {
    pub fn new() -> Self {
        Self {
            registered_instances: DashMap::new(),
            subscribers: DashMap::new(),
            connected: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            redo_delay: Duration::from_millis(DEFAULT_REDO_DELAY_MS),
        }
    }

    pub fn with_delay(redo_delay: Duration) -> Self {
        Self {
            registered_instances: DashMap::new(),
            subscribers: DashMap::new(),
            connected: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            redo_delay,
        }
    }

    // --- Instance operations ---

    /// Cache an instance for redo tracking. Called when registering an instance.
    pub fn cache_instance_for_redo(&self, key: String, data: InstanceRedoData) {
        self.registered_instances.insert(key, Arc::new(data));
    }

    /// Mark an instance as successfully registered.
    pub fn instance_registered(&self, key: &str) {
        if let Some(data) = self.registered_instances.get(key) {
            data.set_registered(true);
        }
    }

    /// Mark an instance for deregistration.
    pub fn instance_deregister(&self, key: &str) {
        if let Some(data) = self.registered_instances.get(key) {
            data.set_unregistering();
        }
    }

    /// Remove an instance from redo tracking (after successful deregistration).
    pub fn remove_instance_for_redo(&self, key: &str) {
        self.registered_instances.remove(key);
    }

    /// Find all instances that need redo (re-registration).
    pub fn find_instance_redo_data(&self) -> Vec<Arc<InstanceRedoData>> {
        self.registered_instances
            .iter()
            .filter(|entry| entry.value().is_need_redo())
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Find all instances that need removal (deregistration).
    pub fn find_instance_remove_data(&self) -> Vec<(String, Arc<InstanceRedoData>)> {
        self.registered_instances
            .iter()
            .filter(|entry| entry.value().is_need_remove())
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    // --- Subscriber operations ---

    /// Cache a subscription for redo tracking.
    pub fn cache_subscriber_for_redo(&self, key: String, data: SubscriberRedoData) {
        self.subscribers.insert(key, Arc::new(data));
    }

    /// Mark a subscription as successfully registered.
    pub fn subscriber_registered(&self, key: &str) {
        if let Some(data) = self.subscribers.get(key) {
            data.set_registered(true);
        }
    }

    /// Remove a subscription from redo tracking.
    pub fn remove_subscriber_for_redo(&self, key: &str) {
        self.subscribers.remove(key);
    }

    /// Find all subscriptions that need redo.
    pub fn find_subscriber_redo_data(&self) -> Vec<(String, Arc<SubscriberRedoData>)> {
        self.subscribers
            .iter()
            .filter(|entry| entry.value().is_need_redo())
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    // --- Connection events ---

    /// Called when connection is established.
    /// Enables the redo task to start processing.
    pub fn on_connected(&self) {
        self.connected.store(true, Ordering::Relaxed);
        info!("NamingGrpcRedoService: connected, redo enabled");
    }

    /// Called when connection is lost.
    /// Marks all instances and subscriptions as unregistered so the redo task
    /// will re-establish them when connection is restored.
    pub fn on_disconnected(&self) {
        self.connected.store(false, Ordering::Relaxed);

        // Mark all instances as not registered
        for entry in self.registered_instances.iter() {
            entry.value().set_registered(false);
        }

        // Mark all subscriptions as not registered
        for entry in self.subscribers.iter() {
            entry.value().set_registered(false);
        }

        info!(
            "NamingGrpcRedoService: disconnected, marked {} instances and {} subscribers for redo",
            self.registered_instances.len(),
            self.subscribers.len()
        );
    }

    /// Whether the service is connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Shutdown the redo service.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.registered_instances.clear();
        self.subscribers.clear();
    }

    /// Whether the redo service is shut down.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }

    /// Get the redo delay interval.
    pub fn redo_delay(&self) -> Duration {
        self.redo_delay
    }

    /// Get the count of registered instances.
    pub fn instance_count(&self) -> usize {
        self.registered_instances.len()
    }

    /// Get the count of subscriptions.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}

impl Default for NamingGrpcRedoService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_instance(ip: &str, port: i32) -> Instance {
        Instance {
            ip: ip.to_string(),
            port,
            ..Default::default()
        }
    }

    #[test]
    fn test_instance_redo_data_initial_state() {
        let data = InstanceRedoData::new(
            "public".into(),
            "DEFAULT_GROUP".into(),
            "my-service".into(),
            make_instance("127.0.0.1", 8080),
        );
        // Initially: not registered, expected registered, not unregistering
        assert!(!data.is_registered());
        assert!(data.is_need_redo()); // needs redo
        assert!(!data.is_need_remove());
    }

    #[test]
    fn test_instance_redo_data_after_register() {
        let data = InstanceRedoData::new(
            "public".into(),
            "DEFAULT_GROUP".into(),
            "my-service".into(),
            make_instance("127.0.0.1", 8080),
        );
        data.set_registered(true);
        assert!(data.is_registered());
        assert!(!data.is_need_redo()); // no redo needed
        assert!(!data.is_need_remove());
    }

    #[test]
    fn test_instance_redo_data_disconnect_reconnect() {
        let data = InstanceRedoData::new(
            "public".into(),
            "DEFAULT_GROUP".into(),
            "my-service".into(),
            make_instance("127.0.0.1", 8080),
        );
        data.set_registered(true);
        assert!(!data.is_need_redo());

        // Simulate disconnect
        data.set_registered(false);
        assert!(data.is_need_redo()); // needs redo after disconnect
    }

    #[test]
    fn test_instance_redo_data_deregister() {
        let data = InstanceRedoData::new(
            "public".into(),
            "DEFAULT_GROUP".into(),
            "my-service".into(),
            make_instance("127.0.0.1", 8080),
        );
        data.set_registered(true);

        data.set_unregistering();
        assert!(data.is_need_remove()); // needs removal
        assert!(!data.is_need_redo()); // does NOT need re-registration
    }

    #[test]
    fn test_subscriber_redo_data_initial() {
        let data = SubscriberRedoData::new(
            "public".into(),
            "DEFAULT_GROUP".into(),
            "my-service".into(),
            "".into(),
        );
        assert!(!data.is_registered());
        assert!(data.is_need_redo());
    }

    #[test]
    fn test_subscriber_redo_data_registered() {
        let data = SubscriberRedoData::new(
            "public".into(),
            "DEFAULT_GROUP".into(),
            "my-service".into(),
            "".into(),
        );
        data.set_registered(true);
        assert!(!data.is_need_redo());
    }

    #[test]
    fn test_redo_service_on_disconnect() {
        let service = NamingGrpcRedoService::new();

        let inst_data = InstanceRedoData::new(
            "public".into(),
            "DEFAULT_GROUP".into(),
            "svc-1".into(),
            make_instance("127.0.0.1", 8080),
        );
        inst_data.set_registered(true);
        service.cache_instance_for_redo("key1".into(), inst_data);

        let sub_data = SubscriberRedoData::new(
            "public".into(),
            "DEFAULT_GROUP".into(),
            "svc-1".into(),
            "".into(),
        );
        sub_data.set_registered(true);
        service.cache_subscriber_for_redo("key1".into(), sub_data);

        // Verify nothing needs redo when registered
        assert!(service.find_instance_redo_data().is_empty());
        assert!(service.find_subscriber_redo_data().is_empty());

        // Simulate disconnect
        service.on_disconnected();

        // Now everything needs redo
        assert_eq!(service.find_instance_redo_data().len(), 1);
        assert_eq!(service.find_subscriber_redo_data().len(), 1);
    }

    #[test]
    fn test_redo_service_instance_lifecycle() {
        let service = NamingGrpcRedoService::new();

        // Register
        let data = InstanceRedoData::new(
            "public".into(),
            "DEFAULT_GROUP".into(),
            "svc".into(),
            make_instance("10.0.0.1", 8080),
        );
        service.cache_instance_for_redo("k1".into(), data);
        assert_eq!(service.find_instance_redo_data().len(), 1);

        // Mark as registered
        service.instance_registered("k1");
        assert!(service.find_instance_redo_data().is_empty());

        // Deregister
        service.instance_deregister("k1");
        assert_eq!(service.find_instance_remove_data().len(), 1);

        // Remove
        service.remove_instance_for_redo("k1");
        assert_eq!(service.instance_count(), 0);
    }

    #[test]
    fn test_redo_service_shutdown() {
        let service = NamingGrpcRedoService::new();
        service.cache_instance_for_redo(
            "k1".into(),
            InstanceRedoData::new(
                "ns".into(),
                "g".into(),
                "s".into(),
                make_instance("1.1.1.1", 80),
            ),
        );
        assert_eq!(service.instance_count(), 1);

        service.shutdown();
        assert!(service.is_shutdown());
        assert_eq!(service.instance_count(), 0);
    }

    #[test]
    fn test_redo_service_connected_state() {
        let service = NamingGrpcRedoService::new();
        assert!(!service.is_connected());

        service.on_connected();
        assert!(service.is_connected());

        service.on_disconnected();
        assert!(!service.is_connected());
    }
}
