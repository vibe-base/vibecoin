use std::collections::HashMap;
use log::{debug, info, warn};

use crate::storage::block_store::{Block, BlockStore, Hash};

/// Chain info
#[derive(Debug, Clone)]
pub struct ChainInfo {
    /// Chain ID
    pub id: String,
    
    /// Latest block
    pub latest_block: Block,
    
    /// Total difficulty
    pub total_difficulty: u128,
    
    /// Chain length
    pub length: u64,
}

/// Fork resolver
pub struct ForkResolver<'a> {
    /// Block store
    block_store: &'a BlockStore<'a>,
    
    /// Known chains
    chains: HashMap<String, ChainInfo>,
    
    /// Current best chain
    best_chain: Option<String>,
}

impl<'a> ForkResolver<'a> {
    /// Create a new fork resolver
    pub fn new(block_store: &'a BlockStore<'a>) -> Self {
        Self {
            block_store,
            chains: HashMap::new(),
            best_chain: None,
        }
    }
    
    /// Add a chain
    pub fn add_chain(&mut self, chain_id: String, latest_block: Block) {
        let total_difficulty = latest_block.total_difficulty;
        let length = latest_block.height + 1;
        
        let chain_info = ChainInfo {
            id: chain_id.clone(),
            latest_block,
            total_difficulty,
            length,
        };
        
        self.chains.insert(chain_id.clone(), chain_info);
        
        // Update the best chain
        self.update_best_chain();
    }
    
    /// Update a chain
    pub fn update_chain(&mut self, chain_id: &str, latest_block: Block) {
        if let Some(chain) = self.chains.get_mut(chain_id) {
            chain.latest_block = latest_block;
            chain.total_difficulty = latest_block.total_difficulty;
            chain.length = latest_block.height + 1;
            
            // Update the best chain
            self.update_best_chain();
        }
    }
    
    /// Remove a chain
    pub fn remove_chain(&mut self, chain_id: &str) {
        self.chains.remove(chain_id);
        
        // Update the best chain
        self.update_best_chain();
    }
    
    /// Get the best chain
    pub fn get_best_chain(&self) -> Option<&ChainInfo> {
        self.best_chain.as_ref().and_then(|id| self.chains.get(id))
    }
    
    /// Update the best chain
    fn update_best_chain(&mut self) {
        let mut best_chain = None;
        let mut best_difficulty = 0u128;
        
        for (id, chain) in &self.chains {
            if chain.total_difficulty > best_difficulty {
                best_difficulty = chain.total_difficulty;
                best_chain = Some(id.clone());
            }
        }
        
        self.best_chain = best_chain;
    }
    
    /// Resolve a fork
    pub fn resolve_fork(&self, chain1_id: &str, chain2_id: &str) -> Option<String> {
        let chain1 = self.chains.get(chain1_id)?;
        let chain2 = self.chains.get(chain2_id)?;
        
        // Choose the chain with the highest total difficulty
        if chain1.total_difficulty > chain2.total_difficulty {
            Some(chain1_id.to_string())
        } else if chain2.total_difficulty > chain1.total_difficulty {
            Some(chain2_id.to_string())
        } else {
            // If equal, choose the longer chain
            if chain1.length > chain2.length {
                Some(chain1_id.to_string())
            } else if chain2.length > chain1.length {
                Some(chain2_id.to_string())
            } else {
                // If still equal, choose the chain with the lower hash (arbitrary but deterministic)
                if chain1.latest_block.hash < chain2.latest_block.hash {
                    Some(chain1_id.to_string())
                } else {
                    Some(chain2_id.to_string())
                }
            }
        }
    }
    
    /// Find the common ancestor of two chains
    pub fn find_common_ancestor(&self, chain1_id: &str, chain2_id: &str) -> Option<Block> {
        let chain1 = self.chains.get(chain1_id)?;
        let chain2 = self.chains.get(chain2_id)?;
        
        // Start from the latest blocks
        let mut block1 = chain1.latest_block.clone();
        let mut block2 = chain2.latest_block.clone();
        
        // If one chain is longer, move back to the same height
        while block1.height > block2.height {
            block1 = self.block_store.get_block_by_hash(&block1.prev_hash)?;
        }
        
        while block2.height > block1.height {
            block2 = self.block_store.get_block_by_hash(&block2.prev_hash)?;
        }
        
        // Now both blocks are at the same height
        // Move back until we find a common block
        while block1.hash != block2.hash {
            block1 = self.block_store.get_block_by_hash(&block1.prev_hash)?;
            block2 = self.block_store.get_block_by_hash(&block2.prev_hash)?;
        }
        
        Some(block1)
    }
    
    /// Get the blocks to apply and revert when switching from one chain to another
    pub fn get_chain_switch_blocks(
        &self,
        from_chain_id: &str,
        to_chain_id: &str,
    ) -> Option<(Vec<Block>, Vec<Block>)> {
        // Find the common ancestor
        let ancestor = self.find_common_ancestor(from_chain_id, to_chain_id)?;
        
        // Get the blocks to revert (from the current chain back to the ancestor)
        let mut revert_blocks = Vec::new();
        let mut current = self.chains.get(from_chain_id)?.latest_block.clone();
        
        while current.hash != ancestor.hash {
            revert_blocks.push(current.clone());
            current = self.block_store.get_block_by_hash(&current.prev_hash)?;
        }
        
        // Get the blocks to apply (from the ancestor to the new chain)
        let mut apply_blocks = Vec::new();
        let mut current = self.chains.get(to_chain_id)?.latest_block.clone();
        
        while current.hash != ancestor.hash {
            apply_blocks.push(current.clone());
            current = self.block_store.get_block_by_hash(&current.prev_hash)?;
        }
        
        // Reverse the apply blocks to get them in ascending order
        apply_blocks.reverse();
        
        Some((apply_blocks, revert_blocks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;
    
    #[test]
    fn test_fork_resolution() {
        // Create a block store
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = BlockStore::new(&kv_store);
        
        // Create a fork resolver
        let mut resolver = ForkResolver::new(&block_store);
        
        // Create two chains
        let chain1_block = Block {
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
        
        let chain2_block = Block {
            height: 9,
            hash: [2; 32],
            prev_hash: [0; 32],
            timestamp: 12345,
            transactions: vec![],
            state_root: [0; 32],
            poh_hash: [0; 32],
            poh_seq: 100,
            nonce: 0,
            difficulty: 1000,
            total_difficulty: 9000,
        };
        
        resolver.add_chain("chain1".to_string(), chain1_block);
        resolver.add_chain("chain2".to_string(), chain2_block);
        
        // Resolve the fork
        let best_chain = resolver.resolve_fork("chain1", "chain2").unwrap();
        assert_eq!(best_chain, "chain1");
        
        // Update chain2 to have higher difficulty
        let chain2_block = Block {
            height: 9,
            hash: [2; 32],
            prev_hash: [0; 32],
            timestamp: 12345,
            transactions: vec![],
            state_root: [0; 32],
            poh_hash: [0; 32],
            poh_seq: 100,
            nonce: 0,
            difficulty: 1000,
            total_difficulty: 11000,
        };
        
        resolver.update_chain("chain2", chain2_block);
        
        // Resolve the fork again
        let best_chain = resolver.resolve_fork("chain1", "chain2").unwrap();
        assert_eq!(best_chain, "chain2");
        
        // Check the best chain
        let best_chain = resolver.get_best_chain().unwrap();
        assert_eq!(best_chain.id, "chain2");
    }
}
