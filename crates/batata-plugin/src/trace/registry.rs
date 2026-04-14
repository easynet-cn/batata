use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use dashmap::DashMap;

use super::event::{TraceEvent, TraceEventKind};

/// Trait that external trace plugins implement.
///
/// Mirrors Nacos `NacosTraceSubscriber`. An empty `subscribed_kinds()` slice
/// is treated as "subscribed to all kinds", matching Nacos' default-all
/// behavior when `subscribeTypes()` returns the base `TraceEvent.class`.
#[async_trait]
pub trait TraceSubscriber: Send + Sync {
    fn name(&self) -> &str;

    fn subscribed_kinds(&self) -> &[TraceEventKind];

    async fn on_event(&self, event: &TraceEvent);
}

/// Holds all registered [`TraceSubscriber`]s keyed by name. Registering a
/// subscriber whose `name()` collides with an existing one replaces it,
/// matching Nacos `NacosTracePluginManager` which overwrites on name collision.
pub struct TraceSubscriberRegistry {
    subscribers: DashMap<String, Arc<dyn TraceSubscriber>>,
}

impl TraceSubscriberRegistry {
    pub fn new() -> Self {
        Self {
            subscribers: DashMap::new(),
        }
    }

    pub fn register(&self, subscriber: Arc<dyn TraceSubscriber>) {
        let name = subscriber.name().to_string();
        self.subscribers.insert(name, subscriber);
    }

    pub fn unregister(&self, name: &str) -> bool {
        self.subscribers.remove(name).is_some()
    }

    pub fn len(&self) -> usize {
        self.subscribers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.subscribers.is_empty()
    }

    pub fn names(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.subscribers.len());
        for entry in self.subscribers.iter() {
            out.push(entry.key().clone());
        }
        out
    }

    /// Fire an event to every subscriber interested in its kind.
    ///
    /// Subscribers are awaited sequentially so that slow subscribers don't
    /// starve the caller's task scheduler with an unbounded fan-out. Call
    /// sites that cannot afford to block should spawn or use `fire_detached`.
    pub async fn fire(&self, event: &TraceEvent) {
        let kind = event.kind();
        let snapshot: Vec<Arc<dyn TraceSubscriber>> = {
            let mut v = Vec::with_capacity(self.subscribers.len());
            for entry in self.subscribers.iter() {
                v.push(entry.value().clone());
            }
            v
        };
        for sub in snapshot {
            let kinds = sub.subscribed_kinds();
            if kinds.is_empty() || kinds.contains(&kind) {
                sub.on_event(event).await;
            }
        }
    }

    /// Fire-and-forget variant: spawns a task so the caller never waits.
    pub fn fire_detached(self: &Arc<Self>, event: TraceEvent) {
        let me = Arc::clone(self);
        tokio::spawn(async move {
            me.fire(&event).await;
        });
    }
}

impl Default for TraceSubscriberRegistry {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL: OnceLock<Arc<TraceSubscriberRegistry>> = OnceLock::new();

/// Process-wide singleton registry, mirroring Nacos `NacosTracePluginManager.getInstance()`.
///
/// Server startup code should register default + configured subscribers here.
/// Call sites (naming, config, auth) fire events against it without needing
/// to thread a handle through every service.
pub fn global_trace_registry() -> Arc<TraceSubscriberRegistry> {
    GLOBAL
        .get_or_init(|| Arc::new(TraceSubscriberRegistry::new()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingSubscriber {
        name: String,
        kinds: Vec<TraceEventKind>,
        seen: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl TraceSubscriber for CountingSubscriber {
        fn name(&self) -> &str {
            &self.name
        }
        fn subscribed_kinds(&self) -> &[TraceEventKind] {
            &self.kinds
        }
        async fn on_event(&self, _event: &TraceEvent) {
            self.seen.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn sample_register_event() -> TraceEvent {
        TraceEvent::RegisterInstance {
            event_time: 1_700_000_000_000,
            namespace: "public".into(),
            group: "DEFAULT_GROUP".into(),
            service: "svc".into(),
            client_ip: "1.2.3.4".into(),
            instance_ip: "10.0.0.1".into(),
            instance_port: 8080,
        }
    }

    #[tokio::test]
    async fn fire_dispatches_to_matching_kind() {
        let reg = TraceSubscriberRegistry::new();
        let seen = Arc::new(AtomicUsize::new(0));
        reg.register(Arc::new(CountingSubscriber {
            name: "match".into(),
            kinds: vec![TraceEventKind::RegisterInstance],
            seen: seen.clone(),
        }));
        let other_seen = Arc::new(AtomicUsize::new(0));
        reg.register(Arc::new(CountingSubscriber {
            name: "nomatch".into(),
            kinds: vec![TraceEventKind::ConfigPublish],
            seen: other_seen.clone(),
        }));

        reg.fire(&sample_register_event()).await;

        assert_eq!(seen.load(Ordering::SeqCst), 1);
        assert_eq!(other_seen.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn empty_kinds_means_subscribe_all() {
        let reg = TraceSubscriberRegistry::new();
        let seen = Arc::new(AtomicUsize::new(0));
        reg.register(Arc::new(CountingSubscriber {
            name: "all".into(),
            kinds: vec![],
            seen: seen.clone(),
        }));

        reg.fire(&sample_register_event()).await;
        reg.fire(&TraceEvent::AuthLogin {
            event_time: 1,
            username: "u".into(),
            client_ip: "1.1.1.1".into(),
            success: true,
        })
        .await;

        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn register_with_same_name_replaces() {
        let reg = TraceSubscriberRegistry::new();
        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));
        reg.register(Arc::new(CountingSubscriber {
            name: "same".into(),
            kinds: vec![],
            seen: first.clone(),
        }));
        reg.register(Arc::new(CountingSubscriber {
            name: "same".into(),
            kinds: vec![],
            seen: second.clone(),
        }));

        assert_eq!(reg.len(), 1);
        reg.fire(&sample_register_event()).await;
        assert_eq!(first.load(Ordering::SeqCst), 0);
        assert_eq!(second.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn unregister_removes_subscriber() {
        let reg = TraceSubscriberRegistry::new();
        let seen = Arc::new(AtomicUsize::new(0));
        reg.register(Arc::new(CountingSubscriber {
            name: "x".into(),
            kinds: vec![],
            seen: seen.clone(),
        }));
        assert!(reg.unregister("x"));
        assert!(!reg.unregister("x"));
        reg.fire(&sample_register_event()).await;
        assert_eq!(seen.load(Ordering::SeqCst), 0);
    }
}
