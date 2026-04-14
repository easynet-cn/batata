use serde::{Deserialize, Serialize};

/// Kinds of trace events. Subscribers declare which kinds they care about;
/// an empty list means "all kinds".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TraceEventKind {
    RegisterInstance,
    DeregisterInstance,
    UpdateInstance,
    BatchRegisterInstance,
    HealthStateChange,
    RegisterService,
    DeregisterService,
    UpdateService,
    SubscribeService,
    UnsubscribeService,
    PushService,
    ConfigPublish,
    ConfigRemove,
    AuthLogin,
}

/// A trace event fired by the server.
///
/// Variants mirror Nacos' `*TraceEvent` class hierarchy. All variants carry a
/// timestamp (ms since epoch) and the namespace/group/name triple that Nacos
/// stamps on every `TraceEvent`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraceEvent {
    RegisterInstance {
        event_time: i64,
        namespace: String,
        group: String,
        service: String,
        client_ip: String,
        instance_ip: String,
        instance_port: u16,
    },
    DeregisterInstance {
        event_time: i64,
        namespace: String,
        group: String,
        service: String,
        client_ip: String,
        instance_ip: String,
        instance_port: u16,
        reason: String,
    },
    UpdateInstance {
        event_time: i64,
        namespace: String,
        group: String,
        service: String,
        client_ip: String,
        instance_ip: String,
        instance_port: u16,
    },
    BatchRegisterInstance {
        event_time: i64,
        namespace: String,
        group: String,
        service: String,
        client_ip: String,
        instance_count: usize,
    },
    HealthStateChange {
        event_time: i64,
        namespace: String,
        group: String,
        service: String,
        instance_ip: String,
        instance_port: u16,
        healthy: bool,
    },
    RegisterService {
        event_time: i64,
        namespace: String,
        group: String,
        service: String,
    },
    DeregisterService {
        event_time: i64,
        namespace: String,
        group: String,
        service: String,
    },
    UpdateService {
        event_time: i64,
        namespace: String,
        group: String,
        service: String,
    },
    SubscribeService {
        event_time: i64,
        namespace: String,
        group: String,
        service: String,
        client_ip: String,
    },
    UnsubscribeService {
        event_time: i64,
        namespace: String,
        group: String,
        service: String,
        client_ip: String,
    },
    PushService {
        event_time: i64,
        namespace: String,
        group: String,
        service: String,
        client_ip: String,
        push_cost_ms: u64,
    },
    ConfigPublish {
        event_time: i64,
        namespace: String,
        group: String,
        data_id: String,
        client_ip: String,
    },
    ConfigRemove {
        event_time: i64,
        namespace: String,
        group: String,
        data_id: String,
        client_ip: String,
    },
    AuthLogin {
        event_time: i64,
        username: String,
        client_ip: String,
        success: bool,
    },
}

impl TraceEvent {
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
