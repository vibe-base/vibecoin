//! Advanced message router for VibeCoin blockchain
//!
//! This module provides an advanced message routing system for the blockchain,
//! with support for prioritization, load balancing, and specialized routing.

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, mpsc, Mutex};
use log::{debug, error, info, warn};

use crate::network::types::message::NetMessage;
use crate::network::peer::registry::PeerRegistry;
use crate::network::peer::broadcaster::PeerBroadcaster;
use crate::network::handlers::message_handler::HandlerRegistry;

/// Message type categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    /// Handshake messages
    Handshake,

    /// Block-related messages
    Block,

    /// Transaction-related messages
    Transaction,

    /// Sync-related messages
    Sync,

    /// Ping/pong messages
    Ping,

    /// Disconnect messages
    Disconnect,

    /// Other messages
    Other,
}

impl From<&NetMessage> for MessageType {
    fn from(message: &NetMessage) -> Self {
        match message {
            NetMessage::Handshake(_) => MessageType::Handshake,
            NetMessage::NewBlock(_) | NetMessage::RequestBlock(_) | NetMessage::ResponseBlock(_) => MessageType::Block,
            NetMessage::NewTransaction(_) => MessageType::Transaction,
            NetMessage::RequestBlockRange { .. } | NetMessage::ResponseBlockRange(_) => MessageType::Sync,
            NetMessage::Ping(_) | NetMessage::Pong(_) => MessageType::Ping,
            NetMessage::Disconnect(_) => MessageType::Disconnect,
        }
    }
}

/// Sync request type
#[derive(Debug, Clone)]
pub enum SyncRequest {
    /// Get the latest block
    GetLatestBlock,

    /// Get blocks in a range
    GetBlocks(u64, u64),

    /// Get a specific block
    GetBlock(u64),

    /// Get the chain state
    GetChainState,

    /// Legacy struct fields
    Legacy {
        /// Peer ID
        peer_id: String,

        /// Start height
        start_height: u64,

        /// End height
        end_height: u64,

        /// Request ID
        request_id: u64,

        /// Timestamp
        timestamp: u64,
    },
}

/// Advanced message router
pub struct AdvancedMessageRouter {
    /// Handler registry
    handlers: Arc<RwLock<HandlerRegistry>>,

    /// Peer registry
    peer_registry: Arc<PeerRegistry>,

    /// Peer broadcaster
    broadcaster: Arc<PeerBroadcaster>,

    /// Message queue
    message_queue: Arc<Mutex<Vec<(String, NetMessage)>>>,

    /// Sync requests
    sync_requests: Arc<RwLock<HashMap<u64, SyncRequest>>>,

    /// Next request ID
    next_request_id: Arc<Mutex<u64>>,
}

impl AdvancedMessageRouter {
    /// Create a new advanced message router
    pub fn new(
        handlers: Arc<RwLock<HandlerRegistry>>,
        peer_registry: Arc<PeerRegistry>,
        broadcaster: Arc<PeerBroadcaster>,
    ) -> Self {
        Self {
            handlers,
            peer_registry,
            broadcaster,
            message_queue: Arc::new(Mutex::new(Vec::new())),
            sync_requests: Arc::new(RwLock::new(HashMap::new())),
            next_request_id: Arc::new(Mutex::new(0)),
        }
    }

    /// Start the router
    pub async fn start(&self) {
        info!("Starting advanced message router");

        // Start the message processor
        let message_queue = Arc::clone(&self.message_queue);
        let handlers = Arc::clone(&self.handlers);
        let peer_registry = Arc::clone(&self.peer_registry);

        tokio::spawn(async move {
            loop {
                // Process messages in the queue
                let messages = {
                    let mut queue = message_queue.lock().await;
                    let messages = queue.clone();
                    queue.clear();
                    messages
                };

                for (peer_id, message) in messages {
                    let message_type = MessageType::from(&message);

                    // Get the appropriate handler
                    let handler: Arc<dyn crate::network::handlers::message_handler::MessageHandler> = {
                        let handlers = handlers.read().await;
                        match handlers.get(&message_type) {
                            Some(handler) => Arc::clone(&handler),
                            None => {
                                warn!("No handler for message type: {:?}", message_type);
                                continue;
                            }
                        }
                    };

                    // Handle the message
                    match handler.handle(peer_id.clone(), message).await {
                        Ok(_) => {
                            debug!("Message handled successfully: {:?}", message_type);
                        }
                        Err(e) => {
                            error!("Failed to handle message: {}", e);
                        }
                    }
                }

                // Sleep for a short time
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        });
    }

    /// Route a message
    pub async fn route(&self, peer_id: String, message: NetMessage) {
        // Add the message to the queue
        let mut queue = self.message_queue.lock().await;
        queue.push((peer_id, message));
    }

    /// Create a sync request
    pub async fn create_sync_request(&self, peer_id: String, start_height: u64, end_height: u64) -> u64 {
        let request_id = {
            let mut next_id = self.next_request_id.lock().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        let request = SyncRequest::Legacy {
            peer_id,
            start_height,
            end_height,
            request_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let mut requests = self.sync_requests.write().await;
        requests.insert(request_id, request.clone());

        info!("Created sync request: id={}, start={}, end={}", request_id, start_height, end_height);

        request_id
    }

    /// Get a sync request
    pub async fn get_sync_request(&self, request_id: u64) -> Option<SyncRequest> {
        let requests = self.sync_requests.read().await;
        requests.get(&request_id).cloned()
    }

    /// Complete a sync request
    pub async fn complete_sync_request(&self, request_id: u64) {
        let mut requests = self.sync_requests.write().await;
        if requests.remove(&request_id).is_some() {
            info!("Completed sync request: id={}", request_id);
        } else {
            warn!("Sync request not found: id={}", request_id);
        }
    }
}
