//! Chain state for consensus
//!
//! This module defines the chain state used by the consensus mechanism.

use std::sync::Arc;
use std::collections::HashMap;

use crate::storage::block_store::{Block, Hash};
use crate::storage::state::StateRoot;

/// Chain state for consensus
#[derive(Debug, Clone)]
pub struct ChainState {
    /// Current height
    pub height: u64,

    /// Current tip hash
    pub tip_hash: Hash,

    /// Current state root
    pub state_root: StateRoot,

    /// Total difficulty
    pub total_difficulty: u64,

    /// Block headers by hash
    pub blocks: HashMap<Hash, Block>,

    /// Block hashes by height
    pub block_hashes: HashMap<u64, Vec<Hash>>,

    /// Current target difficulty
    pub current_target: crate::consensus::types::Target,

    /// Latest block hash
    pub latest_hash: Hash,

    /// Latest block timestamp
    pub latest_timestamp: u64,

    /// Finalized height
    pub finalized_height: u64,

    /// Finalized hash
    pub finalized_hash: Hash,
}

impl ChainState {
    /// Create a new chain state
    pub fn new(
        height: u64,
        tip_hash: Hash,
        state_root: StateRoot,
        total_difficulty: u64,
        finalized_height: u64,
        finalized_hash: Hash,
    ) -> Self {
        Self {
            height,
            tip_hash,
            state_root,
            total_difficulty,
            blocks: HashMap::new(),
            block_hashes: HashMap::new(),
            finalized_height,
            finalized_hash,
            current_target: Default::default(),
            latest_hash: tip_hash,
            latest_timestamp: 0, // Initialize with 0, will be updated later
        }
    }

    /// Add a block to the chain state
    pub fn add_block(&mut self, block: Block) {
        let hash = block.hash;
        let height = block.height;

        // Add to blocks map
        self.blocks.insert(hash, block);

        // Add to block_hashes map
        self.block_hashes.entry(height).or_insert_with(Vec::new).push(hash);
    }

    /// Get a block by hash
    pub fn get_block(&self, hash: &Hash) -> Option<&Block> {
        self.blocks.get(hash)
    }

    /// Get blocks at a specific height
    pub fn get_blocks_at_height(&self, height: u64) -> Vec<&Block> {
        match self.block_hashes.get(&height) {
            Some(hashes) => {
                hashes.iter()
                    .filter_map(|hash| self.blocks.get(hash))
                    .collect()
            }
            None => Vec::new(),
        }
    }

    /// Update the tip
    pub fn update_tip(&mut self, hash: Hash, height: u64, state_root: StateRoot, total_difficulty: u64) {
        self.tip_hash = hash;
        self.height = height;
        self.state_root = state_root;
        self.total_difficulty = total_difficulty;
    }

    /// Update the finalized block
    pub fn update_finalized(&mut self, hash: Hash, height: u64) {
        self.finalized_hash = hash;
        self.finalized_height = height;
    }

    /// Check if a block is finalized
    pub fn is_finalized(&self, height: u64) -> bool {
        height <= self.finalized_height
    }

    /// Prune old blocks
    pub fn prune(&mut self, height: u64) {
        // Remove blocks below the specified height
        self.blocks.retain(|_, block| block.height >= height);

        // Remove block hashes below the specified height
        self.block_hashes.retain(|&h, _| h >= height);
    }
}
