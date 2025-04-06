use std::sync::Arc;
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::consensus::types::{ChainState, ForkChoice};

/// Determine the canonical chain when there are competing forks
pub fn choose_fork(
    block_store: &BlockStore<'_>,
    current_state: &ChainState,
    new_block: &Block,
) -> ForkChoice {
    // If the new block builds on the current chain, accept it
    if new_block.prev_hash == current_state.latest_hash {
        return ForkChoice::Accept;
    }
    
    // If the new block is at a lower height, reject it
    if new_block.height < current_state.height {
        return ForkChoice::Reject;
    }
    
    // If the new block is at the same height, we have a fork
    if new_block.height == current_state.height {
        // In a real implementation, we would compare the cumulative work
        // of each chain to decide which one to follow
        // For now, we'll just reject the new block
        return ForkChoice::Reject;
    }
    
    // If the new block is at a higher height, we need to check if it's
    // part of a valid chain
    let mut current_block = new_block.clone();
    
    // Traverse backwards until we find a common ancestor
    while current_block.height > current_state.height {
        // Get the parent block
        match block_store.get_block_by_hash(&current_block.prev_hash) {
            Some(parent) => {
                current_block = parent;
            }
            None => {
                // If we can't find the parent, we can't validate the chain
                return ForkChoice::Fork;
            }
        }
    }
    
    // Now current_block is at the same height as current_state
    if current_block.hash == current_state.latest_hash {
        // The new block is part of a longer chain, accept it
        return ForkChoice::Accept;
    } else {
        // We have a fork, need to decide which chain to follow
        // In a real implementation, we would compare the cumulative work
        // For now, we'll just consider it a fork
        return ForkChoice::Fork;
    }
}

/// Resolve a fork by comparing cumulative work
pub fn resolve_fork(
    block_store: &BlockStore<'_>,
    fork1_tip: &Block,
    fork2_tip: &Block,
) -> Block {
    // In a real implementation, we would:
    // 1. Find the common ancestor of the two forks
    // 2. Calculate the cumulative work of each fork
    // 3. Choose the fork with more work
    // For now, we'll just choose the block with the higher height
    if fork1_tip.height >= fork2_tip.height {
        fork1_tip.clone()
    } else {
        fork2_tip.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use crate::consensus::types::Target;
    use tempfile::tempdir;
    
    #[test]
    fn test_fork_choice() {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = BlockStore::new(&kv_store);
        
        // Create a chain state
        let chain_state = ChainState {
            height: 10,
            current_target: Target::from_difficulty(100),
            latest_hash: [10u8; 32],
            latest_timestamp: 100,
            latest_poh_sequence: 1000,
        };
        
        // Create a block that builds on the current chain
        let block1 = Block {
            height: 11,
            hash: [11u8; 32],
            prev_hash: [10u8; 32], // Points to the current tip
            timestamp: 110,
            transactions: vec![],
            state_root: [0u8; 32],
        };
        
        // Create a block that forms a fork
        let block2 = Block {
            height: 11,
            hash: [99u8; 32],
            prev_hash: [9u8; 32], // Points to a different block
            timestamp: 110,
            transactions: vec![],
            state_root: [0u8; 32],
        };
        
        // Store the blocks
        block_store.put_block(&block1);
        block_store.put_block(&block2);
        
        // Test the fork choice rule
        let choice1 = choose_fork(&block_store, &chain_state, &block1);
        let choice2 = choose_fork(&block_store, &chain_state, &block2);
        
        // Block 1 should be accepted
        assert_eq!(choice1, ForkChoice::Accept);
        
        // Block 2 should be considered a fork
        assert_eq!(choice2, ForkChoice::Fork);
    }
}
