use std::net::SocketAddr;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

use crate::network::types::node_info::NodeInfo;

/// Connection state of a peer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    
    /// Attempting to connect
    Connecting,
    
    /// Connected but handshake not complete
    Connected,
    
    /// Handshake complete, ready for messages
    Ready,
    
    /// Connection failed, will retry
    Failed,
    
    /// Banned peer
    Banned,
}

/// Information about a peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer's address
    pub addr: SocketAddr,
    
    /// Peer's node information (if known)
    pub node_info: Option<NodeInfo>,
    
    /// Current connection state
    pub state: ConnectionState,
    
    /// When the peer was last seen
    pub last_seen: Instant,
    
    /// When to retry connection if failed
    pub retry_at: Option<Instant>,
    
    /// Number of failed connection attempts
    pub failed_attempts: u32,
    
    /// Whether this is an outbound connection
    pub is_outbound: bool,
}

impl PeerInfo {
    /// Create a new PeerInfo for an outbound connection
    pub fn new_outbound(addr: SocketAddr) -> Self {
        Self {
            addr,
            node_info: None,
            state: ConnectionState::Disconnected,
            last_seen: Instant::now(),
            retry_at: None,
            failed_attempts: 0,
            is_outbound: true,
        }
    }
    
    /// Create a new PeerInfo for an inbound connection
    pub fn new_inbound(addr: SocketAddr) -> Self {
        Self {
            addr,
            node_info: None,
            state: ConnectionState::Connected,
            last_seen: Instant::now(),
            retry_at: None,
            failed_attempts: 0,
            is_outbound: false,
        }
    }
    
    /// Update the connection state
    pub fn update_state(&mut self, state: ConnectionState) {
        self.state = state;
        self.last_seen = Instant::now();
        
        if state == ConnectionState::Failed {
            self.failed_attempts += 1;
            
            // Exponential backoff for retries
            let retry_delay = Duration::from_secs(2u64.pow(self.failed_attempts.min(6)));
            self.retry_at = Some(Instant::now() + retry_delay);
        } else if state == ConnectionState::Ready {
            // Reset failed attempts when connection is successful
            self.failed_attempts = 0;
            self.retry_at = None;
        }
    }
    
    /// Check if it's time to retry the connection
    pub fn should_retry(&self) -> bool {
        if self.state != ConnectionState::Failed {
            return false;
        }
        
        if let Some(retry_at) = self.retry_at {
            Instant::now() >= retry_at
        } else {
            false
        }
    }
    
    /// Update the node info
    pub fn set_node_info(&mut self, info: NodeInfo) {
        self.node_info = Some(info);
    }
    
    /// Get the node ID if available
    pub fn node_id(&self) -> Option<&str> {
        self.node_info.as_ref().map(|info| info.node_id.as_str())
    }
    
    /// Check if the peer is active (connected and ready)
    pub fn is_active(&self) -> bool {
        self.state == ConnectionState::Ready
    }
    
    /// Check if the peer is banned
    pub fn is_banned(&self) -> bool {
        self.state == ConnectionState::Banned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    
    #[test]
    fn test_peer_state_transitions() {
        let addr = "127.0.0.1:8000".parse().unwrap();
        let mut peer = PeerInfo::new_outbound(addr);
        
        // Initial state
        assert_eq!(peer.state, ConnectionState::Disconnected);
        assert_eq!(peer.failed_attempts, 0);
        assert!(peer.retry_at.is_none());
        
        // Update to connecting
        peer.update_state(ConnectionState::Connecting);
        assert_eq!(peer.state, ConnectionState::Connecting);
        
        // Update to connected
        peer.update_state(ConnectionState::Connected);
        assert_eq!(peer.state, ConnectionState::Connected);
        
        // Update to ready
        peer.update_state(ConnectionState::Ready);
        assert_eq!(peer.state, ConnectionState::Ready);
        assert_eq!(peer.failed_attempts, 0);
        
        // Update to failed
        peer.update_state(ConnectionState::Failed);
        assert_eq!(peer.state, ConnectionState::Failed);
        assert_eq!(peer.failed_attempts, 1);
        assert!(peer.retry_at.is_some());
        
        // Should not retry immediately
        assert!(!peer.should_retry());
        
        // Wait for retry time
        sleep(Duration::from_secs(3));
        assert!(peer.should_retry());
    }
    
    #[test]
    fn test_peer_node_info() {
        let addr = "127.0.0.1:8000".parse().unwrap();
        let mut peer = PeerInfo::new_outbound(addr);
        
        // Initially no node info
        assert!(peer.node_info.is_none());
        assert!(peer.node_id().is_none());
        
        // Set node info
        let node_info = NodeInfo::new(
            "1.0.0".to_string(),
            "test-node".to_string(),
            addr,
        );
        peer.set_node_info(node_info);
        
        // Now we have node info
        assert!(peer.node_info.is_some());
        assert_eq!(peer.node_id(), Some("test-node"));
    }
}
