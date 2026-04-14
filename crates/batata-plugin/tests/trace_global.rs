//! Integration test: prove `global_trace_registry()` is a shared singleton
//! across call sites, so distinct modules firing events reach subscribers
//! registered by startup code.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use batata_plugin::{TraceEvent, TraceEventKind, TraceSubscriber, global_trace_registry};

struct CaptureSubscriber {
    name: String,
    seen: Arc<AtomicUsize>,
}

#[async_trait]
impl TraceSubscriber for CaptureSubscriber {
    fn name(&self) -> &str {
        &self.name
    }
    fn subscribed_kinds(&self) -> &[TraceEventKind] {
        &[]
    }
    async fn on_event(&self, _event: &TraceEvent) {
        self.seen.fetch_add(1, Ordering::SeqCst);
    }
}

fn make_event() -> TraceEvent {
    TraceEvent::ConfigPublish {
        event_time: 42,
        namespace: "public".into(),
        group: "g".into(),
        data_id: "d".into(),
        client_ip: "1.2.3.4".into(),
    }
}

#[tokio::test]
async fn global_registry_is_a_shared_singleton() {
    let seen = Arc::new(AtomicUsize::new(0));
    // Simulate startup registering a subscriber.
    global_trace_registry().register(Arc::new(CaptureSubscriber {
        name: "global_test_singleton".into(),
        seen: seen.clone(),
    }));

    // Simulate an unrelated module fetching the registry and firing.
    global_trace_registry().fire(&make_event()).await;

    assert_eq!(seen.load(Ordering::SeqCst), 1);

    // Clean up so we don't leak into other integration tests.
    global_trace_registry().unregister("global_test_singleton");
}
