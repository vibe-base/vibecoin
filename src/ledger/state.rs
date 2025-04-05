use crate::ledger::block::Block;
use crate::types::error::VibecoinError;
use crate::consensus::poh::ProofOfHistory;
use crate::consensus::pow::{self, Difficulty};
use crate::consensus::slot::SlotManager;
use crate::consensus::fork_resolver;
use crate::types::primitives::Hash;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Clone)]
pub struct Blockchain {
    pub chain: Vec<Block>,
    pub difficulty: Difficulty,
    pub poh: ProofOfHistory,
    pub slot_manager: Option<SlotManager>,
    pub miner_key: Hash,
}

impl Blockchain {
    pub fn new(genesis: Block, difficulty: Difficulty, initial_poh: ProofOfHistory, miner_key: Hash) -> Self {
        // Create a slot manager with the genesis hash
        let slot_manager = SlotManager::new(genesis.hash);

        Blockchain {
            chain: vec![genesis],
            difficulty,
            poh: initial_poh,
            slot_manager: Some(slot_manager),
            miner_key,
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
        println!("New block slot: {}", new_block.slot_number);

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

        // Validate PoH proof
        match pow::verify_poh_proof(&new_block, &self.poh) {
            Ok(_) => println!("PoH proof validation: PASSED"),
            Err(e) => {
                // Skip PoH validation for genesis block
                if new_block.index > 0 {
                    println!("PoH proof validation: FAILED - {}", e);
                    return Err(e);
                }
            }
        }

        // Validate slot information
        if let Some(slot_manager) = &self.slot_manager {
            // Check if the block's slot number is valid
            if new_block.slot_number < slot_manager.current_slot.slot_number {
                // Block is from a past slot, check if it's a valid slot
                if let Some(slot) = slot_manager.get_slot_by_number(new_block.slot_number) {
                    // Check if the block's slot leader matches
                    if new_block.slot_leader != slot.leader {
                        println!("Slot leader validation: FAILED - Invalid slot leader");
                        return Err(VibecoinError::InvalidBlock);
                    }
                } else {
                    println!("Slot validation: FAILED - Unknown slot");
                    return Err(VibecoinError::InvalidBlock);
                }
            }
            println!("Slot validation: PASSED");
        }

        // Update the PoH state with the new block's PoH proof
        if let Some(proof) = new_block.poh_proof {
            // In a real implementation, we would synchronize the PoH state
            // For now, we just update the last_hash
            self.poh.last_hash = proof;
            println!("Updated PoH state with proof: {}", hex::encode(proof));
        }

        // Add the block to the chain
        self.chain.push(new_block.clone());
        println!("Block added to chain at height {}", self.chain.len() - 1);

        // Update slot manager if needed
        if let Some(slot_manager) = &mut self.slot_manager {
            if new_block.slot_number == slot_manager.current_slot.slot_number {
                // Add the block to the current slot
                slot_manager.current_slot.add_block(new_block.hash);
            }
        }

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
        let current_slot = if let Some(slot_manager) = &self.slot_manager {
            slot_manager.current_slot.slot_number
        } else {
            0
        };

        BlockchainState {
            height: self.chain.len() as u64 - 1, // Height is the index of the latest block
            latest_hash: self.latest_block().hash,
            difficulty: self.difficulty,
            poh_count: self.poh.count,
            current_slot,
        }
    }

    // Resolve a fork with another blockchain
    pub fn resolve_fork(&self, other: &Blockchain) -> fork_resolver::ChainComparison {
        fork_resolver::resolve_fork(self, other)
    }

    // Mine a new block with PoW and PoH
    pub fn mine_new_block(
        &mut self,
        transactions: Vec<crate::ledger::transaction::Transaction>,
        stop_signal: Option<Arc<AtomicBool>>,
        max_time: Option<std::time::Duration>,
    ) -> Result<Block, VibecoinError> {
        // Get the current slot
        let slot_manager = if let Some(manager) = &mut self.slot_manager {
            manager
        } else {
            return Err(VibecoinError::InvalidState);
        };

        let current_slot = slot_manager.current_slot.clone();
        let previous_block = self.latest_block();

        // Create a new block
        let mut block = Block::new(
            previous_block.index + 1,
            previous_block.hash,
            transactions,
        )?;

        // Mine the block
        pow::mine_block(
            &mut block,
            self.difficulty,
            &mut self.poh,
            current_slot.slot_number,
            self.miner_key,
            stop_signal,
            max_time,
        )?;

        // Add the block to the chain
        self.add_block(block.clone())?;

        Ok(block)
    }

    // Advance to the next slot
    pub fn advance_slot(&mut self) -> Result<(), VibecoinError> {
        let slot_manager = if let Some(manager) = &mut self.slot_manager {
            manager
        } else {
            return Err(VibecoinError::InvalidState);
        };

        // Advance to the next slot
        let new_slot = slot_manager.advance_slot(&self.poh);
        println!("Advanced to slot {}", new_slot.slot_number);

        Ok(())
    }

    // Compete to become the leader for the next slot
    pub fn compete_for_leadership(
        &mut self,
        stop_signal: Option<Arc<AtomicBool>>,
        max_time: Option<std::time::Duration>,
    ) -> Result<bool, VibecoinError> {
        let slot_manager = if let Some(manager) = &mut self.slot_manager {
            manager
        } else {
            return Err(VibecoinError::InvalidState);
        };

        let current_slot = slot_manager.current_slot.clone();

        // Mine a PoW solution to become the leader
        match crate::consensus::slot::mine_slot_leader(
            &current_slot,
            self.difficulty,
            &self.miner_key,
            stop_signal,
            max_time,
        ) {
            Ok(pow_solution) => {
                // We found a solution, set ourselves as the leader
                if slot_manager.is_leader(&self.miner_key, &pow_solution, self.difficulty) {
                    slot_manager.current_slot.set_leader(self.miner_key);
                    println!("Became leader for slot {}", current_slot.slot_number);
                    return Ok(true);
                }
                Ok(false)
            },
            Err(e) => {
                println!("Failed to mine slot leader: {}", e);
                Ok(false)
            }
        }
    }

    // Generate PoH stream for the current slot
    pub fn generate_poh_stream(
        &mut self,
        stop_signal: Option<Arc<AtomicBool>>,
    ) -> Result<(), VibecoinError> {
        let slot_manager = if let Some(manager) = &mut self.slot_manager {
            manager
        } else {
            return Err(VibecoinError::InvalidState);
        };

        // Check if we're the leader
        if slot_manager.current_slot.leader != Some(self.miner_key) {
            return Err(VibecoinError::NotSlotLeader);
        }

        // Generate PoH stream
        slot_manager.generate_poh_stream(&mut self.poh, stop_signal)?;

        // Complete the slot
        slot_manager.current_slot.complete(self.poh.last_hash);

        Ok(())
    }
}

// A lightweight summary of blockchain state
#[derive(Debug)]
pub struct BlockchainState {
    pub height: u64,
    pub latest_hash: [u8; 32],
    pub difficulty: Difficulty,
    pub poh_count: u64,
    pub current_slot: u64,
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

        // Create miner key
        let miner_key = [1u8; 32];

        // Create blockchain with genesis block
        let blockchain = Blockchain::new(genesis, 1, poh, miner_key);

        assert_eq!(blockchain.chain.len(), 1);
        assert_eq!(blockchain.chain[0].index, 0);
        assert_eq!(blockchain.miner_key, miner_key);
        assert!(blockchain.slot_manager.is_some());

        Ok(())
    }

    #[test]
    fn test_add_block() -> Result<(), VibecoinError> {
        // Create genesis block
        let genesis = Blockchain::create_genesis_block()?;

        // Create initial PoH
        let initial_hash = [0u8; 32];
        let mut poh = ProofOfHistory::new(initial_hash, 1000);

        // Create miner key
        let miner_key = [1u8; 32];

        // Create blockchain with genesis block
        let mut blockchain = Blockchain::new(genesis, 1, poh.clone(), miner_key);

        // Create a new block
        let previous_hash = blockchain.latest_block().hash;
        let transactions = Vec::new();
        let mut block = Block::new(1, previous_hash, transactions)?;

        // Set slot information
        let slot_number = 1;

        // Mine the block
        pow::mine_block(
            &mut block,
            blockchain.difficulty,
            &mut poh,
            slot_number,
            miner_key,
            None,
            Some(Duration::from_secs(1))
        )?;

        // Add the block to the chain
        blockchain.add_block(block)?;

        assert_eq!(blockchain.chain.len(), 2);
        assert_eq!(blockchain.chain[1].index, 1);
        assert_eq!(blockchain.chain[1].slot_number, slot_number);
        assert_eq!(blockchain.chain[1].slot_leader, Some(miner_key));

        Ok(())
    }

    #[test]
    fn test_fork_resolution() -> Result<(), VibecoinError> {
        // Create genesis block
        let genesis = Blockchain::create_genesis_block()?;

        // Create initial PoH
        let initial_hash = [0u8; 32];
        let mut poh1 = ProofOfHistory::new(initial_hash, 1000);
        let mut poh2 = ProofOfHistory::new(initial_hash, 1000);

        // Create miner keys
        let miner_key1 = [1u8; 32];
        let miner_key2 = [2u8; 32];

        // Create two blockchains with the same genesis block
        let mut blockchain1 = Blockchain::new(genesis.clone(), 1, poh1.clone(), miner_key1);
        let mut blockchain2 = Blockchain::new(genesis.clone(), 1, poh2.clone(), miner_key2);

        // Add more blocks to chain 1
        for i in 1..4 {
            let previous_hash = blockchain1.latest_block().hash;
            let transactions = Vec::new();
            let mut block = Block::new(i, previous_hash, transactions)?;

            pow::mine_block(&mut block, blockchain1.difficulty, &mut poh1, i, miner_key1, None, Some(Duration::from_secs(1)))?;
            blockchain1.add_block(block)?;
        }

        // Add fewer blocks to chain 2
        for i in 1..3 {
            let previous_hash = blockchain2.latest_block().hash;
            let transactions = Vec::new();
            let mut block = Block::new(i, previous_hash, transactions)?;

            pow::mine_block(&mut block, blockchain2.difficulty, &mut poh2, i, miner_key2, None, Some(Duration::from_secs(1)))?;
            blockchain2.add_block(block)?;
        }

        // Resolve fork
        let result = blockchain1.resolve_fork(&blockchain2);
        assert_eq!(result, fork_resolver::ChainComparison::FirstChainBetter);

        // Check chain lengths
        assert_eq!(blockchain1.chain.len(), 4); // Genesis + 3 blocks
        assert_eq!(blockchain2.chain.len(), 3); // Genesis + 2 blocks

        Ok(())
    }

    #[test]
    fn test_slot_based_consensus() -> Result<(), VibecoinError> {
        // Create genesis block
        let genesis = Blockchain::create_genesis_block()?;

        // Create initial PoH
        let initial_hash = [0u8; 32];
        let mut poh = ProofOfHistory::new(initial_hash, 1000);

        // Create miner key
        let miner_key = [1u8; 32];

        // Create blockchain with genesis block
        let mut blockchain = Blockchain::new(genesis, 1, poh.clone(), miner_key);

        // Advance to slot 1
        blockchain.advance_slot()?;

        // Verify slot number
        let slot_manager = blockchain.slot_manager.as_ref().unwrap();
        assert_eq!(slot_manager.current_slot.slot_number, 1);

        // Compete for leadership (with very low difficulty, should succeed)
        let became_leader = blockchain.compete_for_leadership(None, Some(Duration::from_secs(1)))?;

        // If we became the leader, generate PoH stream and mine a block
        if became_leader {
            // Generate PoH stream
            blockchain.generate_poh_stream(None)?;

            // Mine a block
            let transactions = Vec::new();
            let block = blockchain.mine_new_block(transactions, None, Some(Duration::from_secs(1)))?;

            // Verify block slot information
            assert_eq!(block.slot_number, 1);
            assert_eq!(block.slot_leader, Some(miner_key));

            // Verify blockchain state
            assert_eq!(blockchain.chain.len(), 2); // Genesis + 1 block
            assert_eq!(blockchain.latest_block().slot_number, 1);
        }

        Ok(())
    }
}
