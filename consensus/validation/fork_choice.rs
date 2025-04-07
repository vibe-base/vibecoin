use std::sync::Arc;
use std::collections::HashMap;
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::consensus::types::ChainState;

/// Fork choice result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkChoice {
    /// Accept the new block
    Accept,

    /// Reject the new block
    Reject,

    /// Replace the current chain with the new block's chain
    Replace,

    /// Fork detected, needs resolution
    Fork,
}

/// Determine the canonical chain when there are competing forks
pub fn choose_fork(
    block_store: &BlockStore<'_>,
    current_state: &ChainState,
    new_block: &Block,
) -> ForkChoice {
    // If the new block builds on the current chain, accept it
    if new_block.prev_hash == current_state.tip_hash {
        return ForkChoice::Accept;
    }

    // If the new block is at a lower height, reject it
    if new_block.height < current_state.height {
        return ForkChoice::Reject;
    }

    // If the new block is at the same height, we have a fork
    if new_block.height == current_state.height {
        // Compare the total difficulty (cumulative work)
        let current_tip = match block_store.get_block_by_hash(&current_state.tip_hash) {
            Ok(Some(block)) => block,
            _ => {
                error!("Failed to get current tip block");
                return ForkChoice::Reject;
            }
        };

        if new_block.total_difficulty > current_tip.total_difficulty {
            // The new block has more work, consider it a fork to be resolved
            return ForkChoice::Fork;
        } else {
            // The current chain has more work, reject the new block
            return ForkChoice::Reject;
        }
    }

    // If the new block is at a higher height, we need to check if it's
    // part of a valid chain
    let mut current_block = new_block.clone();
    let mut common_ancestor: Option<Block> = None;

    // Traverse backwards until we find a common ancestor
    while current_block.height > current_state.height {
        // Get the parent block
        match block_store.get_block_by_hash(&current_block.prev_hash) {
            Ok(Some(parent)) => {
                current_block = parent;
            },
            Ok(None) => {
                // If we can't find the parent, we can't validate the chain
                warn!("Cannot find parent block at height {}", current_block.height - 1);
                return ForkChoice::Fork;
            },
            Err(e) => {
                error!("Error getting parent block: {}", e);
                return ForkChoice::Reject;
            }
        }
    }

    // Now current_block is at the same height as current_state
    let current_tip = match block_store.get_block_by_hash(&current_state.tip_hash) {
        Ok(Some(block)) => block,
        Ok(None) => {
            error!("Current tip block not found");
            return ForkChoice::Reject;
        },
        Err(e) => {
            error!("Error getting current tip block: {}", e);
            return ForkChoice::Reject;
        }
    };

    if current_block.hash == current_state.tip_hash {
        // The new block is part of a longer chain, accept it
        return ForkChoice::Accept;
    } else {
        // We have a fork, need to decide which chain to follow based on work
        if new_block.total_difficulty > current_tip.total_difficulty {
            // The new chain has more work, accept it
            return ForkChoice::Accept;
        } else if new_block.total_difficulty < current_tip.total_difficulty {
            // The current chain has more work, reject the new block
            return ForkChoice::Reject;
        } else {
            // Equal work, consider it a fork to be resolved
            return ForkChoice::Fork;
        }
    }
}

/// Resolve a fork by comparing cumulative work
pub fn resolve_fork(
    block_store: &BlockStore<'_>,
    fork1_tip: &Block,
    fork2_tip: &Block,
) -> Result<Block, String> {
    // Find the common ancestor of the two forks
    let common_ancestor = find_common_ancestor(block_store, fork1_tip, fork2_tip)?;

    info!("Found common ancestor at height {}", common_ancestor.height);

    // Compare the total difficulty (cumulative work)
    if fork1_tip.total_difficulty > fork2_tip.total_difficulty {
        info!("Fork 1 has more work ({} vs {})", fork1_tip.total_difficulty, fork2_tip.total_difficulty);
        Ok(fork1_tip.clone())
    } else if fork2_tip.total_difficulty > fork1_tip.total_difficulty {
        info!("Fork 2 has more work ({} vs {})", fork2_tip.total_difficulty, fork1_tip.total_difficulty);
        Ok(fork2_tip.clone())
    } else {
        // If the work is equal, choose the chain with the lower hash (deterministic tie-breaking)
        if fork1_tip.hash <= fork2_tip.hash {
            info!("Equal work, choosing fork 1 based on hash");
            Ok(fork1_tip.clone())
        } else {
            info!("Equal work, choosing fork 2 based on hash");
            Ok(fork2_tip.clone())
        }
    }
}

/// Find the common ancestor of two blocks
pub fn find_common_ancestor(
    block_store: &BlockStore<'_>,
    block1: &Block,
    block2: &Block,
) -> Result<Block, String> {
    // If one block is an ancestor of the other, return it
    if block1.hash == block2.hash {
        return Ok(block1.clone());
    }

    // Create maps to track visited blocks
    let mut visited1 = HashMap::new();
    let mut visited2 = HashMap::new();

    // Start with the two blocks
    let mut current1 = block1.clone();
    let mut current2 = block2.clone();

    // Add the starting blocks to the visited maps
    visited1.insert(current1.hash, current1.clone());
    visited2.insert(current2.hash, current2.clone());

    // Traverse backwards from both blocks until we find a common ancestor
    loop {
        // Check if we've already seen the other block's current node
        if visited2.contains_key(&current1.hash) {
            return Ok(current1.clone());
        }
        if visited1.contains_key(&current2.hash) {
            return Ok(current2.clone());
        }

        // Move to the parent of block1 if possible
        if current1.height > 0 {
            match block_store.get_block_by_hash(&current1.prev_hash) {
                Ok(Some(parent)) => {
                    current1 = parent;
                    visited1.insert(current1.hash, current1.clone());

                    // Check if this is a common ancestor
                    if visited2.contains_key(&current1.hash) {
                        return Ok(current1.clone());
                    }
                },
                Ok(None) => {
                    return Err(format!("Cannot find parent of block at height {}", current1.height));
                },
                Err(e) => {
                    return Err(format!("Error getting parent block: {}", e));
                }
            }
        }

        // Move to the parent of block2 if possible
        if current2.height > 0 {
            match block_store.get_block_by_hash(&current2.prev_hash) {
                Ok(Some(parent)) => {
                    current2 = parent;
                    visited2.insert(current2.hash, current2.clone());

                    // Check if this is a common ancestor
                    if visited1.contains_key(&current2.hash) {
                        return Ok(current2.clone());
                    }
                },
                Ok(None) => {
                    return Err(format!("Cannot find parent of block at height {}", current2.height));
                },
                Err(e) => {
                    return Err(format!("Error getting parent block: {}", e));
                }
            }
        }

        // If we've reached the genesis block for both chains and still haven't found
        // a common ancestor, there isn't one (which shouldn't happen in a valid blockchain)
        if current1.height == 0 && current2.height == 0 {
            return Err("No common ancestor found".to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use crate::consensus::types::Target;
    use tempfile::tempdir;
    use std::sync::Arc;

    #[test]
    fn test_fork_choice() {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let block_store = Arc::new(BlockStore::new(&kv_store));

        // Create a chain state
        let chain_state = ChainState::new(
            10, // height
            [10u8; 32], // tip_hash
            [0u8; 32], // state_root
            1000, // total_difficulty
            0, // finalized_height
            [0u8; 32], // finalized_hash
        );

        // Create a block that builds on the current chain
        let block1 = Block {
            height: 11,
            hash: [11u8; 32],
            prev_hash: [10u8; 32], // Points to the current tip
            timestamp: 110,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 1100,
            poh_hash: [0u8; 32],
            difficulty: 100,
            total_difficulty: 1100, // 1000 (parent) + 100 (this block)
        };

        // Create a block that forms a fork
        let block2 = Block {
            height: 11,
            hash: [99u8; 32],
            prev_hash: [9u8; 32], // Points to a different block
            timestamp: 110,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 1100,
            poh_hash: [0u8; 32],
            difficulty: 100,
            total_difficulty: 1200, // 1100 (parent) + 100 (this block)
        };

        // Create the parent blocks
        let parent1 = Block {
            height: 10,
            hash: [10u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 100,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 1000,
            poh_hash: [0u8; 32],
            difficulty: 100,
            total_difficulty: 1000,
        };

        let parent2 = Block {
            height: 10,
            hash: [9u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 100,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 1000,
            poh_hash: [0u8; 32],
            difficulty: 100,
            total_difficulty: 1100,
        };

        // Store the blocks
        block_store.put_block(&parent1).unwrap();
        block_store.put_block(&parent2).unwrap();
        block_store.put_block(&block1).unwrap();
        block_store.put_block(&block2).unwrap();

        // Test the fork choice rule
        let choice1 = choose_fork(&block_store, &chain_state, &block1);
        let choice2 = choose_fork(&block_store, &chain_state, &block2);

        // Block 1 should be accepted
        assert_eq!(choice1, ForkChoice::Accept);

        // Block 2 should be considered a fork (and would be accepted due to more work)
        assert_eq!(choice2, ForkChoice::Fork);

        // Test resolving the fork
        let resolved = resolve_fork(&block_store, &block1, &block2).unwrap();

        // Block 2 should be chosen as it has more work
        assert_eq!(resolved.hash, block2.hash);
    }

    #[test]
    fn test_find_common_ancestor() {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let block_store = Arc::new(BlockStore::new(&kv_store));

        // Create a chain of blocks
        // Genesis block
        let genesis = Block {
            height: 0,
            hash: [0u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 0,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 0,
            poh_hash: [0u8; 32],
            difficulty: 100,
            total_difficulty: 100,
        };

        // Block 1
        let block1 = Block {
            height: 1,
            hash: [1u8; 32],
            prev_hash: [0u8; 32], // Points to genesis
            timestamp: 10,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 100,
            poh_hash: [0u8; 32],
            difficulty: 100,
            total_difficulty: 200,
        };

        // Block 2 (main chain)
        let block2 = Block {
            height: 2,
            hash: [2u8; 32],
            prev_hash: [1u8; 32], // Points to block1
            timestamp: 20,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 200,
            poh_hash: [0u8; 32],
            difficulty: 100,
            total_difficulty: 300,
        };

        // Block 3 (main chain)
        let block3 = Block {
            height: 3,
            hash: [3u8; 32],
            prev_hash: [2u8; 32], // Points to block2
            timestamp: 30,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 300,
            poh_hash: [0u8; 32],
            difficulty: 100,
            total_difficulty: 400,
        };

        // Block 2' (fork)
        let block2_fork = Block {
            height: 2,
            hash: [22u8; 32],
            prev_hash: [1u8; 32], // Points to block1
            timestamp: 20,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 200,
            poh_hash: [0u8; 32],
            difficulty: 100,
            total_difficulty: 300,
        };

        // Block 3' (fork)
        let block3_fork = Block {
            height: 3,
            hash: [33u8; 32],
            prev_hash: [22u8; 32], // Points to block2_fork
            timestamp: 30,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 300,
            poh_hash: [0u8; 32],
            difficulty: 100,
            total_difficulty: 400,
        };

        // Store the blocks
        block_store.put_block(&genesis).unwrap();
        block_store.put_block(&block1).unwrap();
        block_store.put_block(&block2).unwrap();
        block_store.put_block(&block3).unwrap();
        block_store.put_block(&block2_fork).unwrap();
        block_store.put_block(&block3_fork).unwrap();

        // Find common ancestor of block3 and block3_fork
        let ancestor = find_common_ancestor(&block_store, &block3, &block3_fork).unwrap();

        // The common ancestor should be block1
        assert_eq!(ancestor.hash, block1.hash);

        // Find common ancestor of block2 and block2_fork
        let ancestor = find_common_ancestor(&block_store, &block2, &block2_fork).unwrap();

        // The common ancestor should be block1
        assert_eq!(ancestor.hash, block1.hash);

        // Find common ancestor of block3 and block2
        let ancestor = find_common_ancestor(&block_store, &block3, &block2).unwrap();

        // The common ancestor should be block2 (as it's an ancestor of block3)
        assert_eq!(ancestor.hash, block2.hash);
    }
}
