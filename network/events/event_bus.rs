use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use log::error;

use crate::network::events::event_types::{EventType, NetworkEvent};

/// Event bus for network events
pub struct EventBus {
    /// Subscribers by event type
    subscribers: Arc<RwLock<HashMap<EventType, Vec<mpsc::Sender<NetworkEvent>>>>>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Publish an event to all subscribers
    pub async fn publish(&self, event: NetworkEvent) {
        let event_type = event.get_type();

        // Get subscribers for this event type and for all events
        let subscribers = {
            let subscribers = self.subscribers.read().await;

            let mut result = Vec::new();

            // Add subscribers for this specific event type
            if let Some(type_subscribers) = subscribers.get(&event_type) {
                result.extend(type_subscribers.clone());
            }

            // Add subscribers for all events
            if let Some(all_subscribers) = subscribers.get(&EventType::All) {
                result.extend(all_subscribers.clone());
            }

            result
        };

        // Send the event to all subscribers
        for subscriber in subscribers {
            if let Err(e) = subscriber.send(event.clone()).await {
                error!("Failed to send event to subscriber: {:?}", e);
            }
        }
    }

    /// Subscribe to events of a specific type
    pub async fn subscribe(&self, event_type: EventType) -> mpsc::Receiver<NetworkEvent> {
        let (tx, rx) = mpsc::channel(100);

        // Add the subscriber
        {
            let mut subscribers = self.subscribers.write().await;
            subscribers.entry(event_type).or_insert_with(Vec::new).push(tx);
        }

        rx
    }

    /// Unsubscribe from events
    pub async fn unsubscribe(&self, event_type: EventType, _rx: &mpsc::Receiver<NetworkEvent>) {
        // Find and remove the subscriber
        let mut subscribers = self.subscribers.write().await;

        if let Some(type_subscribers) = subscribers.get_mut(&event_type) {
            // We can't directly compare receivers, so we'll remove closed channels
            type_subscribers.retain(|tx| !tx.is_closed());
        }
    }

    /// Get the number of subscribers for an event type
    pub async fn subscriber_count(&self, event_type: EventType) -> usize {
        let subscribers = self.subscribers.read().await;
        subscribers.get(&event_type).map(|s| s.len()).unwrap_or(0)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::block_store::Block;

    #[tokio::test]
    async fn test_event_bus() {
        // Create an event bus
        let event_bus = EventBus::new();

        // Subscribe to block events
        let mut block_rx = event_bus.subscribe(EventType::Block).await;

        // Subscribe to all events
        let mut all_rx = event_bus.subscribe(EventType::All).await;

        // Check subscriber count
        assert_eq!(event_bus.subscriber_count(EventType::Block).await, 1);
        assert_eq!(event_bus.subscriber_count(EventType::All).await, 1);

        // Create a block event
        let block = Block {
            height: 1,
            hash: [1u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 10,
            transactions: vec![],
            state_root: [0u8; 32],
        };

        let event = NetworkEvent::BlockReceived(block, "peer1".to_string());

        // Publish the event
        event_bus.publish(event.clone()).await;

        // Check that both subscribers received the event
        let received_block = block_rx.recv().await.unwrap();
        let received_all = all_rx.recv().await.unwrap();

        match received_block {
            NetworkEvent::BlockReceived(block, peer_id) => {
                assert_eq!(block.height, 1);
                assert_eq!(peer_id, "peer1");
            },
            _ => panic!("Expected BlockReceived event"),
        }

        match received_all {
            NetworkEvent::BlockReceived(block, peer_id) => {
                assert_eq!(block.height, 1);
                assert_eq!(peer_id, "peer1");
            },
            _ => panic!("Expected BlockReceived event"),
        }
    }
}
