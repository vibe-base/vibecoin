use log::{debug, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::consensus::types::{Target, ConsensusConfig};

/// Difficulty adjuster
pub struct DifficultyAdjuster<'a> {
    /// Block store
    block_store: &'a BlockStore<'a>,
    
    /// Configuration
    config: ConsensusConfig,
}

impl<'a> DifficultyAdjuster<'a> {
    /// Create a new difficulty adjuster
    pub fn new(block_store: &'a BlockStore<'a>, config: ConsensusConfig) -> Self {
        Self {
            block_store,
            config,
        }
    }
    
    /// Calculate the next difficulty
    pub fn calculate_next_difficulty(&self, latest_block: &Block) -> u64 {
        // Check if we need to adjust the difficulty
        if latest_block.height % self.config.difficulty_adjustment_interval != 0 {
            // No adjustment needed
            return latest_block.difficulty;
        }
        
        // Get the block at the start of the adjustment period
        let adjustment_height = latest_block.height.saturating_sub(self.config.difficulty_adjustment_interval);
        let start_block = match self.block_store.get_block_by_height(adjustment_height) {
            Some(block) => block,
            None => {
                warn!("Could not find block at height {}, using latest block", adjustment_height);
                return latest_block.difficulty;
            }
        };
        
        // Calculate the time taken for the adjustment period
        let time_taken = latest_block.timestamp.saturating_sub(start_block.timestamp);
        let target_time = self.config.target_block_time * self.config.difficulty_adjustment_interval;
        
        // Adjust the difficulty based on the time taken
        let mut new_difficulty = (latest_block.difficulty as u128 * target_time as u128 / time_taken as u128) as u64;
        
        // Limit the adjustment to a factor of 4
        let max_adjustment = latest_block.difficulty * 4;
        let min_adjustment = latest_block.difficulty / 4;
        
        new_difficulty = new_difficulty.min(max_adjustment).max(min_adjustment);
        
        // Ensure the difficulty is at least 1
        new_difficulty.max(1)
    }
    
    /// Calculate the target from a difficulty
    pub fn calculate_target(&self, difficulty: u64) -> Target {
        Target::from_difficulty(difficulty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;
    
    #[test]
    fn test_difficulty_adjustment() {
        // Create a block store
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = BlockStore::new(&kv_store);
        
        // Create a config with a small adjustment interval
        let mut config = ConsensusConfig::default();
        config.difficulty_adjustment_interval = 10;
        config.target_block_time = 60; // 1 minute
        
        // Create a difficulty adjuster
        let adjuster = DifficultyAdjuster::new(&block_store, config);
        
        // Create a block at the adjustment height
        let start_block = Block {
            height: 10,
            hash: [1; 32],
            prev_hash: [0; 32],
            timestamp: 12345,
            transactions: vec![],
            state_root: [0; 32],
            poh_hash: [0; 32],
            poh_seq: 100,
            nonce: 0,
            difficulty: 1000,
            total_difficulty: 10000,
        };
        
        // Store the block
        block_store.put_block(&start_block).unwrap();
        
        // Create a block at the next adjustment height
        
        // Case 1: Blocks are being mined too quickly (half the target time)
        let fast_block = Block {
            height: 20,
            hash: [2; 32],
            prev_hash: [1; 32],
            timestamp: 12345 + (60 * 10 / 2), // Half the target time
            transactions: vec![],
            state_root: [0; 32],
            poh_hash: [0; 32],
            poh_seq: 200,
            nonce: 0,
            difficulty: 1000,
            total_difficulty: 20000,
        };
        
        // The difficulty should increase
        let new_difficulty = adjuster.calculate_next_difficulty(&fast_block);
        assert!(new_difficulty > fast_block.difficulty);
        
        // Case 2: Blocks are being mined too slowly (twice the target time)
        let slow_block = Block {
            height: 20,
            hash: [3; 32],
            prev_hash: [1; 32],
            timestamp: 12345 + (60 * 10 * 2), // Twice the target time
            transactions: vec![],
            state_root: [0; 32],
            poh_hash: [0; 32],
            poh_seq: 200,
            nonce: 0,
            difficulty: 1000,
            total_difficulty: 20000,
        };
        
        // The difficulty should decrease
        let new_difficulty = adjuster.calculate_next_difficulty(&slow_block);
        assert!(new_difficulty < slow_block.difficulty);
        
        // Case 3: Blocks are being mined at the target time
        let target_block = Block {
            height: 20,
            hash: [4; 32],
            prev_hash: [1; 32],
            timestamp: 12345 + (60 * 10), // Exactly the target time
            transactions: vec![],
            state_root: [0; 32],
            poh_hash: [0; 32],
            poh_seq: 200,
            nonce: 0,
            difficulty: 1000,
            total_difficulty: 20000,
        };
        
        // The difficulty should stay the same
        let new_difficulty = adjuster.calculate_next_difficulty(&target_block);
        assert_eq!(new_difficulty, target_block.difficulty);
    }
}
