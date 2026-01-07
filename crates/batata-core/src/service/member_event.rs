// Member change event handling
// Provides event-driven notifications for cluster membership changes

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, broadcast, mpsc};
use tracing::{debug, info};

use batata_api::model::{Member, NodeState};

/// Type of member change event
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MemberChangeType {
    /// A new member joined the cluster
    MemberJoin,
    /// A member left the cluster
    MemberLeave,
    /// A member's state changed (e.g., UP -> DOWN)
    MemberStateChange,
    /// Member metadata was updated
    MemberMetaUpdate,
}

impl std::fmt::Display for MemberChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemberChangeType::MemberJoin => write!(f, "MEMBER_JOIN"),
            MemberChangeType::MemberLeave => write!(f, "MEMBER_LEAVE"),
            MemberChangeType::MemberStateChange => write!(f, "MEMBER_STATE_CHANGE"),
            MemberChangeType::MemberMetaUpdate => write!(f, "MEMBER_META_UPDATE"),
        }
    }
}

/// Member change event
#[derive(Clone, Debug)]
pub struct MemberChangeEvent {
    /// Type of change
    pub change_type: MemberChangeType,
    /// The affected member
    pub member: Member,
    /// Previous state (for state changes)
    pub previous_state: Option<NodeState>,
    /// Timestamp of the event
    pub timestamp: i64,
}

impl MemberChangeEvent {
    /// Create a new member join event
    pub fn member_join(member: Member) -> Self {
        Self {
            change_type: MemberChangeType::MemberJoin,
            member,
            previous_state: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Create a new member leave event
    pub fn member_leave(member: Member) -> Self {
        Self {
            change_type: MemberChangeType::MemberLeave,
            member,
            previous_state: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Create a new member state change event
    pub fn member_state_change(member: Member, previous_state: NodeState) -> Self {
        Self {
            change_type: MemberChangeType::MemberStateChange,
            member,
            previous_state: Some(previous_state),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Create a new member meta update event
    pub fn member_meta_update(member: Member) -> Self {
        Self {
            change_type: MemberChangeType::MemberMetaUpdate,
            member,
            previous_state: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Trait for handling member change events
#[tonic::async_trait]
pub trait MemberChangeListener: Send + Sync {
    /// Called when a member change event occurs
    async fn on_member_change(&self, event: &MemberChangeEvent);
}

/// Member change event publisher
/// Manages subscriptions and broadcasts events to listeners
pub struct MemberChangeEventPublisher {
    /// Broadcast sender for events
    broadcast_tx: broadcast::Sender<MemberChangeEvent>,
    /// Channel for async event submission
    event_tx: mpsc::Sender<MemberChangeEvent>,
    /// Registered listeners
    listeners: Arc<RwLock<Vec<Arc<dyn MemberChangeListener>>>>,
    /// Whether the publisher is running
    running: Arc<RwLock<bool>>,
}

impl MemberChangeEventPublisher {
    /// Create a new event publisher
    pub fn new(queue_size: usize) -> Self {
        let (broadcast_tx, _) = broadcast::channel(queue_size);
        let (event_tx, _event_rx) = mpsc::channel(queue_size);

        Self {
            broadcast_tx,
            event_tx,
            listeners: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the event publisher
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            return;
        }
        *running = true;
        info!("Starting member change event publisher");
    }

    /// Stop the event publisher
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopped member change event publisher");
    }

    /// Register a listener for member change events
    pub async fn register_listener(&self, listener: Arc<dyn MemberChangeListener>) {
        let mut listeners = self.listeners.write().await;
        listeners.push(listener);
        debug!(
            "Registered member change listener, total: {}",
            listeners.len()
        );
    }

    /// Publish a member change event
    pub async fn publish(&self, event: MemberChangeEvent) {
        let is_running = *self.running.read().await;
        if !is_running {
            return;
        }

        info!(
            "Publishing member change event: {} for {}",
            event.change_type, event.member.address
        );

        // Broadcast to subscribers
        let _ = self.broadcast_tx.send(event.clone());

        // Notify listeners
        let listeners = self.listeners.read().await;
        for listener in listeners.iter() {
            listener.on_member_change(&event).await;
        }
    }

    /// Subscribe to member change events
    pub fn subscribe(&self) -> broadcast::Receiver<MemberChangeEvent> {
        self.broadcast_tx.subscribe()
    }

    /// Get event sender for async publishing
    pub fn get_sender(&self) -> mpsc::Sender<MemberChangeEvent> {
        self.event_tx.clone()
    }
}

/// A simple logging listener for debugging
pub struct LoggingMemberChangeListener;

#[tonic::async_trait]
impl MemberChangeListener for LoggingMemberChangeListener {
    async fn on_member_change(&self, event: &MemberChangeEvent) {
        match event.change_type {
            MemberChangeType::MemberJoin => {
                info!(
                    "[MemberEvent] Member joined: {} (state: {})",
                    event.member.address, event.member.state
                );
            }
            MemberChangeType::MemberLeave => {
                info!("[MemberEvent] Member left: {}", event.member.address);
            }
            MemberChangeType::MemberStateChange => {
                if let Some(prev_state) = &event.previous_state {
                    info!(
                        "[MemberEvent] Member state changed: {} ({} -> {})",
                        event.member.address, prev_state, event.member.state
                    );
                }
            }
            MemberChangeType::MemberMetaUpdate => {
                debug!(
                    "[MemberEvent] Member meta updated: {}",
                    event.member.address
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_api::model::MemberBuilder;

    #[tokio::test]
    async fn test_event_publisher() {
        let publisher = MemberChangeEventPublisher::new(100);
        publisher.start().await;

        let mut receiver = publisher.subscribe();

        let member = MemberBuilder::new("127.0.0.1".to_string(), 8848).build();
        let event = MemberChangeEvent::member_join(member);

        publisher.publish(event.clone()).await;

        let received = receiver.try_recv();
        assert!(received.is_ok());
        assert_eq!(received.unwrap().change_type, MemberChangeType::MemberJoin);
    }

    #[test]
    fn test_event_creation() {
        let member = MemberBuilder::new("127.0.0.1".to_string(), 8848).build();

        let join_event = MemberChangeEvent::member_join(member.clone());
        assert_eq!(join_event.change_type, MemberChangeType::MemberJoin);

        let leave_event = MemberChangeEvent::member_leave(member.clone());
        assert_eq!(leave_event.change_type, MemberChangeType::MemberLeave);

        let state_event = MemberChangeEvent::member_state_change(member.clone(), NodeState::Up);
        assert_eq!(state_event.change_type, MemberChangeType::MemberStateChange);
        assert!(state_event.previous_state.is_some());
    }
}
