use std::net::SocketAddr;
use std::time::{Duration, Instant};
use log::{debug, warn};
use dashmap::DashMap;

use crate::network::types::node_info::NodeInfo;
use crate::network::peer::state::ConnectionState;

/// Metadata about a peer
#[derive(Debug, Clone)]
pub struct PeerMetadata {
    /// Peer's node ID
    pub node_id: String,
    
    /// Peer's address
    pub addr: SocketAddr,
    
    /// Peer's node information
    pub node_info: Option<NodeInfo>,
    
    /// When the peer was last seen
    pub last_seen: Instant,
    
    /// Current connection state
    pub state: ConnectionState,
    
    /// Whether this is an outbound connection
    pub is_outbound: bool,
    
    /// Number of failed connection attempts
    pub failed_attempts: u32,
    
    /// Protocol version
    pub version: Option<String>,
    
    /// User agent
    pub user_agent: Option<String>,
    
    /// Ping latency in milliseconds
    pub ping_latency: Option<u64>,
}

impl PeerMetadata {
    /// Create new peer metadata
    pub fn new(node_id: String, addr: SocketAddr, is_outbound: bool) -> Self {
        Self {
            node_id,
            addr,
            node_info: None,
            last_seen: Instant::now(),
            state: if is_outbound { ConnectionState::Disconnected } else { ConnectionState::Connected },
            is_outbound,
            failed_attempts: 0,
            version: None,
            user_agent: None,
            ping_latency: None,
        }
    }
    
    /// Update the last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen = Instant::now();
    }
    
    /// Set the node information
    pub fn set_node_info(&mut self, node_info: NodeInfo) {
        self.version = Some(node_info.version.clone());
        self.node_info = Some(node_info);
    }
    
    /// Update the connection state
    pub fn update_state(&mut self, state: ConnectionState) {
        self.state = state;
        self.update_last_seen();
        
        if state == ConnectionState::Failed {
            self.failed_attempts += 1;
        } else if state == ConnectionState::Ready {
            self.failed_attempts = 0;
        }
    }
    
    /// Check if the peer is active
    pub fn is_active(&self) -> bool {
        self.state == ConnectionState::Ready
    }
    
    /// Check if the peer is banned
    pub fn is_banned(&self) -> bool {
        self.state == ConnectionState::Banned
    }
    
    /// Check if the peer is disconnected
    pub fn is_disconnected(&self) -> bool {
        self.state == ConnectionState::Disconnected || self.state == ConnectionState::Failed
    }
    
    /// Check if the peer has been inactive for too long
    pub fn is_inactive(&self, timeout: Duration) -> bool {
        self.last_seen.elapsed() > timeout
    }
    
    /// Update the ping latency
    pub fn update_ping_latency(&mut self, latency: u64) {
        self.ping_latency = Some(latency);
    }
}

/// Registry for tracking peers
pub struct PeerRegistry {
    /// Map of peer IDs to peer metadata
    peers: DashMap<String, PeerMetadata>,
    
    /// Map of addresses to peer IDs
    addr_to_id: DashMap<SocketAddr, String>,
    
    /// Inactivity timeout
    inactivity_timeout: Duration,
    
    /// Maximum number of failed attempts before banning
    max_failed_attempts: u32,
}

impl PeerRegistry {
    /// Create a new peer registry
    pub fn new() -> Self {
        Self {
            peers: DashMap::new(),
            addr_to_id: DashMap::new(),
            inactivity_timeout: Duration::from_secs(300), // 5 minutes
            max_failed_attempts: 5,
        }
    }
    
    /// Register a peer with the registry
    pub fn register_peer(&self, node_id: &str, addr: SocketAddr, is_outbound: bool) -> bool {
        // Check if we already have this peer ID
        if self.peers.contains_key(node_id) {
            return false;
        }
        
        // Check if we already have this address
        if self.addr_to_id.contains_key(&addr) {
            return false;
        }
        
        // Create peer metadata
        let metadata = PeerMetadata::new(node_id.to_string(), addr, is_outbound);
        
        // Add to maps
        self.peers.insert(node_id.to_string(), metadata);
        self.addr_to_id.insert(addr, node_id.to_string());
        
        debug!("Registered peer {} at {}", node_id, addr);
        true
    }
    
    /// Unregister a peer from the registry
    pub fn unregister_peer(&self, node_id: &str) -> bool {
        // Get the peer's address
        let addr = match self.get_peer_addr(node_id) {
            Some(addr) => addr,
            None => return false,
        };
        
        // Remove from maps
        self.peers.remove(node_id);
        self.addr_to_id.remove(&addr);
        
        debug!("Unregistered peer {} at {}", node_id, addr);
        true
    }
    
    /// Update a peer's state
    pub fn update_peer_state(&self, node_id: &str, state: ConnectionState) -> bool {
        if let Some(mut entry) = self.peers.get_mut(node_id) {
            entry.update_state(state);
            
            // If the peer has too many failed attempts, ban it
            if state == ConnectionState::Failed && entry.failed_attempts >= self.max_failed_attempts {
                entry.update_state(ConnectionState::Banned);
                warn!("Banned peer {} after {} failed attempts", node_id, entry.failed_attempts);
            }
            
            true
        } else {
            false
        }
    }
    
    /// Update a peer's node information
    pub fn update_peer_info(&self, node_id: &str, node_info: NodeInfo) -> bool {
        if let Some(mut entry) = self.peers.get_mut(node_id) {
            entry.set_node_info(node_info);
            true
        } else {
            false
        }
    }
    
    /// Update a peer's last seen timestamp
    pub fn update_peer_last_seen(&self, node_id: &str) -> bool {
        if let Some(mut entry) = self.peers.get_mut(node_id) {
            entry.update_last_seen();
            true
        } else {
            false
        }
    }
    
    /// Update a peer's ping latency
    pub fn update_peer_ping_latency(&self, node_id: &str, latency: u64) -> bool {
        if let Some(mut entry) = self.peers.get_mut(node_id) {
            entry.update_ping_latency(latency);
            true
        } else {
            false
        }
    }
    
    /// Get a peer's metadata
    pub fn get_peer_metadata(&self, node_id: &str) -> Option<PeerMetadata> {
        self.peers.get(node_id).map(|entry| entry.clone())
    }
    
    /// Get a peer's address
    pub fn get_peer_addr(&self, node_id: &str) -> Option<SocketAddr> {
        self.peers.get(node_id).map(|entry| entry.addr)
    }
    
    /// Get a peer's node ID by address
    pub fn get_peer_id(&self, addr: &SocketAddr) -> Option<String> {
        self.addr_to_id.get(addr).map(|entry| entry.clone())
    }
    
    /// Check if a peer is active
    pub fn is_peer_active(&self, node_id: &str) -> bool {
        self.peers.get(node_id)
            .map(|entry| entry.is_active())
            .unwrap_or(false)
    }
    
    /// Check if a peer is banned
    pub fn is_peer_banned(&self, node_id: &str) -> bool {
        self.peers.get(node_id)
            .map(|entry| entry.is_banned())
            .unwrap_or(false)
    }
    
    /// Get all active peers
    pub fn get_active_peers(&self) -> Vec<PeerMetadata> {
        self.peers.iter()
            .filter(|entry| entry.is_active())
            .map(|entry| entry.clone())
            .collect()
    }
    
    /// Get all peers
    pub fn get_all_peers(&self) -> Vec<PeerMetadata> {
        self.peers.iter()
            .map(|entry| entry.clone())
            .collect()
    }
    
    /// Get the number of active peers
    pub fn active_peer_count(&self) -> usize {
        self.peers.iter()
            .filter(|entry| entry.is_active())
            .count()
    }
    
    /// Get the total number of peers
    pub fn total_peer_count(&self) -> usize {
        self.peers.len()
    }
    
    /// Clean up inactive peers
    pub fn cleanup_inactive_peers(&self) -> usize {
        let inactive_peers: Vec<String> = self.peers.iter()
            .filter(|entry| entry.is_inactive(self.inactivity_timeout))
            .map(|entry| entry.node_id.clone())
            .collect();
        
        for node_id in &inactive_peers {
            self.unregister_peer(node_id);
        }
        
        inactive_peers.len()
    }
    
    /// Ban a peer
    pub fn ban_peer(&self, node_id: &str) -> bool {
        self.update_peer_state(node_id, ConnectionState::Banned)
    }
    
    /// Unban a peer
    pub fn unban_peer(&self, node_id: &str) -> bool {
        if let Some(mut entry) = self.peers.get_mut(node_id) {
            if entry.is_banned() {
                entry.update_state(ConnectionState::Disconnected);
                entry.failed_attempts = 0;
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::time::Duration;
    
    #[test]
    fn test_peer_registry() {
        // Create a registry
        let registry = PeerRegistry::new();
        
        // Register a peer
        let addr: SocketAddr = "127.0.0.1:8000".parse().unwrap();
        assert!(registry.register_peer("peer1", addr, true));
        
        // Check that the peer is registered
        assert_eq!(registry.total_peer_count(), 1);
        
        // Get the peer's metadata
        let metadata = registry.get_peer_metadata("peer1").unwrap();
        assert_eq!(metadata.node_id, "peer1");
        assert_eq!(metadata.addr, addr);
        assert_eq!(metadata.state, ConnectionState::Disconnected);
        assert!(metadata.is_outbound);
        
        // Update the peer's state
        assert!(registry.update_peer_state("peer1", ConnectionState::Ready));
        
        // Check that the peer is active
        assert!(registry.is_peer_active("peer1"));
        assert_eq!(registry.active_peer_count(), 1);
        
        // Unregister the peer
        assert!(registry.unregister_peer("peer1"));
        
        // Check that the peer is unregistered
        assert_eq!(registry.total_peer_count(), 0);
    }
    
    #[test]
    fn test_peer_banning() {
        // Create a registry
        let registry = PeerRegistry::new();
        
        // Register a peer
        let addr: SocketAddr = "127.0.0.1:8000".parse().unwrap();
        registry.register_peer("peer1", addr, true);
        
        // Ban the peer
        assert!(registry.ban_peer("peer1"));
        
        // Check that the peer is banned
        assert!(registry.is_peer_banned("peer1"));
        
        // Unban the peer
        assert!(registry.unban_peer("peer1"));
        
        // Check that the peer is no longer banned
        assert!(!registry.is_peer_banned("peer1"));
    }
    
    #[test]
    fn test_peer_failure_tracking() {
        // Create a registry with a low max_failed_attempts
        let registry = PeerRegistry {
            max_failed_attempts: 2,
            ..PeerRegistry::new()
        };
        
        // Register a peer
        let addr: SocketAddr = "127.0.0.1:8000".parse().unwrap();
        registry.register_peer("peer1", addr, true);
        
        // Fail the peer once
        registry.update_peer_state("peer1", ConnectionState::Failed);
        
        // Check that the peer is not banned yet
        assert!(!registry.is_peer_banned("peer1"));
        
        // Fail the peer again
        registry.update_peer_state("peer1", ConnectionState::Failed);
        
        // Check that the peer is now banned
        assert!(registry.is_peer_banned("peer1"));
    }
}
