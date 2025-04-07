
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{debug, warn};
use dashmap::DashMap;

use crate::network::types::node_info::NodeInfo;
use crate::network::peer::state::ConnectionState;
use crate::network::peer::performance::PeerPerformance;
use crate::network::peer::reputation::{ReputationSystem, ReputationEvent};

/// Type alias for peer ID
pub type PeerId = String;

/// Enhanced peer metadata
#[derive(Debug, Clone)]
pub struct EnhancedPeerMetadata {
    /// Peer's node ID
    pub node_id: PeerId,

    /// Peer's address
    pub addr: SocketAddr,

    /// Peer's node information
    pub node_info: Option<NodeInfo>,

    /// Current connection state
    pub state: ConnectionState,

    /// Whether this is an outbound connection
    pub is_outbound: bool,

    /// When the peer was first seen
    pub first_seen: Instant,

    /// When the peer was last seen
    pub last_seen: Instant,

    /// Number of connection attempts
    pub connection_attempts: u32,

    /// When the peer was last connected
    pub last_connection: Option<Instant>,

    /// Protocol version
    pub version: Option<String>,

    /// User agent
    pub user_agent: Option<String>,

    /// Ping latency in milliseconds
    pub ping_latency: Option<u64>,

    /// Supported services
    pub services: u64,

    /// Additional addresses advertised by the peer
    pub advertised_addresses: Vec<SocketAddr>,
}

impl EnhancedPeerMetadata {
    /// Create new peer metadata
    pub fn new(node_id: PeerId, addr: SocketAddr, is_outbound: bool) -> Self {
        let now = Instant::now();

        Self {
            node_id,
            addr,
            node_info: None,
            state: if is_outbound { ConnectionState::Disconnected } else { ConnectionState::Connected },
            is_outbound,
            first_seen: now,
            last_seen: now,
            connection_attempts: 0,
            last_connection: None,
            version: None,
            user_agent: None,
            ping_latency: None,
            services: 0,
            advertised_addresses: Vec::new(),
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
        self.update_last_seen();
    }

    /// Update the connection state
    pub fn update_state(&mut self, state: ConnectionState) {
        self.state = state;
        self.update_last_seen();

        if state == ConnectionState::Failed {
            self.connection_attempts += 1;
        } else if state == ConnectionState::Ready {
            self.connection_attempts = 0;
            self.last_connection = Some(Instant::now());
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
        self.update_last_seen();
    }

    /// Add an advertised address
    pub fn add_advertised_address(&mut self, addr: SocketAddr) {
        if !self.advertised_addresses.contains(&addr) {
            self.advertised_addresses.push(addr);
        }
    }

    /// Set the supported services
    pub fn set_services(&mut self, services: u64) {
        self.services = services;
    }
}

/// Configuration for the peer registry
#[derive(Debug, Clone)]
pub struct PeerRegistryConfig {
    /// Inactivity timeout
    pub inactivity_timeout: Duration,

    /// Maximum number of failed attempts before banning
    pub max_failed_attempts: u32,

    /// Maximum number of peers
    pub max_peers: usize,

    /// Maximum number of outbound connections
    pub max_outbound: usize,

    /// Maximum number of inbound connections
    pub max_inbound: usize,

    /// Ban duration
    pub ban_duration: Duration,
}

impl Default for PeerRegistryConfig {
    fn default() -> Self {
        Self {
            inactivity_timeout: Duration::from_secs(300), // 5 minutes
            max_failed_attempts: 5,
            max_peers: 1000,
            max_outbound: 8,
            max_inbound: 32,
            ban_duration: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Advanced registry for tracking peers
pub struct AdvancedPeerRegistry {
    /// Map of peer IDs to peer metadata
    peers: DashMap<PeerId, EnhancedPeerMetadata>,

    /// Map of addresses to peer IDs
    addr_to_id: DashMap<SocketAddr, PeerId>,

    /// Performance metrics for peers
    performance: DashMap<PeerId, PeerPerformance>,

    /// Reputation system
    reputation: Arc<ReputationSystem>,

    /// Configuration
    config: PeerRegistryConfig,
}

impl AdvancedPeerRegistry {
    /// Create a new peer registry
    pub fn new() -> Self {
        Self {
            peers: DashMap::new(),
            addr_to_id: DashMap::new(),
            performance: DashMap::new(),
            reputation: Arc::new(ReputationSystem::new()),
            config: PeerRegistryConfig::default(),
        }
    }

    /// Create a new peer registry with the given configuration
    pub fn with_config(config: PeerRegistryConfig) -> Self {
        Self {
            peers: DashMap::new(),
            addr_to_id: DashMap::new(),
            performance: DashMap::new(),
            reputation: Arc::new(ReputationSystem::new()),
            config,
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

        // Check if we have reached the maximum number of peers
        if self.peers.len() >= self.config.max_peers {
            warn!("Maximum number of peers reached: {}", self.config.max_peers);
            return false;
        }

        // Check connection limits
        let outbound_count = self.peers.iter()
            .filter(|entry| entry.is_outbound && entry.is_active())
            .count();

        let inbound_count = self.peers.iter()
            .filter(|entry| !entry.is_outbound && entry.is_active())
            .count();

        if is_outbound && outbound_count >= self.config.max_outbound {
            warn!("Maximum outbound connections reached: {}", self.config.max_outbound);
            return false;
        }

        if !is_outbound && inbound_count >= self.config.max_inbound {
            warn!("Maximum inbound connections reached: {}", self.config.max_inbound);
            return false;
        }

        // Create peer metadata
        let metadata = EnhancedPeerMetadata::new(node_id.to_string(), addr, is_outbound);

        // Create performance metrics
        let performance = PeerPerformance::new();

        // Add to maps
        self.peers.insert(node_id.to_string(), metadata);
        self.addr_to_id.insert(addr, node_id.to_string());
        self.performance.insert(node_id.to_string(), performance);

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
        self.performance.remove(node_id);

        debug!("Unregistered peer {} at {}", node_id, addr);
        true
    }

    /// Update a peer's state
    pub fn update_peer_state(&self, node_id: &str, state: ConnectionState) -> bool {
        if let Some(mut entry) = self.peers.get_mut(node_id) {
            entry.update_state(state);

            // If the peer has too many failed attempts, ban it
            if state == ConnectionState::Failed && entry.connection_attempts >= self.config.max_failed_attempts {
                entry.update_state(ConnectionState::Banned);
                warn!("Banned peer {} after {} failed attempts", node_id, entry.connection_attempts);

                // Update reputation
                self.reputation.update_score(node_id, ReputationEvent::ConnectionFailure);
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

            // Update performance metrics
            if let Some(mut perf) = self.performance.get_mut(node_id) {
                perf.update_ping_latency(latency);
            }

            true
        } else {
            false
        }
    }

    /// Update a peer's reputation
    pub fn update_peer_reputation(&self, node_id: &str, event: ReputationEvent) -> bool {
        self.reputation.update_score(node_id, event)
    }

    /// Get a peer's metadata
    pub fn get_peer_metadata(&self, node_id: &str) -> Option<EnhancedPeerMetadata> {
        self.peers.get(node_id).map(|entry| entry.clone())
    }

    /// Get a peer's address
    pub fn get_peer_addr(&self, node_id: &str) -> Option<SocketAddr> {
        self.peers.get(node_id).map(|entry| entry.addr)
    }

    /// Get a peer's node ID by address
    pub fn get_peer_id(&self, addr: &SocketAddr) -> Option<PeerId> {
        self.addr_to_id.get(addr).map(|entry| entry.clone())
    }

    /// Get a peer's performance metrics
    pub fn get_peer_performance(&self, node_id: &str) -> Option<PeerPerformance> {
        self.performance.get(node_id).map(|entry| entry.clone())
    }

    /// Get a peer's reputation score
    pub fn get_peer_reputation(&self, node_id: &str) -> i32 {
        self.reputation.get_score(node_id)
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
            .unwrap_or(false) || self.reputation.is_banned(node_id)
    }

    /// Get all active peers
    pub fn get_active_peers(&self) -> Vec<EnhancedPeerMetadata> {
        self.peers.iter()
            .filter(|entry| entry.is_active())
            .map(|entry| entry.clone())
            .collect()
    }

    /// Get all peers
    pub fn get_all_peers(&self) -> Vec<EnhancedPeerMetadata> {
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

    /// Get the number of outbound peers
    pub fn outbound_peer_count(&self) -> usize {
        self.peers.iter()
            .filter(|entry| entry.is_outbound)
            .count()
    }

    /// Get the number of inbound peers
    pub fn inbound_peer_count(&self) -> usize {
        self.peers.iter()
            .filter(|entry| !entry.is_outbound)
            .count()
    }

    /// Clean up inactive peers
    pub fn cleanup_inactive_peers(&self) -> usize {
        let inactive_peers: Vec<PeerId> = self.peers.iter()
            .filter(|entry| entry.is_inactive(self.config.inactivity_timeout))
            .map(|entry| entry.node_id.clone())
            .collect();

        for node_id in &inactive_peers {
            self.unregister_peer(node_id);
        }

        inactive_peers.len()
    }

    /// Ban a peer
    pub fn ban_peer(&self, node_id: &str) -> bool {
        // Update peer state
        let state_updated = self.update_peer_state(node_id, ConnectionState::Banned);

        // Update reputation
        let reputation_updated = self.reputation.ban_peer(node_id);

        state_updated || reputation_updated
    }

    /// Unban a peer
    pub fn unban_peer(&self, node_id: &str) -> bool {
        // Update peer state
        let mut state_updated = false;
        if let Some(mut entry) = self.peers.get_mut(node_id) {
            if entry.is_banned() {
                entry.update_state(ConnectionState::Disconnected);
                entry.connection_attempts = 0;
                state_updated = true;
            }
        }

        // Update reputation
        let reputation_updated = self.reputation.unban_peer(node_id);

        state_updated || reputation_updated
    }

    /// Get peers for outbound connection
    pub fn get_peers_for_outbound(&self, count: usize) -> Vec<SocketAddr> {
        let mut candidates = Vec::new();

        // First, try to get peers that are disconnected but have been connected before
        for entry in self.peers.iter() {
            if entry.is_outbound && entry.is_disconnected() && entry.last_connection.is_some() {
                candidates.push((entry.addr, entry.last_seen, true));
            }
        }

        // If we don't have enough, add disconnected peers that have never been connected
        if candidates.len() < count {
            for entry in self.peers.iter() {
                if entry.is_outbound && entry.is_disconnected() && entry.last_connection.is_none() {
                    candidates.push((entry.addr, entry.first_seen, false));
                }
            }
        }

        // Sort by preference (previously connected first, then by last seen)
        candidates.sort_by(|a, b| {
            // Previously connected peers first
            if a.2 && !b.2 {
                return std::cmp::Ordering::Less;
            }
            if !a.2 && b.2 {
                return std::cmp::Ordering::Greater;
            }

            // Then by last seen (most recent first)
            b.1.cmp(&a.1)
        });

        // Take the requested number of peers
        candidates.iter()
            .take(count)
            .map(|(addr, _, _)| *addr)
            .collect()
    }

    /// Get the best peers for syncing
    pub fn get_best_sync_peers(&self, count: usize) -> Vec<PeerId> {
        let mut candidates = Vec::new();

        // Get active peers with their performance metrics
        for entry in self.peers.iter() {
            if entry.is_active() {
                if let Some(perf) = self.performance.get(&entry.node_id) {
                    // Calculate a score based on latency and success rate
                    let latency_score = entry.ping_latency.unwrap_or(1000);
                    let success_rate = perf.success_rate();

                    // Lower is better
                    let score = if success_rate > 0.0 {
                        latency_score as f64 / success_rate
                    } else {
                        f64::MAX
                    };

                    candidates.push((entry.node_id.clone(), score));
                }
            }
        }

        // Sort by score (lower is better)
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take the requested number of peers
        candidates.iter()
            .take(count)
            .map(|(node_id, _)| node_id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_advanced_peer_registry() {
        // Create a registry
        let registry = AdvancedPeerRegistry::new();

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

        // Update peer reputation
        registry.update_peer_reputation("peer1", ReputationEvent::GoodBlock);

        // Check reputation score
        let score = registry.get_peer_reputation("peer1");
        assert!(score > 0);

        // Unregister the peer
        assert!(registry.unregister_peer("peer1"));

        // Check that the peer is unregistered
        assert_eq!(registry.total_peer_count(), 0);
    }

    #[test]
    fn test_peer_banning() {
        // Create a registry
        let registry = AdvancedPeerRegistry::new();

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
        let config = PeerRegistryConfig {
            max_failed_attempts: 2,
            ..PeerRegistryConfig::default()
        };
        let registry = AdvancedPeerRegistry::with_config(config);

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
