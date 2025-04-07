use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use log::{debug, error, info, warn};

use crate::network::types::message::NetMessage;

/// Type for message handlers
type MessageHandler = Arc<Box<dyn Fn(String, NetMessage) -> bool + Send + Sync>>;

/// Router for network messages
pub struct MessageRouter {
    /// Handlers for different message types
    handlers: RwLock<HashMap<String, Vec<MessageHandler>>>,
}

impl MessageRouter {
    /// Create a new message router
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a handler for a specific message type
    pub async fn register_handler<F>(&self, message_type: &str, handler: F)
    where
        F: Fn(String, NetMessage) -> bool + Send + Sync + 'static,
    {
        let mut handlers = self.handlers.write().await;

        let type_handlers = handlers
            .entry(message_type.to_string())
            .or_insert_with(Vec::new);

        type_handlers.push(Arc::new(Box::new(handler)));
    }

    /// Route a message to the appropriate handlers
    pub async fn route_message(&self, node_id: String, message: NetMessage) {
        let message_type = match &message {
            NetMessage::Handshake(_) => "handshake",
            NetMessage::NewBlock(_) => "new_block",
            NetMessage::NewTransaction(_) => "new_transaction",
            NetMessage::RequestBlock(_) => "request_block",
            NetMessage::ResponseBlock(_) => "response_block",
            NetMessage::RequestBlockRange { .. } => "request_block_range",
            NetMessage::ResponseBlockRange(_) => "response_block_range",
            NetMessage::Ping(_) => "ping",
            NetMessage::Pong(_) => "pong",
            NetMessage::Disconnect(_) => "disconnect",
        };

        debug!("Routing {} message from {}", message_type, node_id);

        // Get handlers for this message type
        let handlers = {
            let handlers_map = self.handlers.read().await;

            // Check if we have handlers for this message type
            if !handlers_map.contains_key(message_type) {
                debug!("No handlers for {} messages", message_type);
                return;
            }

            // Clone the handlers before dropping the lock
            handlers_map.get(message_type).unwrap().clone()
        }; // handlers_map is dropped here automatically

        // Call each handler
        let mut handled = false;
        for handler in handlers {
            if handler(node_id.clone(), message.clone()) {
                handled = true;
            }
        }

        if !handled {
            debug!("Message not handled: {} from {}", message_type, node_id);
        }
    }

    /// Create a channel for a specific message type
    pub async fn create_channel(&self, message_type: &str) -> mpsc::Receiver<(String, NetMessage)> {
        let (tx, rx) = mpsc::channel(100);

        self.register_handler(message_type, move |node_id, message| {
            match tx.try_send((node_id, message)) {
                Ok(_) => true,
                Err(e) => {
                    error!("Failed to send message to channel: {:?}", e);
                    false
                }
            }
        }).await;

        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::types::node_info::NodeInfo;

    #[tokio::test]
    async fn test_message_router() {
        // Create a router
        let router = MessageRouter::new();

        // Create a channel to receive messages
        let (tx, mut rx) = mpsc::channel(10);

        // Register a handler
        router.register_handler("handshake", move |node_id, message| {
            if let NetMessage::Handshake(info) = message {
                let _ = tx.try_send((node_id, info));
                true
            } else {
                false
            }
        }).await;

        // Create a test message
        let node_info = NodeInfo::new(
            "1.0.0".to_string(),
            "test-node".to_string(),
            "127.0.0.1:8000".parse().unwrap(),
        );

        let message = NetMessage::Handshake(node_info.clone());

        // Route the message
        router.route_message("sender-node".to_string(), message).await;

        // Check that the handler was called
        let (received_node_id, received_info) = rx.try_recv().unwrap();
        assert_eq!(received_node_id, "sender-node");
        assert_eq!(received_info, node_info);
    }

    #[tokio::test]
    async fn test_create_channel() {
        // Create a router
        let router = MessageRouter::new();

        // Create a channel for handshake messages
        let mut rx = router.create_channel("handshake").await;

        // Create a test message
        let node_info = NodeInfo::new(
            "1.0.0".to_string(),
            "test-node".to_string(),
            "127.0.0.1:8000".parse().unwrap(),
        );

        let message = NetMessage::Handshake(node_info);

        // Route the message
        router.route_message("sender-node".to_string(), message.clone()).await;

        // Check that we received the message
        let received = rx.try_recv().unwrap();
        assert_eq!(received.0, "sender-node");

        if let NetMessage::Handshake(info) = received.1 {
            assert_eq!(info.node_id, "test-node");
        } else {
            panic!("Expected Handshake message");
        }
    }
}
