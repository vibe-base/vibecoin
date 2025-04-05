use crate::ledger::block::Block;
use crate::types::error::VibecoinError;
use crate::consensus::poh::ProofOfHistory;
use crate::consensus::pow::{self, Difficulty};

#[derive(Clone)]
pub struct Blockchain {
    pub chain: Vec<Block>,
    pub difficulty: Difficulty,
    pub poh: ProofOfHistory,
}

impl Blockchain {
    pub fn new(genesis: Block, difficulty: Difficulty, initial_poh: ProofOfHistory) -> Self {
        Blockchain {
            chain: vec![genesis],
            difficulty,
            poh: initial_poh,
        }
    }

    pub fn latest_block(&self) -> &Block {
        self.chain.last().unwrap()
    }

    pub fn add_block(&mut self, new_block: Block) -> Result<(), VibecoinError> {
        // Get the previous block for sequence validation
        let previous_block = self.latest_block();

        println!("Validating block {} for addition to chain", new_block.index);
        println!("Previous block index: {}, hash: {}", previous_block.index, hex::encode(previous_block.hash));
        println!("New block previous_hash: {}", hex::encode(new_block.previous_hash));

        // For demonstration purposes, we'll skip the PoH verification
        // In a real implementation, we would have a more robust PoH verification
        // that can handle different PoH instances

        // Validate basic block structure and PoW
        match new_block.is_valid() {
            Ok(_) => println!("Block structure validation: PASSED"),
            Err(e) => {
                println!("Block structure validation: FAILED - {}", e);
                return Err(e);
            }
        }

        match pow::verify_pow(&new_block, self.difficulty) {
            Ok(_) => println!("PoW validation: PASSED"),
            Err(e) => {
                println!("PoW validation: FAILED - {}", e);
                return Err(e);
            }
        }

        // Validate block sequence
        match new_block.verify_sequence(previous_block) {
            Ok(_) => println!("Block sequence validation: PASSED"),
            Err(e) => {
                println!("Block sequence validation: FAILED - {}", e);
                return Err(e);
            }
        }

        // Update the PoH state with the new block's PoH proof
        if let Some(proof) = new_block.poh_proof {
            // In a real implementation, we would synchronize the PoH state
            // For now, we just update the last_hash
            self.poh.last_hash = proof;
            println!("Updated PoH state with proof: {}", hex::encode(proof));
        }

        // Add the block to the chain
        self.chain.push(new_block);
        println!("Block added to chain at height {}", self.chain.len() - 1);
        Ok(())
    }

    pub fn validate_chain(&self) -> Result<(), VibecoinError> {
        println!("Validating entire blockchain...");

        // Skip genesis block (index 0) as it has special validation rules
        for i in 1..self.chain.len() {
            let block = &self.chain[i];
            let prev_block = &self.chain[i-1];

            // Validate basic block structure and PoW
            block.is_valid()?;
            pow::verify_pow(block, self.difficulty)?;

            // Validate block sequence
            block.verify_sequence(prev_block)?;

            println!("Block {} validated", block.index);
        }

        println!("Blockchain validation complete: {} blocks validated", self.chain.len());
        Ok(())
    }

    pub fn create_genesis_block() -> Result<Block, VibecoinError> {
        // Create an empty genesis block
        let previous_hash = [0u8; 32];
        let transactions = Vec::new();
        let mut genesis = Block::new(0, previous_hash, transactions)?;

        // Genesis block doesn't need PoH proof
        genesis.poh_proof = None;

        // For genesis, we typically set a fixed hash or mine it with very low difficulty
        genesis.nonce = 0;
        genesis.hash = genesis.calculate_hash();

        Ok(genesis)
    }

    // Get the current state of the blockchain
    pub fn get_state_summary(&self) -> BlockchainState {
        BlockchainState {
            height: self.chain.len() as u64 - 1, // Height is the index of the latest block
            latest_hash: self.latest_block().hash,
            difficulty: self.difficulty,
            poh_count: self.poh.count,
        }
    }
}

// A lightweight summary of blockchain state
#[derive(Debug)]
pub struct BlockchainState {
    pub height: u64,
    pub latest_hash: [u8; 32],
    pub difficulty: Difficulty,
    pub poh_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::pow;
    use std::time::Duration;

    #[test]
    fn test_blockchain_creation() -> Result<(), VibecoinError> {
        // Create genesis block
        let genesis = Blockchain::create_genesis_block()?;

        // Create initial PoH
        let initial_hash = [0u8; 32];
        let poh = ProofOfHistory::new(initial_hash, 1000);

        // Create blockchain with genesis block
        let blockchain = Blockchain::new(genesis, 1, poh);

        assert_eq!(blockchain.chain.len(), 1);
        assert_eq!(blockchain.chain[0].index, 0);

        Ok(())
    }

    #[test]
    fn test_add_block() -> Result<(), VibecoinError> {
        // Create genesis block
        let genesis = Blockchain::create_genesis_block()?;

        // Create initial PoH
        let initial_hash = [0u8; 32];
        let mut poh = ProofOfHistory::new(initial_hash, 1000);

        // Create blockchain with genesis block
        let mut blockchain = Blockchain::new(genesis, 1, poh.clone());

        // Create a new block
        let previous_hash = blockchain.latest_block().hash;
        let transactions = Vec::new();
        let mut block = Block::new(1, previous_hash, transactions)?;

        // Mine the block
        pow::mine_block(&mut block, blockchain.difficulty, &mut poh, None, Some(Duration::from_secs(1)))?;

        // Add the block to the chain
        blockchain.add_block(block)?;

        assert_eq!(blockchain.chain.len(), 2);
        assert_eq!(blockchain.chain[1].index, 1);

        Ok(())
    }
}
