use crate::types::primitives::Hash;
use crate::types::error::VibecoinError;
use crate::ledger::block::Block;
use crate::consensus::poh::ProofOfHistory;
use crate::consensus::pow::{self, Difficulty};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// Slot duration in seconds
pub const SLOT_DURATION: u64 = 1;

// Slot information
#[derive(Debug, Clone)]
pub struct Slot {
    pub slot_number: u64,
    pub leader: Option<Hash>, // Public key of the slot leader (None if not determined yet)
    pub start_time: u64,      // Unix timestamp
    pub end_time: u64,        // Unix timestamp
    pub poh_start_hash: Hash, // PoH hash at the beginning of the slot
    pub poh_end_hash: Hash,   // PoH hash at the end of the slot (None if slot not completed)
    pub blocks: Vec<Hash>,    // Hashes of blocks produced in this slot
}

impl Slot {
    pub fn new(slot_number: u64, start_time: u64, poh_start_hash: Hash) -> Self {
        Slot {
            slot_number,
            leader: None,
            start_time,
            end_time: start_time + SLOT_DURATION,
            poh_start_hash,
            poh_end_hash: [0u8; 32],
            blocks: Vec::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        current_time >= self.start_time && current_time < self.end_time
    }

    pub fn is_complete(&self) -> bool {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        current_time >= self.end_time
    }

    pub fn add_block(&mut self, block_hash: Hash) {
        self.blocks.push(block_hash);
    }

    pub fn set_leader(&mut self, leader_key: Hash) {
        self.leader = Some(leader_key);
    }

    pub fn complete(&mut self, poh_end_hash: Hash) {
        self.poh_end_hash = poh_end_hash;
    }
}

// Slot manager to handle slot transitions and leader selection
pub struct SlotManager {
    pub current_slot: Slot,
    pub slots_history: Vec<Slot>,
    pub slot_duration: Duration,
}

impl SlotManager {
    pub fn new(genesis_hash: Hash) -> Self {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let current_slot = Slot::new(0, current_time, genesis_hash);
        
        SlotManager {
            current_slot,
            slots_history: Vec::new(),
            slot_duration: Duration::from_secs(SLOT_DURATION),
        }
    }

    pub fn advance_slot(&mut self, poh: &ProofOfHistory) -> Slot {
        // Store the current slot in history
        let completed_slot = self.current_slot.clone();
        self.slots_history.push(completed_slot);
        
        // Create a new slot
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let new_slot_number = self.current_slot.slot_number + 1;
        let new_slot = Slot::new(new_slot_number, current_time, poh.last_hash);
        
        // Update current slot
        self.current_slot = new_slot;
        
        self.current_slot.clone()
    }

    pub fn get_current_slot(&self) -> &Slot {
        &self.current_slot
    }

    pub fn get_slot_by_number(&self, slot_number: u64) -> Option<&Slot> {
        if slot_number == self.current_slot.slot_number {
            return Some(&self.current_slot);
        }
        
        self.slots_history.iter().find(|slot| slot.slot_number == slot_number)
    }

    // Determine if the node is the leader for the current slot based on PoW solution
    pub fn is_leader(&self, miner_key: &Hash, pow_solution: &Hash, difficulty: Difficulty) -> bool {
        // Check if the PoW solution meets the difficulty requirement
        if !pow::meets_difficulty(pow_solution, difficulty) {
            return false;
        }
        
        // In a real implementation, we would also verify that the PoW solution
        // is correctly derived from the previous slot's data
        
        true
    }

    // Generate PoH stream during the slot
    pub fn generate_poh_stream(&self, poh: &mut ProofOfHistory, stop_signal: Option<Arc<AtomicBool>>) -> Result<(), VibecoinError> {
        let start_time = Instant::now();
        let slot_end_time = start_time + self.slot_duration;
        
        println!("Generating PoH stream for slot {}", self.current_slot.slot_number);
        
        while Instant::now() < slot_end_time {
            // Check if we should stop
            if let Some(signal) = &stop_signal {
                if signal.load(Ordering::Relaxed) {
                    return Err(VibecoinError::MiningInterrupted);
                }
            }
            
            // Generate a PoH tick
            poh.tick();
            
            // Sleep a small amount to control tick rate
            std::thread::sleep(Duration::from_millis(10));
        }
        
        println!("Completed PoH stream for slot {}", self.current_slot.slot_number);
        Ok(())
    }
}

// Function to mine a PoW solution to become the leader for the next slot
pub fn mine_slot_leader(
    previous_slot: &Slot,
    difficulty: Difficulty,
    miner_key: &Hash,
    stop_signal: Option<Arc<AtomicBool>>,
    max_time: Option<Duration>,
) -> Result<Hash, VibecoinError> {
    let start_time = Instant::now();
    let mut nonce: u64 = 0;
    
    println!("Mining PoW solution to become leader for slot {}", previous_slot.slot_number + 1);
    
    // Create data to mine on: previous slot hash + miner key
    let mut data = Vec::with_capacity(64);
    data.extend_from_slice(&previous_slot.poh_end_hash);
    data.extend_from_slice(miner_key);
    
    // Start mining loop
    loop {
        // Check if we should stop
        if let Some(signal) = &stop_signal {
            if signal.load(Ordering::Relaxed) {
                return Err(VibecoinError::MiningInterrupted);
            }
        }
        
        // Check if we've exceeded max time
        if let Some(max_duration) = max_time {
            if start_time.elapsed() > max_duration {
                return Err(VibecoinError::MiningTimeout);
            }
        }
        
        // Try a new nonce
        let nonce_bytes = nonce.to_le_bytes();
        let mut hash_data = data.clone();
        hash_data.extend_from_slice(&nonce_bytes);
        
        // Calculate hash
        let hash = sha2::Sha256::digest(&hash_data);
        let mut pow_solution = [0u8; 32];
        pow_solution.copy_from_slice(&hash);
        
        // Check if the hash meets the difficulty requirement
        if pow::meets_difficulty(&pow_solution, difficulty) {
            println!("Found PoW solution! Nonce: {}, Hash: {}", nonce, hex::encode(pow_solution));
            return Ok(pow_solution);
        }
        
        // Increment nonce and continue
        nonce += 1;
        
        // Periodically yield to other threads
        if nonce % 100_000 == 0 {
            std::thread::yield_now();
            
            // Print progress every million attempts
            if nonce % 1_000_000 == 0 {
                println!("Mining... Tried {} nonces", nonce);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_slot_creation() {
        let genesis_hash = [0u8; 32];
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let slot = Slot::new(0, current_time, genesis_hash);
        
        assert_eq!(slot.slot_number, 0);
        assert_eq!(slot.start_time, current_time);
        assert_eq!(slot.end_time, current_time + SLOT_DURATION);
        assert_eq!(slot.poh_start_hash, genesis_hash);
        assert!(slot.blocks.is_empty());
    }
    
    #[test]
    fn test_slot_manager() {
        let genesis_hash = [0u8; 32];
        let mut manager = SlotManager::new(genesis_hash);
        
        assert_eq!(manager.current_slot.slot_number, 0);
        assert!(manager.slots_history.is_empty());
        
        // Create a PoH instance
        let mut poh = ProofOfHistory::new(genesis_hash, 1000);
        
        // Advance slot
        let new_slot = manager.advance_slot(&poh);
        
        assert_eq!(new_slot.slot_number, 1);
        assert_eq!(manager.slots_history.len(), 1);
        assert_eq!(manager.slots_history[0].slot_number, 0);
    }
    
    #[test]
    fn test_mine_slot_leader() -> Result<(), VibecoinError> {
        let genesis_hash = [0u8; 32];
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut slot = Slot::new(0, current_time, genesis_hash);
        slot.poh_end_hash = [1u8; 32]; // Set a non-zero end hash
        
        let miner_key = [2u8; 32];
        let difficulty = 1; // Very low difficulty for testing
        
        let pow_solution = mine_slot_leader(
            &slot,
            difficulty,
            &miner_key,
            None,
            Some(Duration::from_secs(1))
        )?;
        
        assert!(pow::meets_difficulty(&pow_solution, difficulty));
        
        Ok(())
    }
}
