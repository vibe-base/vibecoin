use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{Duration, interval};
use log::{debug, error, info, warn};

use crate::network::peer::state::{ConnectionState, PeerInfo};
use crate::network::peer::handler::PeerHandler;
use crate::network::types::node_info::NodeInfo;
use crate::network::types::message::NetMessage;
use crate::network::service::router::MessageRouter;

/// Manager for peer connections
pub struct PeerManager {
    /// Map of peer addresses to peer info
    peers: RwLock<HashMap<SocketAddr, Arc<PeerInfo>>>,
    
    /// Map of node IDs to peer addresses
    node_id_to_addr: RwLock<HashMap<String, SocketAddr>>,
    
    /// Map of peer addresses to message senders
    peer_senders: RwLock<HashMap<SocketAddr, mpsc::Sender<NetMessage>>>,
    
    /// Local node information
    local_node_info: NodeInfo,
    
    /// Message router
    router: Arc<MessageRouter>,
    
    /// Channel for incoming messages
    incoming_tx: mpsc::Sender<(String, NetMessage)>,
    
    /// Maximum number of outbound connections
    max_outbound: usize,
    
    /// Maximum number of inbound connections
    max_inbound: usize,
}

impl PeerManager {
    /// Create a new peer manager
    pub fn new(
        local_node_info: NodeInfo,
        router: Arc<MessageRouter>,
        incoming_tx: mpsc::Sender<(String, NetMessage)>,
        max_outbound: usize,
        max_inbound: usize,
    ) -> Self {
        Self {
            peers: RwLock::new(HashMap::new()),
            node_id_to_addr: RwLock::new(HashMap::new()),
            peer_senders: RwLock::new(HashMap::new()),
            local_node_info,
            router,
            incoming_tx,
            max_outbound,
            max_inbound,
        }
    }
    
    /// Add a peer to the manager
    pub async fn add_peer(&self, addr: SocketAddr, is_outbound: bool) -> bool {
        let mut peers = self.peers.write().await;
        
        // Check if we already have this peer
        if peers.contains_key(&addr) {
            return false;
        }
        
        // Check connection limits
        let outbound_count = peers.values()
            .filter(|p| p.is_outbound && p.is_active())
            .count();
        
        let inbound_count = peers.values()
            .filter(|p| !p.is_outbound && p.is_active())
            .count();
        
        if is_outbound && outbound_count >= self.max_outbound {
            warn!("Maximum outbound connections reached ({})", self.max_outbound);
            return false;
        }
        
        if !is_outbound && inbound_count >= self.max_inbound {
            warn!("Maximum inbound connections reached ({})", self.max_inbound);
            return false;
        }
        
        // Create peer info
        let peer_info = if is_outbound {
            PeerInfo::new_outbound(addr)
        } else {
            PeerInfo::new_inbound(addr)
        };
        
        // Add to peers map
        let peer_info = Arc::new(peer_info);
        peers.insert(addr, peer_info);
        
        true
    }
    
    /// Handle a new inbound connection
    pub async fn handle_inbound_connection(&self, stream: TcpStream, addr: SocketAddr) {
        // Add the peer
        if !self.add_peer(addr, false).await {
            warn!("Rejected inbound connection from {}", addr);
            return;
        }
        
        // Get the peer info
        let peer_info = {
            let peers = self.peers.read().await;
            peers.get(&addr).cloned()
        };
        
        if let Some(peer_info) = peer_info {
            // Create and start the peer handler
            let handler = PeerHandler::new(
                stream,
                peer_info,
                self.local_node_info.clone(),
                self.router.clone(),
                self.incoming_tx.clone(),
            );
            
            // Get the message sender
            let sender = handler.get_sender();
            
            // Store the sender
            {
                let mut peer_senders = self.peer_senders.write().await;
                peer_senders.insert(addr, sender);
            }
            
            // Spawn the handler task
            tokio::spawn(async move {
                handler.handle().await;
            });
        }
    }
    
    /// Connect to a peer
    pub async fn connect_to_peer(&self, addr: SocketAddr) {
        // Add the peer
        if !self.add_peer(addr, true).await {
            return;
        }
        
        // Get the peer info
        let peer_info = {
            let mut peers = self.peers.write().await;
            let peer_info = peers.get_mut(&addr).unwrap();
            peer_info.update_state(ConnectionState::Connecting);
            peer_info.clone()
        };
        
        // Connect to the peer
        match TcpStream::connect(addr).await {
            Ok(stream) => {
                info!("Connected to peer {}", addr);
                
                // Create and start the peer handler
                let handler = PeerHandler::new(
                    stream,
                    peer_info,
                    self.local_node_info.clone(),
                    self.router.clone(),
                    self.incoming_tx.clone(),
                );
                
                // Get the message sender
                let sender = handler.get_sender();
                
                // Store the sender
                {
                    let mut peer_senders = self.peer_senders.write().await;
                    peer_senders.insert(addr, sender);
                }
                
                // Spawn the handler task
                tokio::spawn(async move {
                    handler.handle().await;
                });
            }
            Err(e) => {
                error!("Failed to connect to peer {}: {}", addr, e);
                
                // Update peer state
                let mut peers = self.peers.write().await;
                if let Some(peer_info) = peers.get_mut(&addr) {
                    peer_info.update_state(ConnectionState::Failed);
                }
            }
        }
    }
    
    /// Broadcast a message to all connected peers
    pub async fn broadcast(&self, message: NetMessage) {
        let peer_senders = self.peer_senders.read().await;
        
        for (addr, sender) in peer_senders.iter() {
            if let Err(e) = sender.send(message.clone()).await {
                error!("Failed to broadcast message to peer {}: {:?}", addr, e);
            }
        }
    }
    
    /// Send a message to a specific peer by node ID
    pub async fn send_to_peer(&self, node_id: &str, message: NetMessage) -> bool {
        // Look up the peer address
        let addr = {
            let node_id_to_addr = self.node_id_to_addr.read().await;
            match node_id_to_addr.get(node_id) {
                Some(addr) => *addr,
                None => {
                    warn!("Unknown peer node ID: {}", node_id);
                    return false;
                }
            }
        };
        
        // Get the sender
        let sender = {
            let peer_senders = self.peer_senders.read().await;
            match peer_senders.get(&addr) {
                Some(sender) => sender.clone(),
                None => {
                    warn!("No sender for peer {}", addr);
                    return false;
                }
            }
        };
        
        // Send the message
        match sender.send(message).await {
            Ok(_) => true,
            Err(e) => {
                error!("Failed to send message to peer {}: {:?}", addr, e);
                false
            }
        }
    }
    
    /// Get the number of connected peers
    pub async fn connected_peer_count(&self) -> usize {
        let peers = self.peers.read().await;
        peers.values()
            .filter(|p| p.is_active())
            .count()
    }
    
    /// Get a list of connected peer addresses
    pub async fn connected_peers(&self) -> Vec<SocketAddr> {
        let peers = self.peers.read().await;
        peers.iter()
            .filter(|(_, p)| p.is_active())
            .map(|(addr, _)| *addr)
            .collect()
    }
    
    /// Start the peer manager background tasks
    pub async fn start(&self) {
        // Spawn a task to periodically check for peers to reconnect
        let self_clone = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                self_clone.check_reconnects().await;
            }
        });
    }
    
    /// Check for peers that need to be reconnected
    async fn check_reconnects(&self) {
        let peers_to_reconnect = {
            let peers = self.peers.read().await;
            peers.iter()
                .filter(|(_, p)| p.is_outbound && p.should_retry())
                .map(|(addr, _)| *addr)
                .collect::<Vec<_>>()
        };
        
        for addr in peers_to_reconnect {
            debug!("Attempting to reconnect to peer {}", addr);
            self.connect_to_peer(addr).await;
        }
    }
}

impl Clone for PeerManager {
    fn clone(&self) -> Self {
        Self {
            peers: self.peers.clone(),
            node_id_to_addr: self.node_id_to_addr.clone(),
            peer_senders: self.peer_senders.clone(),
            local_node_info: self.local_node_info.clone(),
            router: self.router.clone(),
            incoming_tx: self.incoming_tx.clone(),
            max_outbound: self.max_outbound,
            max_inbound: self.max_inbound,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::service::router::MessageRouter;
    
    #[tokio::test]
    async fn test_peer_manager() {
        // Create a message router
        let router = Arc::new(MessageRouter::new());
        
        // Create channels for incoming messages
        let (incoming_tx, _incoming_rx) = mpsc::channel(100);
        
        // Create a local node info
        let local_node_info = NodeInfo::new(
            "1.0.0".to_string(),
            "local-node".to_string(),
            "127.0.0.1:8000".parse().unwrap(),
        );
        
        // Create a peer manager
        let peer_manager = PeerManager::new(
            local_node_info,
            router,
            incoming_tx,
            8,
            32,
        );
        
        // Add some peers
        let addr1 = "127.0.0.1:8001".parse().unwrap();
        let addr2 = "127.0.0.1:8002".parse().unwrap();
        
        assert!(peer_manager.add_peer(addr1, true).await);
        assert!(peer_manager.add_peer(addr2, true).await);
        
        // Check peer count
        assert_eq!(peer_manager.connected_peer_count().await, 0); // Not connected yet
        
        // Check connected peers
        let connected = peer_manager.connected_peers().await;
        assert_eq!(connected.len(), 0);
    }
}
