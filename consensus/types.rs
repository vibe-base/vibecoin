use serde::{Serialize, Deserialize};
use std::fmt;
use std::cmp::Ordering;
use num_bigint::BigUint;
use num_traits::Zero;

use crate::crypto::hash::Hash;
use crate::storage::block_store::Block;

/// Difficulty target for Proof of Work
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Target {
    /// The target value as a big unsigned integer
    value: BigUint,
}

impl Target {
    /// Create a new target from a difficulty value
    pub fn from_difficulty(difficulty: u64) -> Self {
        if difficulty == 0 {
            return Self { value: BigUint::zero() };
        }
        
        // Max target (easiest): 2^256 - 1
        let max_target = BigUint::from(1u8) << 256;
        
        // Target = max_target / difficulty
        let value = max_target / BigUint::from(difficulty);
        
        Self { value }
    }
    
    /// Check if a hash meets the target
    pub fn is_met_by(&self, hash: &[u8; 32]) -> bool {
        // Convert hash to BigUint
        let hash_int = BigUint::from_bytes_be(hash);
        
        // Check if hash < target
        hash_int < self.value
    }
    
    /// Get the difficulty corresponding to this target
    pub fn to_difficulty(&self) -> u64 {
        if self.value.is_zero() {
            return 0;
        }
        
        // Max target (easiest): 2^256 - 1
        let max_target = BigUint::from(1u8) << 256;
        
        // Difficulty = max_target / target
        let difficulty = max_target / &self.value;
        
        // Convert to u64, capping at u64::MAX if necessary
        match difficulty.to_u64() {
            Some(d) => d,
            None => u64::MAX,
        }
    }
    
    /// Get the target as bytes
    pub fn as_bytes(&self) -> Vec<u8> {
        self.value.to_bytes_be()
    }
    
    /// Create a target from bytes
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            value: BigUint::from_bytes_be(bytes),
        }
    }
}

impl fmt::Debug for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Target(difficulty={})", self.to_difficulty())
    }
}

/// Chain state for consensus decisions
#[derive(Clone, Debug)]
pub struct ChainState {
    /// The current blockchain height
    pub height: u64,
    
    /// The current target difficulty
    pub current_target: Target,
    
    /// The latest block hash
    pub latest_hash: [u8; 32],
    
    /// The latest block timestamp
    pub latest_timestamp: u64,
    
    /// The latest PoH sequence number
    pub latest_poh_sequence: u64,
}

impl ChainState {
    /// Create a new chain state from the genesis block
    pub fn new_from_genesis(genesis: &Block) -> Self {
        Self {
            height: 0,
            current_target: Target::from_difficulty(1),
            latest_hash: genesis.hash,
            latest_timestamp: genesis.timestamp,
            latest_poh_sequence: 0,
        }
    }
    
    /// Update the chain state with a new block
    pub fn update_with_block(&mut self, block: &Block) {
        self.height = block.height;
        self.latest_hash = block.hash;
        self.latest_timestamp = block.timestamp;
        // PoH sequence would be updated separately
    }
}

/// Block template for mining
#[derive(Clone, Debug)]
pub struct BlockTemplate {
    /// Block height
    pub height: u64,
    
    /// Previous block hash
    pub prev_hash: [u8; 32],
    
    /// Block timestamp
    pub timestamp: u64,
    
    /// Transactions to include
    pub transactions: Vec<[u8; 32]>,
    
    /// State root
    pub state_root: [u8; 32],
    
    /// Target difficulty
    pub target: Target,
    
    /// PoH sequence start
    pub poh_sequence_start: u64,
}

impl BlockTemplate {
    /// Convert to a block with the given nonce and PoH data
    pub fn to_block(&self, nonce: u64, poh_hash: [u8; 32]) -> Block {
        // In a real implementation, we would compute the block hash here
        // For now, we'll just use a placeholder
        let hash = [0u8; 32]; // This would be computed based on block contents
        
        Block {
            height: self.height,
            hash,
            prev_hash: self.prev_hash,
            timestamp: self.timestamp,
            transactions: self.transactions.clone(),
            state_root: self.state_root,
            // Additional fields would be added to Block struct
        }
    }
}

/// Fork choice rule result
#[derive(Debug, PartialEq, Eq)]
pub enum ForkChoice {
    /// Accept the new block
    Accept,
    
    /// Reject the new block
    Reject,
    
    /// The new block forms a fork that needs resolution
    Fork,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_target_difficulty_conversion() {
        let difficulties = vec![1, 10, 100, 1000, 10000, 100000];
        
        for difficulty in difficulties {
            let target = Target::from_difficulty(difficulty);
            let converted = target.to_difficulty();
            
            // Due to integer division, there might be small differences
            // We'll allow a small margin of error
            let ratio = if converted > difficulty {
                converted as f64 / difficulty as f64
            } else {
                difficulty as f64 / converted as f64
            };
            
            assert!(ratio < 1.01, "Conversion error too large: {} -> {}", difficulty, converted);
        }
    }
    
    #[test]
    fn test_target_hash_check() {
        // Create a target with difficulty 16
        let target = Target::from_difficulty(16);
        
        // A hash with all zeros should meet any reasonable target
        let zero_hash = [0u8; 32];
        assert!(target.is_met_by(&zero_hash));
        
        // A hash with the first byte as 0x10 (16) should not meet the target
        // because 16 * 2^248 is greater than 2^256 / 16
        let mut hard_hash = [0u8; 32];
        hard_hash[0] = 0x10;
        assert!(!target.is_met_by(&hard_hash));
    }
}
