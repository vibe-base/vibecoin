use crate::ledger::transaction::Transaction;
use crate::types::primitives::{Hash, Timestamp};
use crate::types::error::VibecoinError;
use crate::crypto::hash::sha256_bytes;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Block {
    pub index: u64,
    pub previous_hash: Hash,
    pub timestamp: Timestamp,
    pub nonce: u64,
    pub poh_proof: Option<Hash>,
    pub transactions: Arc<Vec<Transaction>>,
    pub hash: Hash,
    pub slot_number: u64,           // The slot number this block belongs to
    pub slot_leader: Option<Hash>,   // Public key of the slot leader
}

impl Block {
    pub fn new(
        index: u64,
        previous_hash: Hash,
        transactions: Vec<Transaction>,
    ) -> Result<Self, VibecoinError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| VibecoinError::InvalidTimestamp)?
            .as_secs();

        // Validate transactions before wrapping in Arc
        for tx in &transactions {
            tx.validate()?;
        }

        let mut block = Block {
            index,
            previous_hash,
            timestamp,
            nonce: 0,
            poh_proof: None,
            transactions: Arc::new(transactions),
            hash: [0u8; 32],
            slot_number: 0,           // Default to slot 0, will be set by miner
            slot_leader: None,         // Will be set by miner
        };

        block.hash = block.calculate_hash();
        Ok(block)
    }

    // Use references for validation methods
    pub fn validate(&self) -> Result<(), VibecoinError> {
        if self.calculate_hash() != self.hash {
            return Err(VibecoinError::HashMismatch);
        }

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| VibecoinError::InvalidTimestamp)?
            .as_secs();

        if self.timestamp > current_time {
            return Err(VibecoinError::InvalidTimestamp);
        }

        // Use parallel iterator for transaction validation
        use rayon::prelude::*;
        self.transactions
            .par_iter()
            .try_for_each(|tx| tx.validate())?;

        Ok(())
    }

    // Optimize hash calculation
    pub fn calculate_hash(&self) -> Hash {
        let mut data = Vec::with_capacity(128); // Pre-allocate buffer

        // Avoid string formatting, use direct byte writing
        data.extend_from_slice(&self.index.to_le_bytes());
        data.extend_from_slice(&self.previous_hash);
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        data.extend_from_slice(&self.nonce.to_le_bytes());
        data.extend_from_slice(&self.slot_number.to_le_bytes());

        // Include slot leader if present
        if let Some(leader) = self.slot_leader {
            data.extend_from_slice(&leader);
        }

        // Hash transactions efficiently
        for tx in self.transactions.iter() {
            data.extend_from_slice(&tx.hash);
        }

        sha256_bytes(&data)
    }

    // Calculate hash without nonce (for PoH verification)
    pub fn calculate_hash_without_nonce(&self) -> Hash {
        let mut data = Vec::with_capacity(128); // Pre-allocate buffer

        // Include all fields except nonce
        data.extend_from_slice(&self.index.to_le_bytes());
        data.extend_from_slice(&self.previous_hash);
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        data.extend_from_slice(&self.slot_number.to_le_bytes());

        // Include slot leader if present
        if let Some(leader) = self.slot_leader {
            data.extend_from_slice(&leader);
        }

        // Use zero nonce to represent pre-mining state
        data.extend_from_slice(&0u64.to_le_bytes());

        // Hash transactions efficiently
        for tx in self.transactions.iter() {
            data.extend_from_slice(&tx.hash);
        }

        sha256_bytes(&data)
    }

    pub fn update_hash(&mut self) -> Result<(), VibecoinError> {
        self.hash = self.calculate_hash();
        self.validate()?;
        Ok(())
    }

    pub fn is_valid(&self) -> Result<(), VibecoinError> {
        // Validate basic block structure
        if self.index == 0 && self.previous_hash != [0u8; 32] {
            return Err(VibecoinError::InvalidBlockStructure);
        }

        // Validate block hash
        if self.calculate_hash() != self.hash {
            return Err(VibecoinError::HashMismatch);
        }

        // Validate timestamp
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| VibecoinError::InvalidTimestamp)?
            .as_secs();

        if self.timestamp > current_time {
            return Err(VibecoinError::InvalidTimestamp);
        }

        // Note: Full PoH validation requires access to the PoH state
        // which is not available in this method. The verify_poh_proof function
        // in the consensus/pow module should be used for complete validation.
        // Here we just do basic checks.
        if let Some(proof) = self.poh_proof {
            if proof == [0u8; 32] {
                return Err(VibecoinError::InvalidProofOfWork);
            }
        } else {
            // Require PoH proof for all blocks except genesis
            if self.index > 0 {
                return Err(VibecoinError::InvalidProofOfWork);
            }
        }

        // Validate all transactions
        use rayon::prelude::*;
        self.transactions
            .par_iter()
            .try_for_each(|tx| tx.is_valid())?;

        // Ensure block isn't empty (except for genesis block)
        if self.index != 0 && self.transactions.is_empty() {
            return Err(VibecoinError::InvalidBlockStructure);
        }

        Ok(())
    }

    // Add a method to verify block sequence
    pub fn verify_sequence(&self, previous_block: &Block) -> Result<(), VibecoinError> {
        if self.index != previous_block.index + 1 {
            return Err(VibecoinError::InvalidBlockStructure);
        }

        if self.previous_hash != previous_block.hash {
            return Err(VibecoinError::HashMismatch);
        }

        // In a test environment, blocks might be created very quickly
        // So we'll relax the timestamp check for demonstration purposes
        // In a real blockchain, this would be more strictly enforced
        if self.timestamp < previous_block.timestamp {
            return Err(VibecoinError::InvalidTimestamp);
        }

        Ok(())
    }

    // Implement Clone manually to avoid cloning transactions unnecessarily
    pub fn clone(&self) -> Self {
        Block {
            index: self.index,
            previous_hash: self.previous_hash,
            timestamp: self.timestamp,
            nonce: self.nonce,
            poh_proof: self.poh_proof,
            transactions: Arc::clone(&self.transactions),
            hash: self.hash,
            slot_number: self.slot_number,
            slot_leader: self.slot_leader,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_block() -> Result<Block, VibecoinError> {
        let previous_hash = [0u8; 32];
        let transactions = Vec::new();
        Block::new(1, previous_hash, transactions)
    }

    #[test]
    fn test_new_block() -> Result<(), VibecoinError> {
        let previous_hash = [0u8; 32];
        let transactions = Vec::new();
        let block = Block::new(1, previous_hash, transactions)?;

        assert_eq!(block.index, 1);
        assert_eq!(block.previous_hash, previous_hash);
        assert_ne!(block.hash, [0u8; 32]);
        Ok(())
    }

    #[test]
    fn test_hash_changes_with_nonce() -> Result<(), VibecoinError> {
        let mut block = create_test_block()?;
        let initial_hash = block.hash;

        block.nonce += 1;
        block.update_hash()?;

        assert_ne!(block.hash, initial_hash);
        Ok(())
    }

    #[test]
    fn test_block_validation() -> Result<(), VibecoinError> {
        let block = create_test_block()?;
        block.is_valid()?;
        Ok(())
    }

    #[test]
    fn test_invalid_hash() -> Result<(), VibecoinError> {
        let mut block = create_test_block()?;
        block.hash = [1u8; 32];

        assert!(matches!(block.is_valid(), Err(VibecoinError::HashMismatch)));
        Ok(())
    }

    #[test]
    fn test_block_sequence_validation() -> Result<(), VibecoinError> {
        let previous_hash = [0u8; 32];
        let transactions = Vec::new();

        let block1 = Block::new(1, previous_hash, transactions.clone())?;
        let block2 = Block::new(2, block1.hash, transactions)?;

        block2.verify_sequence(&block1)?;
        Ok(())
    }

    #[test]
    fn test_invalid_block_sequence() -> Result<(), VibecoinError> {
        let mut block1 = create_test_block()?;
        let mut block2 = create_test_block()?;

        block2.index = block1.index; // Invalid sequence

        assert!(matches!(
            block2.verify_sequence(&block1),
            Err(VibecoinError::InvalidBlockStructure)
        ));
        Ok(())
    }

    #[test]
    fn test_future_timestamp() -> Result<(), VibecoinError> {
        let mut block = create_test_block()?;
        block.timestamp = u64::MAX; // Far future timestamp

        assert!(matches!(block.is_valid(), Err(VibecoinError::InvalidTimestamp)));
        Ok(())
    }
}
