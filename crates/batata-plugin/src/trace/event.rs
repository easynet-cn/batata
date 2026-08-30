use serde::{Deserialize, Serialize};

/// Kinds of trace events. Subscribers declare which kinds they care about;
/// an empty list means "all kinds".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TraceEventKind {
    /// An instance was registered.
    RegisterInstance,
    /// An instance was deregistered.
    DeregisterInstance,
    /// An instance was updated.
    UpdateInstance,
    /// Multiple instances were registered in a batch.
    BatchRegisterInstance,
    /// An instance's health state changed.
    HealthStateChange,
    /// A service was registered.
    RegisterService,
    /// A service was deregistered.
    DeregisterService,
    /// A service was updated.
    UpdateService,
    /// A client subscribed to a service.
    SubscribeService,
    /// A client unsubscribed from a service.
    UnsubscribeService,
    /// A service push was delivered to a client.
    PushService,
    /// A config was published.
    ConfigPublish,
    /// A config was removed.
    ConfigRemove,
    /// A user logged in.
    AuthLogin,
}

/// A trace event fired by the server.
///
/// Variants mirror Nacos' `*TraceEvent` class hierarchy. All variants carry a
/// timestamp (ms since epoch) and the namespace/group/name triple that Nacos
/// stamps on every `TraceEvent`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraceEvent {
    /// An instance was registered.
    RegisterInstance {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Name of the service the instance belongs to.
        service: String,
        /// IP address of the client that triggered the event.
        client_ip: String,
        /// IP address of the registered instance.
        instance_ip: String,
        /// Port of the registered instance.
        instance_port: u16,
    },
    /// An instance was deregistered.
    DeregisterInstance {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Name of the service the instance belongs to.
        service: String,
        /// IP address of the client that triggered the event.
        client_ip: String,
        /// IP address of the deregistered instance.
        instance_ip: String,
        /// Port of the deregistered instance.
        instance_port: u16,
        /// Reason for the deregistration.
        reason: String,
    },
    /// An instance was updated.
    UpdateInstance {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Name of the service the instance belongs to.
        service: String,
        /// IP address of the client that triggered the event.
        client_ip: String,
        /// IP address of the updated instance.
        instance_ip: String,
        /// Port of the updated instance.
        instance_port: u16,
    },
    /// Multiple instances were registered in a batch.
    BatchRegisterInstance {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Name of the service the instances belong to.
        service: String,
        /// IP address of the client that triggered the event.
        client_ip: String,
        /// Number of instances in the batch.
        instance_count: usize,
    },
    /// An instance's health state changed.
    HealthStateChange {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Name of the service the instance belongs to.
        service: String,
        /// IP address of the affected instance.
        instance_ip: String,
        /// Port of the affected instance.
        instance_port: u16,
        /// Whether the instance is now healthy.
        healthy: bool,
    },
    /// A service was registered.
    RegisterService {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Name of the registered service.
        service: String,
    },
    /// A service was deregistered.
    DeregisterService {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Name of the deregistered service.
        service: String,
    },
    /// A service was updated.
    UpdateService {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Name of the updated service.
        service: String,
    },
    /// A client subscribed to a service.
    SubscribeService {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Name of the subscribed service.
        service: String,
        /// IP address of the subscribing client.
        client_ip: String,
    },
    /// A client unsubscribed from a service.
    UnsubscribeService {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Name of the unsubscribed service.
        service: String,
        /// IP address of the unsubscribing client.
        client_ip: String,
    },
    /// A service push was delivered to a client.
    PushService {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Name of the pushed service.
        service: String,
        /// IP address of the client that received the push.
        client_ip: String,
        /// Cost of the push in milliseconds.
        push_cost_ms: u64,
    },
    /// A config was published.
    ConfigPublish {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Config data ID that was published.
        data_id: String,
        /// IP address of the client that published the config.
        client_ip: String,
    },
    /// A config was removed.
    ConfigRemove {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Namespace of the affected resource.
        namespace: String,
        /// Group of the affected resource.
        group: String,
        /// Config data ID that was removed.
        data_id: String,
        /// IP address of the client that removed the config.
        client_ip: String,
    },
    /// A user logged in.
    AuthLogin {
        /// Event timestamp in milliseconds since the Unix epoch.
        event_time: i64,
        /// Username used to log in.
        username: String,
        /// IP address of the client that logged in.
        client_ip: String,
        /// Whether the login succeeded.
        success: bool,
    },
}

impl TraceEvent {
    /// Returns the [`TraceEventKind`] of this event.
    pub fn kind(&self) -> TraceEventKind {
        match self {
            TraceEvent::RegisterInstance { .. } => TraceEventKind::RegisterInstance,
            TraceEvent::DeregisterInstance { .. } => TraceEventKind::DeregisterInstance,
            TraceEvent::UpdateInstance { .. } => TraceEventKind::UpdateInstance,
            TraceEvent::BatchRegisterInstance { .. } => TraceEventKind::BatchRegisterInstance,
            TraceEvent::HealthStateChange { .. } => TraceEventKind::HealthStateChange,
            TraceEvent::RegisterService { .. } => TraceEventKind::RegisterService,
            TraceEvent::DeregisterService { .. } => TraceEventKind::DeregisterService,
            TraceEvent::UpdateService { .. } => TraceEventKind::UpdateService,
            TraceEvent::SubscribeService { .. } => TraceEventKind::SubscribeService,
            TraceEvent::UnsubscribeService { .. } => TraceEventKind::UnsubscribeService,
            TraceEvent::PushService { .. } => TraceEventKind::PushService,
            TraceEvent::ConfigPublish { .. } => TraceEventKind::ConfigPublish,
            TraceEvent::ConfigRemove { .. } => TraceEventKind::ConfigRemove,
            TraceEvent::AuthLogin { .. } => TraceEventKind::AuthLogin,
        }
    }

    /// Returns the event timestamp in milliseconds since the Unix epoch.
    pub fn event_time(&self) -> i64 {
        match self {
            TraceEvent::RegisterInstance { event_time, .. }
            | TraceEvent::DeregisterInstance { event_time, .. }
            | TraceEvent::UpdateInstance { event_time, .. }
            | TraceEvent::BatchRegisterInstance { event_time, .. }
            | TraceEvent::HealthStateChange { event_time, .. }
            | TraceEvent::RegisterService { event_time, .. }
            | TraceEvent::DeregisterService { event_time, .. }
            | TraceEvent::UpdateService { event_time, .. }
            | TraceEvent::SubscribeService { event_time, .. }
            | TraceEvent::UnsubscribeService { event_time, .. }
            | TraceEvent::PushService { event_time, .. }
            | TraceEvent::ConfigPublish { event_time, .. }
            | TraceEvent::ConfigRemove { event_time, .. }
            | TraceEvent::AuthLogin { event_time, .. } => *event_time,
        }
    }

    /// Returns the namespace of the event, or an empty string if not applicable.
    pub fn namespace(&self) -> &str {
        match self {
            TraceEvent::RegisterInstance { namespace, .. }
            | TraceEvent::DeregisterInstance { namespace, .. }
            | TraceEvent::UpdateInstance { namespace, .. }
            | TraceEvent::BatchRegisterInstance { namespace, .. }
            | TraceEvent::HealthStateChange { namespace, .. }
            | TraceEvent::RegisterService { namespace, .. }
            | TraceEvent::DeregisterService { namespace, .. }
            | TraceEvent::UpdateService { namespace, .. }
            | TraceEvent::SubscribeService { namespace, .. }
            | TraceEvent::UnsubscribeService { namespace, .. }
            | TraceEvent::PushService { namespace, .. }
            | TraceEvent::ConfigPublish { namespace, .. }
            | TraceEvent::ConfigRemove { namespace, .. } => namespace,
            TraceEvent::AuthLogin { .. } => "",
        }
    }
}
