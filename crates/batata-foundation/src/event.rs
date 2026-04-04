//! Event bus abstraction
//!
//! Provides a typed publish-subscribe event system for internal communication
//! between components. Domain crates define their own event types.

use async_trait::async_trait;

use crate::FoundationError;

/// Event publisher — publishes events to subscribers
#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// Publish an event. The event is serialized to bytes by the caller.
    ///
    /// `topic`: event topic/channel name (e.g., "naming.instance.changed")
    /// `payload`: serialized event data
    async fn publish(&self, topic: &str, payload: &[u8]) -> Result<(), FoundationError>;
}

/// Event subscriber — receives events from a topic
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Subscribe to a topic. Returns a receiver for event payloads.
    async fn subscribe(
        &self,
        topic: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<EventMessage>, FoundationError>;

    /// Unsubscribe from a topic
    async fn unsubscribe(&self, topic: &str) -> Result<(), FoundationError>;
}

/// Event bus — combines publisher and subscriber
#[async_trait]
pub trait EventBus: EventPublisher + EventSubscriber {
    /// Subscribe with a handler function instead of a channel
    async fn on<F>(&self, topic: &str, handler: F) -> Result<SubscriptionId, FoundationError>
    where
        F: Fn(EventMessage) + Send + Sync + 'static;

    /// Remove a handler subscription
    async fn off(&self, id: SubscriptionId) -> Result<(), FoundationError>;
}

/// A received event message
#[derive(Debug, Clone)]
pub struct EventMessage {
    /// Topic this message was published on
    pub topic: String,
    /// Serialized payload
    pub payload: bytes::Bytes,
    /// Timestamp when the event was published
    pub timestamp: i64,
    /// Source node ID (for cluster-level events)
    pub source: Option<String>,
}

/// Unique subscription identifier for cancellation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub u64);
