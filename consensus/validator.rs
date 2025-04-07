use std::sync::Arc;
use log::{debug, info, warn, error};
use sha2::{Sha256, Digest};

use crate::storage::block_store::{Block, BlockStore, Hash};
use crate::storage::state_store::StateStore;
use crate::storage::tx_store::TxStore;
use crate::consensus::types::{BlockHeader, ConsensusError, Target};
use crate::consensus::poh::PoHGenerator;

/// Block validation context
#[derive(Debug, Clone)]
pub struct ValidationContext {
    /// Previous block
    pub prev_block: Option<Block>,
    
    /// Current difficulty target
    pub target: Target,
    
    /// Latest PoH hash
    pub latest_poh_hash: Hash,
    
    /// Latest PoH sequence number
    pub latest_poh_seq: u64,
    
    /// Maximum future time (in seconds)
    pub max_future_time: u64,
}

/// Block validation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockValidationResult {
    /// Block is valid
    Valid,
    
    /// Block is already known
    AlreadyKnown,
    
    /// Block has an unknown parent
    UnknownParent,
    
    /// Block is invalid
    Invalid(String),
}

/// Block validator
pub struct BlockValidator<'a> {
    /// Block store
    block_store: &'a BlockStore<'a>,
    
    /// State store
    state_store: &'a StateStore<'a>,
    
    /// Transaction store
    tx_store: &'a TxStore<'a>,
    
    /// Maximum future time (in seconds)
    max_future_time: u64,
}

impl<'a> BlockValidator<'a> {
    /// Create a new block validator
    pub fn new(
        block_store: &'a BlockStore<'a>,
        state_store: &'a StateStore<'a>,
        tx_store: &'a TxStore<'a>,
        max_future_time: u64,
    ) -> Self {
        Self {
            block_store,
            state_store,
            tx_store,
            max_future_time,
        }
    }
    
    /// Validate a block
    pub fn validate_block(&self, block: &Block, context: &ValidationContext) -> BlockValidationResult {
        // Check if the block is already known
        if self.block_store.has_block_by_hash(&block.hash) {
            return BlockValidationResult::AlreadyKnown;
        }
        
        // Check if we have the parent block
        if block.height > 0 {
            if !self.block_store.has_block_by_hash(&block.prev_hash) && context.prev_block.is_none() {
                return BlockValidationResult::UnknownParent;
            }
        }
        
        // Get the previous block
        let prev_block = if let Some(prev) = &context.prev_block {
            prev
        } else if block.height > 0 {
            match self.block_store.get_block_by_hash(&block.prev_hash) {
                Some(prev) => prev,
                None => return BlockValidationResult::UnknownParent,
            }
        } else {
            // Genesis block has no parent
            return BlockValidationResult::Valid;
        };
        
        // Validate the block header
        match self.validate_header(&block.header, prev_block, context) {
            Ok(_) => {},
            Err(e) => return BlockValidationResult::Invalid(e.to_string()),
        }
        
        // Validate the transactions
        match self.validate_transactions(block) {
            Ok(_) => {},
            Err(e) => return BlockValidationResult::Invalid(e.to_string()),
        }
        
        // Validate the state root
        match self.validate_state_root(block) {
            Ok(_) => {},
            Err(e) => return BlockValidationResult::Invalid(e.to_string()),
        }
        
        BlockValidationResult::Valid
    }
    
    /// Validate the block header
    fn validate_header(&self, header: &BlockHeader, prev_block: &Block, context: &ValidationContext) -> Result<(), ConsensusError> {
        // Check the height
        if header.height != prev_block.height + 1 {
            return Err(ConsensusError::InvalidHeight {
                expected: prev_block.height + 1,
                actual: header.height,
            });
        }
        
        // Check the previous hash
        if header.prev_hash != prev_block.hash {
            return Err(ConsensusError::InvalidPrevHash {
                expected: hex::encode(&prev_block.hash),
                actual: hex::encode(&header.prev_hash),
            });
        }
        
        // Check the timestamp
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if header.timestamp > current_time + self.max_future_time {
            return Err(ConsensusError::InvalidTimestamp(
                format!("Block timestamp is too far in the future: {} > {}", 
                        header.timestamp, current_time + self.max_future_time)
            ));
        }
        
        if header.timestamp <= prev_block.timestamp {
            return Err(ConsensusError::InvalidTimestamp(
                format!("Block timestamp is not greater than previous block: {} <= {}", 
                        header.timestamp, prev_block.timestamp)
            ));
        }
        
        // Check the PoH sequence
        if header.poh_seq <= prev_block.poh_seq {
            return Err(ConsensusError::InvalidPoH(
                format!("PoH sequence is not greater than previous block: {} <= {}", 
                        header.poh_seq, prev_block.poh_seq)
            ));
        }
        
        // Verify the PoH hash
        let poh_steps = header.poh_seq - prev_block.poh_seq;
        let expected_poh_hash = PoHGenerator::generate_hash_sequence(&prev_block.poh_hash, poh_steps);
        
        if header.poh_hash != expected_poh_hash {
            return Err(ConsensusError::InvalidPoH(
                format!("PoH hash does not match expected value: {} != {}", 
                        hex::encode(&header.poh_hash), hex::encode(&expected_poh_hash))
            ));
        }
        
        // Check the PoW
        let header_hash = header.hash();
        if !context.target.is_met_by(&header_hash) {
            return Err(ConsensusError::InvalidPoW);
        }
        
        // Check the total difficulty
        let expected_total_difficulty = prev_block.total_difficulty + header.difficulty as u128;
        if header.total_difficulty != expected_total_difficulty {
            return Err(ConsensusError::Other(
                format!("Total difficulty does not match expected value: {} != {}", 
                        header.total_difficulty, expected_total_difficulty)
            ));
        }
        
        Ok(())
    }
    
    /// Validate the transactions in a block
    fn validate_transactions(&self, block: &Block) -> Result<(), ConsensusError> {
        // Check that the transaction count matches
        if block.transactions.len() != block.header.tx_count as usize {
            return Err(ConsensusError::Other(
                format!("Transaction count mismatch: {} != {}", 
                        block.transactions.len(), block.header.tx_count)
            ));
        }
        
        // Verify the transaction root
        let tx_root = self.calculate_tx_root(&block.transactions);
        if tx_root != block.header.tx_root {
            return Err(ConsensusError::InvalidTxRoot {
                expected: hex::encode(&block.header.tx_root),
                actual: hex::encode(&tx_root),
            });
        }
        
        // Validate each transaction
        for tx_hash in &block.transactions {
            // Get the transaction
            let tx = match self.tx_store.get_transaction(tx_hash) {
                Some(tx) => tx,
                None => return Err(ConsensusError::Other(
                    format!("Transaction not found: {}", hex::encode(tx_hash))
                )),
            };
            
            // Validate the transaction (simplified)
            // In a real implementation, we would check signatures, balances, etc.
            if tx.block_height != block.header.height {
                return Err(ConsensusError::Other(
                    format!("Transaction block height mismatch: {} != {}", 
                            tx.block_height, block.header.height)
                ));
            }
        }
        
        Ok(())
    }
    
    /// Validate the state root
    fn validate_state_root(&self, block: &Block) -> Result<(), ConsensusError> {
        // Calculate the state root
        let state_root = match self.state_store.get_state_root() {
            Some(root) => root.root_hash,
            None => return Err(ConsensusError::Other("State root not found".to_string())),
        };
        
        // Check that it matches the block's state root
        if state_root != block.header.state_root {
            return Err(ConsensusError::InvalidStateRoot {
                expected: hex::encode(&block.header.state_root),
                actual: hex::encode(&state_root),
            });
        }
        
        Ok(())
    }
    
    /// Calculate the transaction root
    fn calculate_tx_root(&self, tx_hashes: &[Hash]) -> Hash {
        // Simple Merkle root calculation
        if tx_hashes.is_empty() {
            return [0u8; 32];
        }
        
        let mut hashes = tx_hashes.to_vec();
        
        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            
            for chunk in hashes.chunks(2) {
                let mut hasher = Sha256::new();
                
                hasher.update(&chunk[0]);
                
                if chunk.len() > 1 {
                    hasher.update(&chunk[1]);
                } else {
                    // If odd number of hashes, duplicate the last one
                    hasher.update(&chunk[0]);
                }
                
                let result = hasher.finalize();
                
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&result);
                
                next_level.push(hash);
            }
            
            hashes = next_level;
        }
        
        hashes[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use crate::consensus::types::Target;
    use tempfile::tempdir;
    
    #[test]
    fn test_tx_root_calculation() {
        // Create a validator
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = BlockStore::new(&kv_store);
        let state_store = StateStore::new(&kv_store);
        let tx_store = TxStore::new(&kv_store);
        
        let validator = BlockValidator::new(&block_store, &state_store, &tx_store, 7200);
        
        // Test with empty transactions
        let empty_root = validator.calculate_tx_root(&[]);
        assert_eq!(empty_root, [0u8; 32]);
        
        // Test with one transaction
        let tx1 = [1u8; 32];
        let root1 = validator.calculate_tx_root(&[tx1]);
        
        // Test with two transactions
        let tx2 = [2u8; 32];
        let root2 = validator.calculate_tx_root(&[tx1, tx2]);
        
        // The roots should be different
        assert_ne!(root1, empty_root);
        assert_ne!(root2, root1);
        
        // Test with three transactions
        let tx3 = [3u8; 32];
        let root3 = validator.calculate_tx_root(&[tx1, tx2, tx3]);
        
        // The roots should be different
        assert_ne!(root3, root2);
    }
}
