use std::collections::HashMap;
use std::sync::RwLock;
use log::{debug, info, warn};

use crate::network::peer::advanced_registry::PeerId;

/// Reputation events that affect a peer's score
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationEvent {
    /// Peer provided a good block
    GoodBlock,
    
    /// Peer provided an invalid block
    InvalidBlock,
    
    /// Peer provided a good transaction
    GoodTransaction,
    
    /// Peer provided an invalid transaction
    InvalidTransaction,
    
    /// Peer violated the protocol
    ProtocolViolation,
    
    /// Peer connection failed
    ConnectionFailure,
    
    /// Peer timed out
    Timeout,
    
    /// Peer responded quickly
    QuickResponse,
    
    /// Peer responded slowly
    SlowResponse,
    
    /// Peer provided useful data
    UsefulData,
    
    /// Peer provided duplicate data
    DuplicateData,
}

/// Thresholds for reputation scores
#[derive(Debug, Clone, Copy)]
pub struct ReputationThresholds {
    /// Score below which a peer is banned
    pub ban_threshold: i32,
    
    /// Score below which a peer is on probation
    pub probation_threshold: i32,
    
    /// Score above which a peer is in good standing
    pub good_standing_threshold: i32,
    
    /// Score above which a peer is preferred
    pub preferred_threshold: i32,
}

impl Default for ReputationThresholds {
    fn default() -> Self {
        Self {
            ban_threshold: -100,
            probation_threshold: -50,
            good_standing_threshold: 0,
            preferred_threshold: 50,
        }
    }
}

/// Reputation system for tracking peer behavior
pub struct ReputationSystem {
    /// Peer reputation scores
    scores: RwLock<HashMap<PeerId, i32>>,
    
    /// Banned peers
    banned: RwLock<HashMap<PeerId, bool>>,
    
    /// Reputation thresholds
    thresholds: ReputationThresholds,
}

impl ReputationSystem {
    /// Create a new reputation system
    pub fn new() -> Self {
        Self {
            scores: RwLock::new(HashMap::new()),
            banned: RwLock::new(HashMap::new()),
            thresholds: ReputationThresholds::default(),
        }
    }
    
    /// Create a new reputation system with custom thresholds
    pub fn with_thresholds(thresholds: ReputationThresholds) -> Self {
        Self {
            scores: RwLock::new(HashMap::new()),
            banned: RwLock::new(HashMap::new()),
            thresholds,
        }
    }
    
    /// Update a peer's reputation score
    pub fn update_score(&self, peer_id: &str, event: ReputationEvent) -> bool {
        let score_change = match event {
            ReputationEvent::GoodBlock => 10,
            ReputationEvent::InvalidBlock => -100,
            ReputationEvent::GoodTransaction => 1,
            ReputationEvent::InvalidTransaction => -10,
            ReputationEvent::ProtocolViolation => -50,
            ReputationEvent::ConnectionFailure => -5,
            ReputationEvent::Timeout => -2,
            ReputationEvent::QuickResponse => 1,
            ReputationEvent::SlowResponse => -1,
            ReputationEvent::UsefulData => 5,
            ReputationEvent::DuplicateData => -2,
        };
        
        // Update the score
        let mut scores = self.scores.write().unwrap();
        let score = scores.entry(peer_id.to_string()).or_insert(0);
        *score += score_change;
        
        // Clamp the score to reasonable bounds
        *score = score.max(-1000).min(1000);
        
        // Check if the peer should be banned
        if *score <= self.thresholds.ban_threshold {
            debug!("Peer {} banned due to low reputation score: {}", peer_id, *score);
            let mut banned = self.banned.write().unwrap();
            banned.insert(peer_id.to_string(), true);
        }
        
        true
    }
    
    /// Get a peer's reputation score
    pub fn get_score(&self, peer_id: &str) -> i32 {
        let scores = self.scores.read().unwrap();
        *scores.get(peer_id).unwrap_or(&0)
    }
    
    /// Check if a peer is banned
    pub fn is_banned(&self, peer_id: &str) -> bool {
        let banned = self.banned.read().unwrap();
        *banned.get(peer_id).unwrap_or(&false)
    }
    
    /// Ban a peer
    pub fn ban_peer(&self, peer_id: &str) -> bool {
        let mut banned = self.banned.write().unwrap();
        let was_banned = banned.insert(peer_id.to_string(), true).unwrap_or(false);
        
        // Update the score to ensure it's below the ban threshold
        let mut scores = self.scores.write().unwrap();
        let score = scores.entry(peer_id.to_string()).or_insert(0);
        *score = self.thresholds.ban_threshold - 1;
        
        !was_banned
    }
    
    /// Unban a peer
    pub fn unban_peer(&self, peer_id: &str) -> bool {
        let mut banned = self.banned.write().unwrap();
        let was_banned = banned.remove(peer_id).unwrap_or(false);
        
        // Reset the score to probation level
        if was_banned {
            let mut scores = self.scores.write().unwrap();
            let score = scores.entry(peer_id.to_string()).or_insert(0);
            *score = self.thresholds.probation_threshold;
        }
        
        was_banned
    }
    
    /// Check if a peer is on probation
    pub fn is_on_probation(&self, peer_id: &str) -> bool {
        let score = self.get_score(peer_id);
        score <= self.thresholds.probation_threshold && score > self.thresholds.ban_threshold
    }
    
    /// Check if a peer is in good standing
    pub fn is_in_good_standing(&self, peer_id: &str) -> bool {
        let score = self.get_score(peer_id);
        score >= self.thresholds.good_standing_threshold
    }
    
    /// Check if a peer is preferred
    pub fn is_preferred(&self, peer_id: &str) -> bool {
        let score = self.get_score(peer_id);
        score >= self.thresholds.preferred_threshold
    }
    
    /// Get all banned peers
    pub fn get_banned_peers(&self) -> Vec<PeerId> {
        let banned = self.banned.read().unwrap();
        banned.iter()
            .filter(|(_, &is_banned)| is_banned)
            .map(|(peer_id, _)| peer_id.clone())
            .collect()
    }
    
    /// Get all peers on probation
    pub fn get_probation_peers(&self) -> Vec<(PeerId, i32)> {
        let scores = self.scores.read().unwrap();
        scores.iter()
            .filter(|(peer_id, &score)| {
                score <= self.thresholds.probation_threshold && 
                score > self.thresholds.ban_threshold &&
                !self.is_banned(peer_id)
            })
            .map(|(peer_id, &score)| (peer_id.clone(), score))
            .collect()
    }
    
    /// Get all preferred peers
    pub fn get_preferred_peers(&self) -> Vec<(PeerId, i32)> {
        let scores = self.scores.read().unwrap();
        scores.iter()
            .filter(|(_, &score)| score >= self.thresholds.preferred_threshold)
            .map(|(peer_id, &score)| (peer_id.clone(), score))
            .collect()
    }
    
    /// Reset a peer's reputation
    pub fn reset_peer(&self, peer_id: &str) -> bool {
        let mut scores = self.scores.write().unwrap();
        let reset = scores.remove(peer_id).is_some();
        
        let mut banned = self.banned.write().unwrap();
        let unbanned = banned.remove(peer_id).unwrap_or(false);
        
        reset || unbanned
    }
    
    /// Get the number of banned peers
    pub fn banned_count(&self) -> usize {
        let banned = self.banned.read().unwrap();
        banned.values().filter(|&&is_banned| is_banned).count()
    }
    
    /// Get the number of peers on probation
    pub fn probation_count(&self) -> usize {
        let scores = self.scores.read().unwrap();
        scores.iter()
            .filter(|(peer_id, &score)| {
                score <= self.thresholds.probation_threshold && 
                score > self.thresholds.ban_threshold &&
                !self.is_banned(peer_id)
            })
            .count()
    }
    
    /// Get the number of preferred peers
    pub fn preferred_count(&self) -> usize {
        let scores = self.scores.read().unwrap();
        scores.iter()
            .filter(|(_, &score)| score >= self.thresholds.preferred_threshold)
            .count()
    }
}

impl Default for ReputationSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_reputation_system() {
        let system = ReputationSystem::new();
        
        // Test initial state
        assert_eq!(system.get_score("peer1"), 0);
        assert!(!system.is_banned("peer1"));
        
        // Test good events
        system.update_score("peer1", ReputationEvent::GoodBlock);
        assert_eq!(system.get_score("peer1"), 10);
        
        system.update_score("peer1", ReputationEvent::GoodTransaction);
        assert_eq!(system.get_score("peer1"), 11);
        
        // Test bad events
        system.update_score("peer2", ReputationEvent::InvalidBlock);
        assert_eq!(system.get_score("peer2"), -100);
        
        // Check if peer2 is banned
        assert!(system.is_banned("peer2"));
        
        // Test unbanning
        system.unban_peer("peer2");
        assert!(!system.is_banned("peer2"));
        
        // Check that the score was reset to probation level
        assert_eq!(system.get_score("peer2"), system.thresholds.probation_threshold);
        
        // Test manual banning
        system.ban_peer("peer3");
        assert!(system.is_banned("peer3"));
        assert!(system.get_score("peer3") <= system.thresholds.ban_threshold);
    }
    
    #[test]
    fn test_reputation_categories() {
        let system = ReputationSystem::new();
        
        // Set up peers with different scores
        system.update_score("good", ReputationEvent::GoodBlock);
        system.update_score("good", ReputationEvent::GoodBlock);
        system.update_score("good", ReputationEvent::GoodBlock);
        system.update_score("good", ReputationEvent::GoodBlock);
        system.update_score("good", ReputationEvent::GoodBlock);
        
        system.update_score("neutral", ReputationEvent::GoodTransaction);
        
        system.update_score("probation", ReputationEvent::InvalidTransaction);
        system.update_score("probation", ReputationEvent::InvalidTransaction);
        system.update_score("probation", ReputationEvent::InvalidTransaction);
        
        system.update_score("banned", ReputationEvent::InvalidBlock);
        
        // Check categories
        assert!(system.is_preferred("good"));
        assert!(system.is_in_good_standing("neutral"));
        assert!(system.is_on_probation("probation"));
        assert!(system.is_banned("banned"));
        
        // Check counts
        assert_eq!(system.preferred_count(), 1);
        assert_eq!(system.probation_count(), 1);
        assert_eq!(system.banned_count(), 1);
    }
}
