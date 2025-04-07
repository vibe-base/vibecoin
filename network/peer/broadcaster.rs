use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use log::{debug, warn};

use crate::network::peer::registry::PeerRegistry;

use crate::network::types::message::NetMessage;

/// Broadcaster for sending messages to peers
pub struct PeerBroadcaster {
    /// Map of peer IDs to message senders
    peer_senders: Arc<RwLock<HashMap<String, mpsc::Sender<NetMessage>>>>,

    /// Maximum number of send retries
    max_retries: usize,
}

impl PeerBroadcaster {
    /// Create a new peer broadcaster
    pub fn new() -> Self {
        Self::with_registry(None)
    }

    /// Create a new peer broadcaster with a registry
    pub fn with_registry(_registry: Option<Arc<PeerRegistry>>) -> Self {
        Self {
            peer_senders: Arc::new(RwLock::new(HashMap::new())),
            max_retries: 3,
        }
    }

    /// Register a peer with the broadcaster
    pub async fn register_peer(&self, peer_id: &str, sender: mpsc::Sender<NetMessage>) {
        let mut senders = self.peer_senders.write().await;
        senders.insert(peer_id.to_string(), sender);
        debug!("Registered peer {} with broadcaster", peer_id);
    }

    /// Unregister a peer from the broadcaster
    pub async fn unregister_peer(&self, peer_id: &str) {
        let mut senders = self.peer_senders.write().await;
        if senders.remove(peer_id).is_some() {
            debug!("Unregistered peer {} from broadcaster", peer_id);
        }
    }

    /// Broadcast a message to all peers
    pub async fn broadcast(&self, message: NetMessage) {
        let senders = self.peer_senders.read().await;
        let peer_count = senders.len();

        if peer_count == 0 {
            debug!("No peers to broadcast to");
            return;
        }

        let message_type = match &message {
            NetMessage::NewBlock(_) => "block",
            NetMessage::NewTransaction(_) => "transaction",
            _ => "message",
        };

        debug!("Broadcasting {} to {} peers", message_type, peer_count);

        let mut failed_peers = Vec::new();

        for (peer_id, sender) in senders.iter() {
            let mut success = false;

            // Try to send the message with retries
            for _ in 0..self.max_retries {
                match sender.try_send(message.clone()) {
                    Ok(_) => {
                        success = true;
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                        // Channel is full, wait a bit and retry
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                    Err(_) => {
                        // Channel is closed, no need to retry
                        break;
                    }
                }
            }

            if !success {
                failed_peers.push(peer_id.clone());
                warn!("Failed to broadcast to peer {}", peer_id);
            }
        }

        // Report success
        let success_count = peer_count - failed_peers.len();
        if success_count > 0 {
            debug!("Successfully broadcast to {}/{} peers", success_count, peer_count);
        }

        // Clean up failed peers in a separate task to avoid holding the read lock
        if !failed_peers.is_empty() {
            let broadcaster = self.clone();
            tokio::spawn(async move {
                for peer_id in failed_peers {
                    broadcaster.unregister_peer(&peer_id).await;
                }
            });
        }
    }

    /// Send a message to a specific peer
    pub async fn send_to_peer(&self, peer_id: &str, message: NetMessage) -> Result<bool, String> {
        let senders = self.peer_senders.read().await;

        if let Some(sender) = senders.get(peer_id) {
            let mut success = false;

            // Try to send the message with retries
            for _ in 0..self.max_retries {
                match sender.try_send(message.clone()) {
                    Ok(_) => {
                        success = true;
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                        // Channel is full, wait a bit and retry
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                    Err(_) => {
                        // Channel is closed, no need to retry
                        break;
                    }
                }
            }

            if !success {
                warn!("Failed to send message to peer {}", peer_id);

                // Clean up the failed peer in a separate task
                let broadcaster = self.clone();
                let peer_id = peer_id.to_string();
                tokio::spawn(async move {
                    broadcaster.unregister_peer(&peer_id).await;
                });

                return Ok(false);
            }

            Ok(true)
        } else {
            warn!("Peer {} not found", peer_id);
            Err(format!("Peer {} not found", peer_id))
        }
    }

    /// Get the number of connected peers
    pub async fn peer_count(&self) -> usize {
        let senders = self.peer_senders.read().await;
        senders.len()
    }

    /// Get a list of connected peer IDs
    pub async fn peer_ids(&self) -> Vec<String> {
        let senders = self.peer_senders.read().await;
        senders.keys().cloned().collect()
    }

    /// Broadcast a message to all peers except one
    pub async fn broadcast_except(&self, message: NetMessage, except_peer_id: &str) -> Result<(), String> {
        let senders = self.peer_senders.read().await;
        let peer_count = senders.len();

        if peer_count <= 1 {
            debug!("No other peers to broadcast to");
            return Ok(());
        }

        let message_type = match &message {
            NetMessage::NewBlock(_) => "block",
            NetMessage::NewTransaction(_) => "transaction",
            _ => "message",
        };

        debug!("Broadcasting {} to all peers except {}", message_type, except_peer_id);

        let mut failed_peers = Vec::new();

        for (peer_id, sender) in senders.iter() {
            // Skip the excluded peer
            if peer_id == except_peer_id {
                continue;
            }

            let mut success = false;

            // Try to send the message with retries
            for _ in 0..self.max_retries {
                match sender.try_send(message.clone()) {
                    Ok(_) => {
                        success = true;
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                        // Channel is full, wait a bit and retry
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                    Err(_) => {
                        // Channel is closed, no need to retry
                        break;
                    }
                }
            }

            if !success {
                failed_peers.push(peer_id.clone());
                warn!("Failed to broadcast to peer {}", peer_id);
            }
        }

        // Report success
        let success_count = peer_count - failed_peers.len() - 1; // Subtract 1 for the excluded peer
        if success_count > 0 {
            debug!("Successfully broadcast to {}/{} peers", success_count, peer_count - 1);
        }

        // Clean up failed peers in a separate task to avoid holding the read lock
        if !failed_peers.is_empty() {
            let broadcaster = self.clone();
            tokio::spawn(async move {
                for peer_id in failed_peers {
                    broadcaster.unregister_peer(&peer_id).await;
                }
            });
        }

        Ok(())
    }
}

impl Clone for PeerBroadcaster {
    fn clone(&self) -> Self {
        Self {
            peer_senders: self.peer_senders.clone(),
            max_retries: self.max_retries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::types::message::NetMessage;
    use crate::network::types::node_info::NodeInfo;

    #[tokio::test]
    async fn test_broadcaster_registration() {
        // Create a broadcaster
        let broadcaster = PeerBroadcaster::new();

        // Create a channel for a peer
        let (tx, mut rx) = mpsc::channel(10);

        // Register the peer
        broadcaster.register_peer("peer1", tx).await;

        // Check peer count
        assert_eq!(broadcaster.peer_count().await, 1);

        // Check peer IDs
        let peer_ids = broadcaster.peer_ids().await;
        assert_eq!(peer_ids.len(), 1);
        assert_eq!(peer_ids[0], "peer1");

        // Unregister the peer
        broadcaster.unregister_peer("peer1").await;

        // Check peer count again
        assert_eq!(broadcaster.peer_count().await, 0);
    }

    #[tokio::test]
    async fn test_send_to_peer() {
        // Create a broadcaster
        let broadcaster = PeerBroadcaster::new();

        // Create a channel for a peer
        let (tx, mut rx) = mpsc::channel(10);

        // Register the peer
        broadcaster.register_peer("peer1", tx).await;

        // Create a test message
        let node_info = NodeInfo::new(
            "1.0.0".to_string(),
            "test-node".to_string(),
            "127.0.0.1:8000".parse().unwrap(),
        );

        let message = NetMessage::Handshake(node_info);

        // Send the message to the peer
        let result = broadcaster.send_to_peer("peer1", message.clone()).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Check that the message was received
        let received = rx.recv().await.unwrap();

        match received {
            NetMessage::Handshake(info) => {
                assert_eq!(info.node_id, "test-node");
            }
            _ => panic!("Expected Handshake message"),
        }
    }

    #[tokio::test]
    async fn test_broadcast() {
        // Create a broadcaster
        let broadcaster = PeerBroadcaster::new();

        // Create channels for peers
        let (tx1, mut rx1) = mpsc::channel(10);
        let (tx2, mut rx2) = mpsc::channel(10);

        // Register the peers
        broadcaster.register_peer("peer1", tx1).await;
        broadcaster.register_peer("peer2", tx2).await;

        // Create a test message
        let node_info = NodeInfo::new(
            "1.0.0".to_string(),
            "test-node".to_string(),
            "127.0.0.1:8000".parse().unwrap(),
        );

        let message = NetMessage::Handshake(node_info);

        // Broadcast the message
        broadcaster.broadcast(message.clone()).await;

        // Check that both peers received the message
        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();

        match received1 {
            NetMessage::Handshake(info) => {
                assert_eq!(info.node_id, "test-node");
            }
            _ => panic!("Expected Handshake message"),
        }

        match received2 {
            NetMessage::Handshake(info) => {
                assert_eq!(info.node_id, "test-node");
            }
            _ => panic!("Expected Handshake message"),
        }
    }
}
