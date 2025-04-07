use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

use crate::storage::block_store::Hash;
use crate::consensus::types::Target;

/// Block header structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Block height
    pub height: u64,
    
    /// Previous block hash
    pub prev_hash: Hash,
    
    /// State root hash
    pub state_root: Hash,
    
    /// Transaction root hash
    pub tx_root: Hash,
    
    /// Block timestamp
    pub timestamp: u64,
    
    /// Proof of History sequence number
    pub poh_seq: u64,
    
    /// Proof of History hash
    pub poh_hash: Hash,
    
    /// Proof of Work nonce
    pub nonce: u64,
    
    /// Block difficulty
    pub difficulty: u64,
    
    /// Total cumulative difficulty
    pub total_difficulty: u128,
}

impl BlockHeader {
    /// Create a new block header
    pub fn new(
        height: u64,
        prev_hash: Hash,
        state_root: Hash,
        tx_root: Hash,
        timestamp: u64,
        poh_seq: u64,
        poh_hash: Hash,
        difficulty: u64,
        prev_total_difficulty: u128,
    ) -> Self {
        Self {
            height,
            prev_hash,
            state_root,
            tx_root,
            timestamp,
            poh_seq,
            poh_hash,
            nonce: 0, // Will be set during mining
            difficulty,
            total_difficulty: prev_total_difficulty + difficulty as u128,
        }
    }
    
    /// Calculate the hash of this header
    pub fn hash(&self) -> Hash {
        let serialized = bincode::serialize(self).expect("Failed to serialize block header");
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        let result = hasher.finalize();
        
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
    
    /// Check if the header meets the given target
    pub fn meets_target(&self, target: &Target) -> bool {
        let hash = self.hash();
        target.is_met_by(&hash)
    }
    
    /// Set the nonce
    pub fn set_nonce(&mut self, nonce: u64) {
        self.nonce = nonce;
    }
    
    /// Get the block time in seconds since the previous block
    pub fn block_time(&self, prev_timestamp: u64) -> u64 {
        if self.timestamp <= prev_timestamp {
            return 1; // Ensure at least 1 second difference
        }
        self.timestamp - prev_timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_block_header_hash() {
        let header = BlockHeader {
            height: 1,
            prev_hash: [0; 32],
            state_root: [1; 32],
            tx_root: [2; 32],
            timestamp: 12345,
            poh_seq: 100,
            poh_hash: [3; 32],
            nonce: 42,
            difficulty: 1000,
            total_difficulty: 1000,
        };
        
        let hash = header.hash();
        
        // Hash should not be all zeros
        assert_ne!(hash, [0; 32]);
        
        // Changing the nonce should change the hash
        let mut header2 = header.clone();
        header2.set_nonce(43);
        let hash2 = header2.hash();
        
        assert_ne!(hash, hash2);
    }
    
    #[test]
    fn test_meets_target() {
        let mut header = BlockHeader {
            height: 1,
            prev_hash: [0; 32],
            state_root: [1; 32],
            tx_root: [2; 32],
            timestamp: 12345,
            poh_seq: 100,
            poh_hash: [3; 32],
            nonce: 0,
            difficulty: 1,
            total_difficulty: 1,
        };
        
        // With difficulty 1, almost any hash should meet the target
        let target = Target::from_difficulty(1);
        
        // Try different nonces until we find one that meets the target
        let mut found = false;
        for nonce in 0..1000 {
            header.set_nonce(nonce);
            if header.meets_target(&target) {
                found = true;
                break;
            }
        }
        
        assert!(found, "Could not find a nonce that meets the target");
    }
}
